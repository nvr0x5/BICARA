//! Audio capture for BICARA.
//!
//! * [`list_audio_apps`] — apps that currently have an audio session (with "playing" flag).
//! * [`AppCapture`] — capture the audio of ONE chosen app (Zoom, a browser running Google Meet, …).
//!   - macOS 14.2+: Core Audio process taps (see `native/macos/tap.m`).
//!   - Windows 10 2004+: WASAPI process loopback including the whole process tree.
//! * [`MicCapture`] — the user's default microphone via cpal.
//!
//! All captures deliver **mono f32** samples plus the sample rate to a [`Sink`].

use serde::{Deserialize, Serialize};

mod mic;
pub use mic::MicCapture;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(windows)]
mod windows_impl;
#[cfg(windows)]
use windows_impl as platform;

#[cfg(not(any(target_os = "macos", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "macos", windows)))]
use unsupported as platform;

/// Receives mono f32 samples and the sample rate. Called on an audio thread: keep it fast.
pub type Sink = Box<dyn FnMut(&[f32], u32) + Send + 'static>;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("{0}")]
    Platform(String),
    #[error("aplikasi tidak ditemukan: {0}")]
    AppNotFound(String),
    #[error("tidak didukung di OS ini")]
    Unsupported,
}

pub type Result<T> = std::result::Result<T, AudioError>;

/// An application that has (or had) an audio session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioApp {
    /// Stable id used to start a capture (macOS: bundle id, Windows: `pid:<root pid>`).
    pub id: String,
    pub name: String,
    pub pids: Vec<u32>,
    /// True when the app is producing audio right now.
    pub playing: bool,
    /// A user-facing app (has a Dock/taskbar presence). Background daemons are hidden
    /// from the picker unless they are playing audio.
    #[serde(default = "yes")]
    pub is_app: bool,
    pub icon_png_base64: Option<String>,
}

fn yes() -> bool {
    true
}

/// List applications with audio sessions, in a stable alphabetical order (the UI refreshes
/// this list periodically, so it must not reshuffle under the user's cursor).
pub fn list_audio_apps() -> Result<Vec<AudioApp>> {
    let mut apps = platform::list_audio_apps()?;
    apps.retain(|a| a.is_app || a.playing);
    apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(apps)
}

/// A running capture of one application's audio. Stops on drop.
pub struct AppCapture {
    inner: Option<platform::Capture>,
}

impl AppCapture {
    pub fn start(app_id: &str, sink: Sink) -> Result<Self> {
        Ok(Self {
            inner: Some(platform::Capture::start(app_id, sink)?),
        })
    }

    pub fn stop(mut self) {
        self.inner.take();
    }
}

impl Drop for AppCapture {
    fn drop(&mut self) {
        self.inner.take();
    }
}

/// Simple level meter helper: RMS and peak of a block.
pub fn levels(samples: &[f32]) -> (f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }
    let mut sum = 0.0f32;
    let mut peak = 0.0f32;
    for &s in samples {
        sum += s * s;
        peak = peak.max(s.abs());
    }
    ((sum / samples.len() as f32).sqrt(), peak)
}
