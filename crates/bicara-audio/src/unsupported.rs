use crate::{AudioApp, AudioError, Result, Sink};

pub fn list_audio_apps() -> Result<Vec<AudioApp>> {
    Err(AudioError::Unsupported)
}

pub struct Capture;

impl Capture {
    pub fn start(_app_id: &str, _sink: Sink) -> Result<Self> {
        Err(AudioError::Unsupported)
    }
}
