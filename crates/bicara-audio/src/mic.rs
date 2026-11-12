use std::sync::mpsc;
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use crate::{AudioError, Result, Sink};

/// Captures the default microphone. The cpal stream lives on its own thread because
/// `cpal::Stream` is not `Send` on every platform. Stops on drop.
pub struct MicCapture {
    stop_tx: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

fn err(e: impl std::fmt::Display) -> AudioError {
    AudioError::Platform(format!("mic: {e}"))
}

impl MicCapture {
    pub fn start(sink: Sink) -> Result<Self> {
        Self::start_with(None, sink)
    }

    /// Start capturing from the named input device (`None` = system default).
    pub fn start_with(device: Option<String>, mut sink: Sink) -> Result<Self> {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();

        let thread = std::thread::Builder::new()
            .name("bicara-mic".into())
            .spawn(move || {
                let build = || -> Result<cpal::Stream> {
                    let host = cpal::default_host();
                    let named = device.as_deref().and_then(|want| {
                        host.input_devices()
                            .ok()?
                            .find(|d| d.name().ok().as_deref() == Some(want))
                    });
                    let dev = named
                        .or_else(|| host.default_input_device())
                        .ok_or_else(|| err("tidak ada mikrofon"))?;
                    let cfg = dev.default_input_config().map_err(err)?;
                    let rate = cfg.sample_rate().0;
                    let ch = cfg.channels() as usize;
                    let fmt = cfg.sample_format();
                    let mut mono: Vec<f32> = Vec::with_capacity(4096);
                    let on_err = |e| eprintln!("[bicara-mic] stream error: {e}");
                    let stream = match fmt {
                        SampleFormat::F32 => dev.build_input_stream(
                            &cfg.into(),
                            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                                mono.clear();
                                mono.extend(
                                    data.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32),
                                );
                                sink(&mono, rate);
                            },
                            on_err,
                            None,
                        ),
                        SampleFormat::I16 => dev.build_input_stream(
                            &cfg.into(),
                            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                                mono.clear();
                                mono.extend(data.chunks(ch).map(|f| {
                                    f.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / ch as f32
                                }));
                                sink(&mono, rate);
                            },
                            on_err,
                            None,
                        ),
                        other => return Err(err(format!("format {other:?} belum didukung"))),
                    }
                    .map_err(err)?;
                    stream.play().map_err(err)?;
                    Ok(stream)
                };
                match build() {
                    Ok(stream) => {
                        let _ = ready_tx.send(Ok(()));
                        let _ = stop_rx.recv(); // block until stop/drop
                        drop(stream);
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                }
            })
            .map_err(err)?;

        ready_rx.recv().map_err(err)??;
        Ok(Self {
            stop_tx: Some(stop_tx),
            thread: Some(thread),
        })
    }
}

impl Drop for MicCapture {
    fn drop(&mut self) {
        self.stop_tx.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Names of the available microphones / input devices, default first.
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let default = host.default_input_device().and_then(|d| d.name().ok());
    let mut names: Vec<String> = host
        .input_devices()
        .map(|it| it.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default();
    names.dedup();
    if let Some(d) = default {
        names.retain(|n| *n != d);
        names.insert(0, d);
    }
    names
}
