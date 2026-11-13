//! Live speech-to-text for BICARA.
//!
//! Pipeline per audio stream ("app" = Mereka, "mic" = Saya):
//! `any-rate mono f32` → [`Resampler`] (16 kHz) → [`Segmenter`] (energy VAD, cuts on pauses)
//! → [`LiveTranscriber`] (one shared whisper.cpp model, one worker thread) → callback.

mod resample;
mod vad;

pub use resample::Resampler;
pub use vad::{Segmenter, SpeechSegment};

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::thread::JoinHandle;

use serde::Serialize;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub const SAMPLE_RATE: u32 = 16_000;

#[derive(Debug, thiserror::Error)]
pub enum SttError {
    #[error("model whisper tidak bisa dimuat: {0}")]
    Model(String),
}

#[derive(Debug, Clone)]
pub struct SttOptions {
    /// ISO code, e.g. "id". `None` = auto-detect.
    pub language: Option<String>,
    /// Custom vocabulary / style hint fed to whisper as the initial prompt.
    pub initial_prompt: String,
    pub threads: i32,
}

impl Default for SttOptions {
    fn default() -> Self {
        Self {
            language: Some("id".into()),
            initial_prompt:
                "Percakapan rapat dalam bahasa Indonesia, kadang bercampur istilah bahasa Inggris."
                    .into(),
            threads: std::thread::available_parallelism()
                .map(|n| n.get() as i32)
                .unwrap_or(4)
                .min(8),
        }
    }
}

/// One finished utterance.
#[derive(Debug, Clone, Serialize)]
pub struct Utterance {
    pub source: String,
    pub text: String,
    /// Seconds since the stream started.
    pub start: f64,
    pub end: f64,
    /// Wall-clock time the transcription took.
    pub latency_ms: u64,
    /// In-progress text for an utterance that is still being spoken; replaced by the final one.
    pub partial: bool,
}

struct Job {
    source: String,
    seg: SpeechSegment,
    partial: bool,
}

/// Partial (in-progress) transcripts: first after this much speech, then every `PARTIAL_STEP`.
const PARTIAL_MIN: usize = SAMPLE_RATE as usize * 6 / 5;
const PARTIAL_STEP: usize = SAMPLE_RATE as usize;

/// Owns the whisper model and transcribes segments from all streams on one worker thread
/// (whisper is not cheap; serialising keeps the GPU/CPU from thrashing).
pub struct LiveTranscriber {
    tx: Option<Sender<Job>>,
    /// Final segments waiting for the worker; partials are only sent/run when this is 0.
    queued: Arc<AtomicUsize>,
    worker: Option<JoinHandle<()>>,
}

/// Known whisper hallucinations on silence/noise (esp. for Indonesian).
const HALLUCINATIONS: &[&str] = &[
    "terima kasih telah menonton",
    "terima kasih sudah menonton",
    "terima kasih.",
    "terima kasih",
    "sampai jumpa",
    "sub indo",
    "subtitle",
    "[musik]",
    "(musik)",
    "♪",
];

fn is_hallucination(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    t.is_empty()
        || HALLUCINATIONS
            .iter()
            .any(|h| t == *h || (t.len() < 40 && t.contains(h) && h.len() > 8))
}

impl LiveTranscriber {
    pub fn new(
        model_path: &str,
        opts: SttOptions,
        mut on_result: impl FnMut(Utterance) + Send + 'static,
    ) -> Result<Self, SttError> {
        let mut cparams = WhisperContextParameters::default();
        cparams.flash_attn = true;
        let ctx = WhisperContext::new_with_params(model_path, cparams)
            .map_err(|e| SttError::Model(e.to_string()))?;
        let (tx, rx) = channel::<Job>();
        let queued = Arc::new(AtomicUsize::new(0));
        let q = queued.clone();
        let worker = std::thread::Builder::new()
            .name("bicara-stt".into())
            .spawn(move || {
                let mut state = match ctx.create_state() {
                    Ok(s) => s,
                    Err(e) => {
                        log::error!("[bicara-stt] create_state: {e}");
                        return;
                    }
                };
                while let Ok(job) = rx.recv() {
                    if !job.partial {
                        q.fetch_sub(1, Ordering::SeqCst);
                    } else if q.load(Ordering::SeqCst) > 0 {
                        continue; // stale: final work is waiting
                    }
                    let t0 = std::time::Instant::now();
                    let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                    p.set_language(opts.language.as_deref());
                    p.set_n_threads(opts.threads);
                    p.set_translate(false);
                    p.set_no_context(true);
                    p.set_single_segment(false);
                    p.set_suppress_blank(true);
                    p.set_suppress_nst(true);
                    p.set_no_speech_thold(0.6);
                    p.set_print_special(false);
                    p.set_print_progress(false);
                    p.set_print_realtime(false);
                    p.set_print_timestamps(false);
                    if !opts.initial_prompt.is_empty() {
                        p.set_initial_prompt(&opts.initial_prompt);
                    }
                    if let Err(e) = state.full(p, &job.seg.samples) {
                        log::error!("[bicara-stt] full: {e}");
                        continue;
                    }
                    let mut text = String::new();
                    for s in state.as_iter() {
                        if s.no_speech_probability() > 0.8 {
                            continue;
                        }
                        if let Ok(t) = s.to_str_lossy() {
                            text.push_str(&t);
                        }
                    }
                    let text = text.trim().to_string();
                    if is_hallucination(&text) || (job.partial && text.len() < 3) {
                        continue;
                    }
                    on_result(Utterance {
                        source: job.source,
                        text,
                        start: job.seg.start,
                        end: job.seg.end,
                        latency_ms: t0.elapsed().as_millis() as u64,
                        partial: job.partial,
                    });
                }
            })
            .map_err(|e| SttError::Model(e.to_string()))?;
        Ok(Self {
            tx: Some(tx),
            queued,
            worker: Some(worker),
        })
    }

    /// Creates a feeder for one audio stream. Call [`StreamFeeder::push`] from a normal thread
    /// (not the realtime audio callback).
    pub fn feeder(&self, source: &str) -> StreamFeeder {
        StreamFeeder {
            source: source.to_string(),
            resampler: None,
            segmenter: Segmenter::new(),
            tx: self.tx.clone().expect("transcriber running"),
            queued: self.queued.clone(),
            buf: Vec::with_capacity(8192),
            partials: true,
            last_partial: 0,
        }
    }
}

impl Drop for LiveTranscriber {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

pub struct StreamFeeder {
    source: String,
    resampler: Option<Resampler>,
    segmenter: Segmenter,
    tx: Sender<Job>,
    queued: Arc<AtomicUsize>,
    buf: Vec<f32>,
    partials: bool,
    /// Length (samples) of the open utterance when the last partial was sent.
    last_partial: usize,
}

impl StreamFeeder {
    /// Turn in-progress transcripts on or off (default on).
    pub fn with_partials(mut self, on: bool) -> Self {
        self.partials = on;
        self
    }

    fn send_final(&mut self, seg: SpeechSegment) {
        self.last_partial = 0;
        self.queued.fetch_add(1, Ordering::SeqCst);
        let _ = self.tx.send(Job {
            source: self.source.clone(),
            seg,
            partial: false,
        });
    }

    pub fn push(&mut self, samples: &[f32], rate: u32) {
        let rs = self
            .resampler
            .get_or_insert_with(|| Resampler::new(rate, SAMPLE_RATE));
        self.buf.clear();
        rs.process(samples, &mut self.buf);
        let segs = self.segmenter.push(&self.buf);
        for seg in segs {
            self.send_final(seg);
        }
        if !self.partials || self.queued.load(Ordering::SeqCst) > 0 {
            return;
        }
        if let Some((samples, start)) = self.segmenter.open() {
            let n = samples.len();
            if n < self.last_partial {
                self.last_partial = 0; // a long utterance was cut and continues
            }
            if n >= PARTIAL_MIN && n - self.last_partial >= PARTIAL_STEP {
                self.last_partial = n;
                let seg = SpeechSegment {
                    samples: samples.to_vec(),
                    start,
                    end: start + n as f64 / SAMPLE_RATE as f64,
                };
                let _ = self.tx.send(Job {
                    source: self.source.clone(),
                    seg,
                    partial: true,
                });
            }
        }
    }

    /// Flush the last open utterance (call when the stream stops).
    pub fn finish(&mut self) {
        if let Some(seg) = self.segmenter.flush() {
            self.send_final(seg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hallucination_filter() {
        assert!(is_hallucination("Terima kasih."));
        assert!(is_hallucination("  "));
        assert!(!is_hallucination(
            "Terima kasih atas penjelasannya, saya mau tanya soal anggaran."
        ));
    }
}
