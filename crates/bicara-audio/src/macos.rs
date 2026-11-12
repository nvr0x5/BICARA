use std::ffi::{c_char, c_void, CStr, CString};

use crate::{AudioApp, AudioError, Result, Sink};

type Callback = extern "C" fn(ctx: *mut c_void, samples: *const f32, frames: u32, sample_rate: u32);

unsafe extern "C" {
    fn bicara_mac_list_audio_apps() -> *mut c_char;
    fn bicara_mac_start_tap(
        app_id: *const c_char,
        cb: Callback,
        ctx: *mut c_void,
        out_sample_rate: *mut u32,
        err: *mut *mut c_char,
    ) -> *mut c_void;
    fn bicara_mac_stop_tap(handle: *mut c_void);
    fn bicara_free(s: *mut c_char);
}

unsafe fn take_string(p: *mut c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    unsafe { bicara_free(p) };
    s
}

pub fn list_audio_apps() -> Result<Vec<AudioApp>> {
    let json = unsafe { take_string(bicara_mac_list_audio_apps()) };
    serde_json::from_str(&json).map_err(|e| AudioError::Platform(e.to_string()))
}

extern "C" fn trampoline(ctx: *mut c_void, samples: *const f32, frames: u32, sample_rate: u32) {
    // SAFETY: ctx is the Box<Sink> we leaked in `start`; the native side calls us from a
    // single serial queue and drains it before `bicara_mac_stop_tap` returns.
    let sink = unsafe { &mut *(ctx as *mut Sink) };
    let data = unsafe { std::slice::from_raw_parts(samples, frames as usize) };
    sink(data, sample_rate);
}

pub struct Capture {
    handle: *mut c_void,
    ctx: *mut Sink,
}

// The native handle is only touched from start/drop.
unsafe impl Send for Capture {}

impl Capture {
    pub fn start(app_id: &str, sink: Sink) -> Result<Self> {
        let id = CString::new(app_id).map_err(|_| AudioError::AppNotFound(app_id.into()))?;
        let ctx = Box::into_raw(Box::new(sink));
        let mut rate = 0u32;
        let mut err: *mut c_char = std::ptr::null_mut();
        let handle = unsafe {
            bicara_mac_start_tap(
                id.as_ptr(),
                trampoline,
                ctx as *mut c_void,
                &mut rate,
                &mut err,
            )
        };
        if handle.is_null() {
            drop(unsafe { Box::from_raw(ctx) });
            return Err(AudioError::Platform(unsafe { take_string(err) }));
        }
        Ok(Self { handle, ctx })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        unsafe {
            bicara_mac_stop_tap(self.handle);
            drop(Box::from_raw(self.ctx));
        }
    }
}
