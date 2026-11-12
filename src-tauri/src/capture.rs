//! Capture session: app audio ("Mereka") + optional mic ("Saya").
//! Each stream is written to a mono f32 WAV and its level is emitted to the UI (~15 Hz).

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bicara_audio::{AppCapture, AudioApp, MicCapture, Sink};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::AppState;

pub struct Session {
    app: Option<AppCapture>,
    mic: Option<MicCapture>,
    writers: Vec<JoinHandle<Option<PathBuf>>>,
    pub dir: PathBuf,
}

#[derive(Clone, Serialize)]
struct Level {
    source: &'static str,
    rms: f32,
    peak: f32,
}

#[derive(Serialize)]
pub struct SessionInfo {
    pub dir: String,
    pub transcribing: bool,
    pub warning: Option<String>,
}

type Chunk = (Vec<f32>, u32);

/// Builds a sink for an audio callback plus a writer thread that saves WAV + emits levels.
/// The audio thread never blocks: if the writer falls behind, chunks are dropped.
fn stream_pipeline(
    app: AppHandle,
    source: &'static str,
    path: PathBuf,
    feeder: Option<bicara_stt::StreamFeeder>,
) -> (Sink, JoinHandle<Option<PathBuf>>) {
    let (tx, rx) = sync_channel::<Chunk>(256);
    let sink: Sink = Box::new(move |samples, rate| {
        let _ = tx.try_send((samples.to_vec(), rate));
    });
    let handle = std::thread::Builder::new()
        .name(format!("bicara-writer-{source}"))
        .spawn(move || writer_loop(app, source, path, rx, feeder))
        .expect("spawn writer");
    (sink, handle)
}

fn writer_loop(
    app: AppHandle,
    source: &'static str,
    path: PathBuf,
    rx: Receiver<Chunk>,
    mut feeder: Option<bicara_stt::StreamFeeder>,
) -> Option<PathBuf> {
    let mut writer: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>> = None;
    let (mut sq, mut n, mut peak) = (0f64, 0usize, 0f32);
    let mut last = Instant::now();
    while let Ok((buf, rate)) = rx.recv() {
        if writer.is_none() {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: rate,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };
            match hound::WavWriter::create(&path, spec) {
                Ok(w) => writer = Some(w),
                Err(e) => {
                    eprintln!("[bicara] cannot create {path:?}: {e}");
                    return None;
                }
            }
        }
        let w = writer.as_mut().unwrap();
        for &s in &buf {
            let _ = w.write_sample(s);
            sq += (s as f64) * (s as f64);
            peak = peak.max(s.abs());
        }
        n += buf.len();
        if let Some(f) = feeder.as_mut() {
            f.push(&buf, rate);
        }
        if last.elapsed() >= Duration::from_millis(66) && n > 0 {
            let _ = app.emit(
                "audio-level",
                Level {
                    source,
                    rms: (sq / n as f64).sqrt() as f32,
                    peak,
                },
            );
            (sq, n, peak) = (0.0, 0, 0.0);
            last = Instant::now();
        }
    }
    if let Some(f) = feeder.as_mut() {
        f.finish();
    }
    let _ = app.emit(
        "audio-level",
        Level {
            source,
            rms: 0.0,
            peak: 0.0,
        },
    );
    writer.and_then(|w| w.finalize().ok()).map(|_| path)
}

#[tauri::command]
pub fn list_audio_apps() -> Result<Vec<AudioApp>, String> {
    bicara_audio::list_audio_apps().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn recordings_dir(app: AppHandle) -> Result<String, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("recordings");
    Ok(dir.to_string_lossy().into_owned())
}

// `async` so loading the speech model (a few seconds) never blocks the UI thread.
#[tauri::command(async)]
pub fn start_capture(
    app: AppHandle,
    state: State<'_, AppState>,
    app_id: Option<String>,
    mic: bool,
) -> Result<SessionInfo, String> {
    let mut guard = state.session.lock().unwrap();
    if guard.is_some() {
        return Err("Rekaman sedang berjalan.".into());
    }
    if app_id.is_none() && !mic {
        return Err("Pilih aplikasi atau aktifkan mikrofon.".into());
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("recordings")
        .join(format!("session-{ts}"));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // Live transcription is optional: without a model we still record.
    let (stt, warning) = match crate::models::transcriber(&app) {
        Ok(t) => (Some(t), None),
        Err(e) => (None, Some(e)),
    };
    let transcribing = stt.is_some();

    let mut session = Session {
        app: None,
        mic: None,
        writers: vec![],
        dir: dir.clone(),
    };

    if let Some(id) = app_id {
        let (sink, w) = stream_pipeline(
            app.clone(),
            "app",
            dir.join("mereka-app.wav"),
            stt.as_ref().map(|t| t.feeder("app")),
        );
        session.writers.push(w);
        session.app = Some(AppCapture::start(&id, sink).map_err(|e| e.to_string())?);
    }
    if mic {
        let (sink, w) = stream_pipeline(
            app.clone(),
            "mic",
            dir.join("saya-mic.wav"),
            stt.as_ref().map(|t| t.feeder("mic")),
        );
        session.writers.push(w);
        let device = state.settings.lock().unwrap().mic_device.clone();
        match MicCapture::start_with(device, sink) {
            Ok(m) => session.mic = Some(m),
            Err(e) => {
                stop_session(session);
                return Err(e.to_string());
            }
        }
    }
    *guard = Some(session);
    Ok(SessionInfo {
        dir: dir.to_string_lossy().into_owned(),
        transcribing,
        warning,
    })
}

fn stop_session(mut s: Session) -> Vec<String> {
    // Dropping captures drops their sinks → writer channels close → writers finalize.
    s.app.take();
    s.mic.take();
    s.writers
        .into_iter()
        .filter_map(|w| w.join().ok().flatten())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

#[tauri::command(async)]
pub fn stop_capture(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let session = state.session.lock().unwrap().take();
    Ok(session.map(stop_session).unwrap_or_default())
}
