//! Energy-based voice activity detection that cuts a 16 kHz stream into utterances.
//! Adaptive noise floor; an utterance ends after a pause or when it gets too long.

const FRAME: usize = 480; // 30 ms at 16 kHz
const PRE_ROLL_FRAMES: usize = 10; // 300 ms kept before speech onset
const END_SILENCE_FRAMES: usize = 20; // 600 ms pause ends an utterance
const MIN_SPEECH_FRAMES: usize = 10; // ignore blips shorter than 300 ms
const MAX_FRAMES: usize = 15 * 1000 / 30; // force a cut at 15 s

#[derive(Debug, Clone)]
pub struct SpeechSegment {
    pub samples: Vec<f32>,
    pub start: f64,
    pub end: f64,
}

pub struct Segmenter {
    pending: Vec<f32>,
    frames_seen: u64,
    noise: f32,
    in_speech: bool,
    pre_roll: std::collections::VecDeque<Vec<f32>>,
    cur: Vec<f32>,
    cur_start_frame: u64,
    speech_frames: usize,
    silence_run: usize,
}

impl Default for Segmenter {
    fn default() -> Self {
        Self::new()
    }
}

impl Segmenter {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            frames_seen: 0,
            noise: 0.003,
            in_speech: false,
            pre_roll: Default::default(),
            cur: Vec::new(),
            cur_start_frame: 0,
            speech_frames: 0,
            silence_run: 0,
        }
    }

    fn threshold(&self) -> f32 {
        (self.noise * 3.0).max(0.008)
    }

    pub fn push(&mut self, samples: &[f32]) -> Vec<SpeechSegment> {
        let mut out = Vec::new();
        self.pending.extend_from_slice(samples);
        let mut off = 0;
        while self.pending.len() - off >= FRAME {
            let frame = self.pending[off..off + FRAME].to_vec();
            off += FRAME;
            if let Some(s) = self.frame(frame) {
                out.push(s);
            }
        }
        self.pending.drain(..off);
        out
    }

    fn frame(&mut self, frame: Vec<f32>) -> Option<SpeechSegment> {
        let rms = (frame.iter().map(|v| v * v).sum::<f32>() / FRAME as f32).sqrt();
        let is_speech = rms > self.threshold();
        let idx = self.frames_seen;
        self.frames_seen += 1;

        if !self.in_speech {
            // Track the noise floor only outside speech.
            self.noise = if rms < self.noise {
                0.9 * self.noise + 0.1 * rms
            } else {
                0.995 * self.noise + 0.005 * rms
            };
            self.noise = self.noise.max(1e-4);
            if is_speech {
                self.in_speech = true;
                self.cur_start_frame = idx - self.pre_roll.len() as u64;
                self.cur = self.pre_roll.drain(..).flatten().collect();
                self.cur.extend_from_slice(&frame);
                self.speech_frames = 1;
                self.silence_run = 0;
            } else {
                self.pre_roll.push_back(frame);
                if self.pre_roll.len() > PRE_ROLL_FRAMES {
                    self.pre_roll.pop_front();
                }
            }
            return None;
        }

        self.cur.extend_from_slice(&frame);
        if is_speech {
            self.speech_frames += 1;
            self.silence_run = 0;
        } else {
            self.silence_run += 1;
        }
        let frames = self.cur.len() / FRAME;
        if self.silence_run >= END_SILENCE_FRAMES {
            return self.close();
        }
        if frames >= MAX_FRAMES {
            return self.cut_long();
        }
        None
    }

    /// Long utterance without a real pause: cut at the quietest frame of the last 2 s so we
    /// don't split a word, and keep the remainder as the start of the next utterance.
    fn cut_long(&mut self) -> Option<SpeechSegment> {
        let frames = self.cur.len() / FRAME;
        let search_from = frames.saturating_sub(2000 / 30);
        let energy = |f: usize| {
            self.cur[f * FRAME..(f + 1) * FRAME]
                .iter()
                .map(|v| v * v)
                .sum::<f32>()
        };
        let cut = (search_from..frames)
            .min_by(|&a, &b| energy(a).total_cmp(&energy(b)))
            .unwrap_or(frames);
        let rest = self.cur.split_off(((cut + 1) * FRAME).min(self.cur.len()));
        let rest_frames = rest.len() / FRAME;
        let seg = self.close();
        // Continue the utterance with the remainder.
        self.in_speech = true;
        self.cur_start_frame = self.frames_seen - rest_frames as u64;
        self.cur = rest;
        self.speech_frames = rest_frames;
        seg
    }

    fn close(&mut self) -> Option<SpeechSegment> {
        self.in_speech = false;
        let samples = std::mem::take(&mut self.cur);
        let speech = self.speech_frames;
        self.speech_frames = 0;
        self.silence_run = 0;
        if speech < MIN_SPEECH_FRAMES {
            return None;
        }
        let start = self.cur_start_frame as f64 * FRAME as f64 / 16_000.0;
        let end = start + samples.len() as f64 / 16_000.0;
        Some(SpeechSegment {
            samples,
            start,
            end,
        })
    }

    pub fn flush(&mut self) -> Option<SpeechSegment> {
        if self.in_speech {
            self.close()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(secs: f32, amp: f32) -> Vec<f32> {
        (0..(secs * 16_000.0) as usize)
            .map(|i| amp * (i as f32 * 0.2).sin())
            .collect()
    }

    #[test]
    fn splits_on_pause() {
        let mut s = Segmenter::new();
        let mut audio = vec![0.0005; 16_000];
        audio.extend(tone(1.5, 0.3));
        audio.extend(vec![0.0005; 16_000]);
        audio.extend(tone(2.0, 0.3));
        audio.extend(vec![0.0005; 16_000]);
        let segs: Vec<_> = audio.chunks(1600).flat_map(|c| s.push(c)).collect();
        assert_eq!(segs.len(), 2);
        assert!((segs[0].start - 0.7).abs() < 0.1, "{}", segs[0].start);
        assert!((segs[1].start - 3.2).abs() < 0.1, "{}", segs[1].start);
    }

    #[test]
    fn ignores_blips_and_forces_long_cut() {
        let mut s = Segmenter::new();
        let mut audio = vec![0.0005; 16_000];
        audio.extend(tone(0.1, 0.3));
        audio.extend(vec![0.0005; 16_000]);
        audio.extend(tone(20.0, 0.3));
        let mut segs: Vec<_> = audio.chunks(1600).flat_map(|c| s.push(c)).collect();
        segs.extend(s.flush());
        assert!(
            segs.len() >= 2,
            "{:?}",
            segs.iter().map(|x| (x.start, x.end)).collect::<Vec<_>>()
        );
        assert!(segs[0].end - segs[0].start <= 15.1);
    }
}
