// Per-application audio capture for macOS using Core Audio Process Taps (macOS 14.2+).
//
// Flow: find the Core Audio "process objects" belonging to the chosen app (including helper
// processes such as "Google Chrome Helper"), create a private stereo-mixdown tap over them,
// wrap the tap in a private aggregate device, and read audio from that device with an IOProc.
// Samples are downmixed to mono float32 and handed to a C callback (implemented in Rust).

#import <AppKit/AppKit.h>
#import <CoreAudio/AudioHardwareTapping.h>
#import <CoreAudio/CATapDescription.h>
#import <CoreAudio/CoreAudio.h>
#import <Foundation/Foundation.h>
#include <libproc.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef void (*bicara_audio_cb)(void *ctx, const float *samples, uint32_t frames,
                                uint32_t sample_rate);

typedef struct {
    AudioObjectID tapID;
    AudioObjectID aggID;
    AudioDeviceIOProcID procID;
    void *queue; // retained dispatch_queue_t
    bicara_audio_cb cb;
    void *ctx;
    float *scratch;
    UInt32 scratchFrames;
    UInt32 sampleRate;
    UInt32 channels;
    BOOL nonInterleaved;
} BicaraTap;

static void setErr(char **err, NSString *msg) {
    if (err) *err = strdup(msg.UTF8String);
}

void bicara_free(char *s) { free(s); }

static OSStatus getProp(AudioObjectID obj, AudioObjectPropertySelector sel, UInt32 *size,
                        void *out) {
    AudioObjectPropertyAddress a = {sel, kAudioObjectPropertyScopeGlobal,
                                    kAudioObjectPropertyElementMain};
    return AudioObjectGetPropertyData(obj, &a, 0, NULL, size, out);
}

static NSArray<NSNumber *> *audioProcessObjects(void) {
    AudioObjectPropertyAddress a = {kAudioHardwarePropertyProcessObjectList,
                                    kAudioObjectPropertyScopeGlobal,
                                    kAudioObjectPropertyElementMain};
    UInt32 size = 0;
    if (AudioObjectGetPropertyDataSize(kAudioObjectSystemObject, &a, 0, NULL, &size) != noErr ||
        size == 0)
        return @[];
    AudioObjectID *ids = malloc(size);
    if (AudioObjectGetPropertyData(kAudioObjectSystemObject, &a, 0, NULL, &size, ids) != noErr) {
        free(ids);
        return @[];
    }
    NSMutableArray *r = [NSMutableArray array];
    for (UInt32 i = 0; i < size / sizeof(AudioObjectID); i++) [r addObject:@(ids[i])];
    free(ids);
    return r;
}

static pid_t processPID(AudioObjectID o) {
    pid_t pid = -1;
    UInt32 s = sizeof(pid);
    if (getProp(o, kAudioProcessPropertyPID, &s, &pid) != noErr) return -1;
    return pid;
}

static NSString *processBundleID(AudioObjectID o) {
    CFStringRef str = NULL;
    UInt32 s = sizeof(str);
    if (getProp(o, kAudioProcessPropertyBundleID, &s, &str) != noErr || !str) return nil;
    return (__bridge_transfer NSString *)str;
}

static BOOL processRunningOutput(AudioObjectID o) {
    UInt32 v = 0, s = sizeof(v);
    if (getProp(o, kAudioProcessPropertyIsRunningOutput, &s, &v) != noErr) return NO;
    return v != 0;
}

static NSString *processName(pid_t pid) {
    char buf[PROC_PIDPATHINFO_MAXSIZE];
    if (proc_name(pid, buf, sizeof(buf)) > 0) return [NSString stringWithUTF8String:buf];
    return [NSString stringWithFormat:@"pid %d", pid];
}

static NSString *iconPNG(NSImage *img) {
    if (!img) return nil;
    NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL
                                                                    pixelsWide:64
                                                                    pixelsHigh:64
                                                                 bitsPerSample:8
                                                               samplesPerPixel:4
                                                                      hasAlpha:YES
                                                                      isPlanar:NO
                                                                colorSpaceName:NSDeviceRGBColorSpace
                                                                   bytesPerRow:0
                                                                  bitsPerPixel:0];
    if (!rep) return nil;
    [NSGraphicsContext saveGraphicsState];
    [NSGraphicsContext setCurrentContext:[NSGraphicsContext graphicsContextWithBitmapImageRep:rep]];
    [img drawInRect:NSMakeRect(0, 0, 64, 64)
           fromRect:NSZeroRect
          operation:NSCompositingOperationCopy
           fraction:1.0];
    [NSGraphicsContext restoreGraphicsState];
    NSData *png = [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
    return [png base64EncodedStringWithOptions:0];
}

static BOOL isBundlePrefix(NSString *child, NSString *parent) {
    if (!child.length || !parent.length || ![child hasPrefix:parent]) return NO;
    return child.length == parent.length || [child characterAtIndex:parent.length] == '.';
}

// Groups Core Audio process objects by the user-facing app that owns them.
// Helper processes (e.g. com.google.Chrome.helper) are attached to their parent app
// (com.google.Chrome) via bundle-ID prefix.
static NSDictionary<NSString *, NSMutableDictionary *> *buildGroups(BOOL withIcons) {
    NSArray<NSRunningApplication *> *apps = [[NSWorkspace sharedWorkspace] runningApplications];
    pid_t me = getpid();
    NSMutableDictionary<NSString *, NSMutableDictionary *> *groups = [NSMutableDictionary dictionary];

    for (NSNumber *n in audioProcessObjects()) {
        AudioObjectID o = n.unsignedIntValue;
        pid_t pid = processPID(o);
        if (pid <= 0 || pid == me) continue;
        NSString *bid = processBundleID(o);

        NSRunningApplication *byPid = nil;
        for (NSRunningApplication *app in apps) {
            if (app.processIdentifier == pid) {
                byPid = app;
                break;
            }
        }
        NSRunningApplication *owner = nil;
        if (byPid && byPid.activationPolicy == NSApplicationActivationPolicyRegular) {
            owner = byPid;
        } else {
            NSString *lookup = bid.length ? bid : byPid.bundleIdentifier;
            NSUInteger best = 0;
            for (NSRunningApplication *app in apps) {
                NSString *ab = app.bundleIdentifier;
                if (app.activationPolicy == NSApplicationActivationPolicyRegular &&
                    isBundlePrefix(lookup, ab) && ab.length > best) {
                    owner = app;
                    best = ab.length;
                }
            }
            if (!owner) owner = byPid;
        }

        NSString *key = owner.bundleIdentifier.length ? owner.bundleIdentifier
                        : bid.length                  ? bid
                                     : [NSString stringWithFormat:@"pid:%d", pid];
        NSMutableDictionary *g = groups[key];
        if (!g) {
            NSString *name = owner.localizedName.length ? owner.localizedName : processName(pid);
            g = [@{
                @"id" : key,
                @"name" : name,
                @"pids" : [NSMutableArray array],
                @"objects" : [NSMutableArray array],
                @"playing" : @NO,
                @"is_app" : (owner && owner.activationPolicy == NSApplicationActivationPolicyRegular) ? @YES : @NO,
            } mutableCopy];
            if (withIcons) {
                NSString *icon = iconPNG(owner.icon);
                if (icon) g[@"icon"] = icon;
            }
            groups[key] = g;
        }
        [g[@"pids"] addObject:@(pid)];
        [g[@"objects"] addObject:n];
        if (processRunningOutput(o)) g[@"playing"] = @YES;
    }
    return groups;
}

// Returns a malloc'd JSON array: [{id, name, pids, playing, icon_png_base64}]. Free with bicara_free.
char *bicara_mac_list_audio_apps(void) {
    @autoreleasepool {
        NSDictionary *groups = buildGroups(YES);
        NSMutableArray *out = [NSMutableArray array];
        for (NSDictionary *g in groups.allValues) {
            [out addObject:@{
                @"id" : g[@"id"],
                @"name" : g[@"name"],
                @"pids" : g[@"pids"],
                @"playing" : g[@"playing"],
                @"is_app" : g[@"is_app"],
                @"icon_png_base64" : g[@"icon"] ?: [NSNull null],
            }];
        }
        NSData *d = [NSJSONSerialization dataWithJSONObject:out options:0 error:nil];
        if (!d) return strdup("[]");
        char *s = malloc(d.length + 1);
        memcpy(s, d.bytes, d.length);
        s[d.length] = 0;
        return s;
    }
}

static void destroyTap(BicaraTap *t) {
    if (!t) return;
    if (t->procID) {
        AudioDeviceStop(t->aggID, t->procID);
        AudioDeviceDestroyIOProcID(t->aggID, t->procID);
    }
    if (t->queue) {
        dispatch_queue_t q = (__bridge_transfer dispatch_queue_t)t->queue;
        dispatch_sync(q, ^{}); // drain any in-flight IO block before freeing ctx
        t->queue = NULL;
    }
    if (t->aggID != kAudioObjectUnknown) AudioHardwareDestroyAggregateDevice(t->aggID);
    if (t->tapID != kAudioObjectUnknown) AudioHardwareDestroyProcessTap(t->tapID);
    free(t->scratch);
    free(t);
}

void bicara_mac_stop_tap(void *handle) { destroyTap((BicaraTap *)handle); }

// Starts capturing the audio of the app with the given id (as returned by
// bicara_mac_list_audio_apps). Returns an opaque handle, or NULL with *err set.
void *bicara_mac_start_tap(const char *appId, bicara_audio_cb cb, void *ctx,
                           uint32_t *outSampleRate, char **err) {
    @autoreleasepool {
        if (@available(macOS 14.2, *)) {
        } else {
            setErr(err, @"Capture per aplikasi butuh macOS 14.2 atau lebih baru.");
            return NULL;
        }

        NSString *key = [NSString stringWithUTF8String:appId];
        NSDictionary *g = buildGroups(NO)[key];
        NSArray *objects = g[@"objects"];
        if (!objects.count) {
            setErr(err, @"Aplikasi ini belum memakai audio. Mulai meeting/putar suara dulu, lalu "
                        @"refresh daftar.");
            return NULL;
        }

        BicaraTap *t = calloc(1, sizeof(BicaraTap));
        t->tapID = kAudioObjectUnknown;
        t->aggID = kAudioObjectUnknown;
        t->cb = cb;
        t->ctx = ctx;

        CATapDescription *desc = [[CATapDescription alloc] initStereoMixdownOfProcesses:objects];
        desc.name = @"BICARA tap";
        desc.UUID = [NSUUID UUID];
        desc.privateTap = YES;
        desc.muteBehavior = CATapUnmuted;

        OSStatus st = AudioHardwareCreateProcessTap(desc, &t->tapID);
        if (st != noErr) {
            setErr(err, [NSString stringWithFormat:@"AudioHardwareCreateProcessTap gagal (%d). "
                                                   @"Cek izin System Audio Recording.",
                                                   (int)st]);
            destroyTap(t);
            return NULL;
        }

        AudioObjectID outDev = kAudioObjectUnknown;
        UInt32 s = sizeof(outDev);
        getProp(kAudioObjectSystemObject, kAudioHardwarePropertyDefaultSystemOutputDevice, &s,
                &outDev);
        CFStringRef uidRef = NULL;
        s = sizeof(uidRef);
        if (outDev == kAudioObjectUnknown ||
            getProp(outDev, kAudioDevicePropertyDeviceUID, &s, &uidRef) != noErr || !uidRef) {
            setErr(err, @"Tidak menemukan output device default.");
            destroyTap(t);
            return NULL;
        }
        NSString *outUID = (__bridge_transfer NSString *)uidRef;

        NSDictionary *agg = @{
            @kAudioAggregateDeviceNameKey : @"BICARA Capture",
            @kAudioAggregateDeviceUIDKey : [[NSUUID UUID] UUIDString],
            @kAudioAggregateDeviceMainSubDeviceKey : outUID,
            @kAudioAggregateDeviceIsPrivateKey : @YES,
            @kAudioAggregateDeviceIsStackedKey : @NO,
            @kAudioAggregateDeviceTapAutoStartKey : @YES,
            @kAudioAggregateDeviceSubDeviceListKey : @[ @{@kAudioSubDeviceUIDKey : outUID} ],
            @kAudioAggregateDeviceTapListKey : @[ @{
                @kAudioSubTapDriftCompensationKey : @YES,
                @kAudioSubTapUIDKey : desc.UUID.UUIDString,
            } ],
        };
        st = AudioHardwareCreateAggregateDevice((__bridge CFDictionaryRef)agg, &t->aggID);
        if (st != noErr) {
            setErr(err, [NSString stringWithFormat:@"AudioHardwareCreateAggregateDevice gagal (%d)",
                                                   (int)st]);
            destroyTap(t);
            return NULL;
        }

        AudioStreamBasicDescription asbd = {0};
        s = sizeof(asbd);
        st = getProp(t->tapID, kAudioTapPropertyFormat, &s, &asbd);
        if (st != noErr || asbd.mFormatID != kAudioFormatLinearPCM ||
            !(asbd.mFormatFlags & kAudioFormatFlagIsFloat) || asbd.mBitsPerChannel != 32 ||
            asbd.mChannelsPerFrame == 0) {
            setErr(err, [NSString stringWithFormat:@"Format tap tidak didukung (status %d)", (int)st]);
            destroyTap(t);
            return NULL;
        }
        t->sampleRate = (UInt32)asbd.mSampleRate;
        t->channels = asbd.mChannelsPerFrame;
        t->nonInterleaved = (asbd.mFormatFlags & kAudioFormatFlagIsNonInterleaved) != 0;
        if (outSampleRate) *outSampleRate = t->sampleRate;

        dispatch_queue_t q = dispatch_queue_create("id.bicara.tap", DISPATCH_QUEUE_SERIAL);
        t->queue = (__bridge_retained void *)q;

        BicaraTap *tt = t;
        st = AudioDeviceCreateIOProcIDWithBlock(
            &t->procID, t->aggID, q,
            ^(const AudioTimeStamp *inNow, const AudioBufferList *in, const AudioTimeStamp *inTime,
              AudioBufferList *outData, const AudioTimeStamp *outTime) {
              if (!in || in->mNumberBuffers == 0) return;
              UInt32 ch = tt->channels;
              UInt32 frames = tt->nonInterleaved
                                  ? in->mBuffers[0].mDataByteSize / sizeof(float)
                                  : in->mBuffers[0].mDataByteSize / (sizeof(float) * ch);
              if (frames == 0) return;
              if (frames > tt->scratchFrames) {
                  free(tt->scratch);
                  tt->scratch = malloc(frames * sizeof(float));
                  tt->scratchFrames = frames;
              }
              float *dst = tt->scratch;
              if (tt->nonInterleaved) {
                  UInt32 nb = in->mNumberBuffers;
                  for (UInt32 f = 0; f < frames; f++) {
                      float sum = 0;
                      for (UInt32 b = 0; b < nb; b++) sum += ((const float *)in->mBuffers[b].mData)[f];
                      dst[f] = sum / (float)nb;
                  }
              } else {
                  const float *src = (const float *)in->mBuffers[0].mData;
                  for (UInt32 f = 0; f < frames; f++) {
                      float sum = 0;
                      for (UInt32 c = 0; c < ch; c++) sum += src[f * ch + c];
                      dst[f] = sum / (float)ch;
                  }
              }
              tt->cb(tt->ctx, dst, frames, tt->sampleRate);
            });
        if (st != noErr) {
            t->procID = NULL;
            setErr(err, [NSString stringWithFormat:@"AudioDeviceCreateIOProcID gagal (%d)", (int)st]);
            destroyTap(t);
            return NULL;
        }

        st = AudioDeviceStart(t->aggID, t->procID);
        if (st != noErr) {
            setErr(err, [NSString stringWithFormat:@"AudioDeviceStart gagal (%d)", (int)st]);
            destroyTap(t);
            return NULL;
        }
        return t;
    }
}
