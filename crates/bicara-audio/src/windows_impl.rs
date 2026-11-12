//! Windows backend.
//!
//! * Listing: enumerate WASAPI audio sessions on every active render endpoint
//!   (`IAudioSessionManager2`), map each session's PID to its *root* process (walking up
//!   parents with the same exe name, so Chrome/Edge/Teams child processes collapse into the
//!   main process), and group by that root.
//! * Capture: WASAPI process loopback (`AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`) on the
//!   root PID with `INCLUDE_TARGET_PROCESS_TREE`. Requires Windows 10 2004 (build 19041)+.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;

use windows::core::Interface;
use windows::Win32::Foundation::{CloseHandle, S_OK};
use windows::Win32::Media::Audio::{
    eRender, AudioSessionStateActive, IAudioSessionControl2, IAudioSessionManager2,
    IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

use crate::{AudioApp, AudioError, Result, Sink};

fn werr(e: impl std::fmt::Display) -> AudioError {
    AudioError::Platform(e.to_string())
}

struct ProcInfo {
    parent: u32,
    exe: String,
}

fn process_table() -> HashMap<u32, ProcInfo> {
    let mut map = HashMap::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return map;
        };
        let mut e = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut e).is_ok() {
            loop {
                let len = e
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(e.szExeFile.len());
                map.insert(
                    e.th32ProcessID,
                    ProcInfo {
                        parent: e.th32ParentProcessID,
                        exe: String::from_utf16_lossy(&e.szExeFile[..len]),
                    },
                );
                if Process32NextW(snap, &mut e).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    map
}

/// Walk up the parent chain while the parent has the same executable name.
fn root_pid(pid: u32, procs: &HashMap<u32, ProcInfo>) -> u32 {
    let mut cur = pid;
    for _ in 0..32 {
        let Some(me) = procs.get(&cur) else { break };
        match procs.get(&me.parent) {
            Some(p) if me.parent != cur && p.exe.eq_ignore_ascii_case(&me.exe) => cur = me.parent,
            _ => break,
        }
    }
    cur
}

fn friendly_name(exe: &str) -> String {
    let stem = exe.trim_end_matches(".exe").trim_end_matches(".EXE");
    match stem.to_ascii_lowercase().as_str() {
        "chrome" => "Google Chrome".into(),
        "msedge" => "Microsoft Edge".into(),
        "firefox" => "Firefox".into(),
        "brave" => "Brave".into(),
        "zoom" => "Zoom".into(),
        "ms-teams" | "teams" => "Microsoft Teams".into(),
        "slack" => "Slack".into(),
        "discord" => "Discord".into(),
        "webexmta" | "ciscocollabhost" => "Webex".into(),
        "whatsapp" => "WhatsApp".into(),
        "telegram" => "Telegram".into(),
        _ => stem.to_string(),
    }
}

pub fn list_audio_apps() -> Result<Vec<AudioApp>> {
    let procs = process_table();
    let mut groups: HashMap<u32, AudioApp> = HashMap::new();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(werr)?;
        let devices = enumerator
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .map_err(werr)?;
        for i in 0..devices.GetCount().map_err(werr)? {
            let Ok(dev) = devices.Item(i) else { continue };
            let Ok(mgr) = dev.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
                continue;
            };
            let Ok(sessions) = mgr.GetSessionEnumerator() else {
                continue;
            };
            for j in 0..sessions.GetCount().unwrap_or(0) {
                let Ok(ctl) = sessions.GetSession(j) else {
                    continue;
                };
                let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                if ctl2.IsSystemSoundsSession() == S_OK {
                    continue;
                }
                let Ok(pid) = ctl2.GetProcessId() else {
                    continue;
                };
                if pid == 0 || pid == std::process::id() {
                    continue;
                }
                let playing = ctl
                    .GetState()
                    .map(|s| s == AudioSessionStateActive)
                    .unwrap_or(false);
                let root = root_pid(pid, &procs);
                let exe = procs.get(&root).map(|p| p.exe.clone()).unwrap_or_default();
                let g = groups.entry(root).or_insert_with(|| AudioApp {
                    id: format!("pid:{root}"),
                    name: if exe.is_empty() {
                        format!("pid {root}")
                    } else {
                        friendly_name(&exe)
                    },
                    pids: vec![],
                    playing: false,
                    is_app: true,
                    icon_png_base64: None,
                });
                if !g.pids.contains(&pid) {
                    g.pids.push(pid);
                }
                g.playing |= playing;
            }
        }
    }
    Ok(groups.into_values().collect())
}

pub struct Capture {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Capture {
    pub fn start(app_id: &str, mut sink: Sink) -> Result<Self> {
        let pid: u32 = app_id
            .strip_prefix("pid:")
            .and_then(|p| p.parse().ok())
            .ok_or_else(|| AudioError::AppNotFound(app_id.into()))?;
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();

        let thread = std::thread::Builder::new()
            .name("bicara-app-capture".into())
            .spawn(move || {
                const RATE: usize = 48_000;
                const CH: usize = 2;
                let setup = || -> Result<(wasapi::AudioClient, wasapi::AudioCaptureClient, wasapi::Handle)> {
                    wasapi::initialize_mta().ok().map_err(werr)?;
                    let fmt = wasapi::WaveFormat::new(32, 32, &wasapi::SampleType::Float, RATE, CH, None);
                    let mut client =
                        wasapi::AudioClient::new_application_loopback_client(pid, true).map_err(werr)?;
                    let mode = wasapi::StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: 0 };
                    client
                        .initialize_client(&fmt, &wasapi::Direction::Capture, &mode)
                        .map_err(werr)?;
                    let ev = client.set_get_eventhandle().map_err(werr)?;
                    let cap = client.get_audiocaptureclient().map_err(werr)?;
                    client.start_stream().map_err(werr)?;
                    Ok((client, cap, ev))
                };
                let (client, cap, ev) = match setup() {
                    Ok(x) => {
                        let _ = ready_tx.send(Ok(()));
                        x
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };

                let mut raw: VecDeque<u8> = VecDeque::new();
                let mut mono: Vec<f32> = Vec::with_capacity(4096);
                let frame_bytes = 4 * CH;
                while !stop2.load(Ordering::Relaxed) {
                    // Timeouts are normal while the target app is silent.
                    if ev.wait_for_event(200).is_err() {
                        continue;
                    }
                    loop {
                        match cap.get_next_packet_size() {
                            Ok(Some(n)) if n > 0 => {
                                if cap.read_from_device_to_deque(&mut raw).is_err() {
                                    break;
                                }
                            }
                            _ => break,
                        }
                    }
                    let frames = raw.len() / frame_bytes;
                    if frames == 0 {
                        continue;
                    }
                    mono.clear();
                    let bytes: Vec<u8> = raw.drain(..frames * frame_bytes).collect();
                    for f in bytes.chunks_exact(frame_bytes) {
                        let mut sum = 0.0f32;
                        for c in f.chunks_exact(4) {
                            sum += f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                        }
                        mono.push(sum / CH as f32);
                    }
                    sink(&mono, RATE as u32);
                }
                let _ = client.stop_stream();
            })
            .map_err(werr)?;

        ready_rx.recv().map_err(werr)??;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
