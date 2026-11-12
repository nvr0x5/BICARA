//! Streaming resampler: windowed-sinc low-pass FIR (anti-aliasing) + linear interpolation.
//! Good enough for speech recognition at 16 kHz; keeps state across chunks.

pub struct Resampler {
    step: f64, // input samples per output sample
    pos: f64,  // fractional read position into `hist` (relative to its start)
    taps: Vec<f32>,
    fir_hist: Vec<f32>, // last taps.len()-1 raw input samples
    hist: Vec<f32>,     // filtered samples not yet consumed
    passthrough: bool,
}

impl Resampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        let passthrough = in_rate == out_rate;
        let step = in_rate as f64 / out_rate as f64;
        // Cutoff a bit below the output Nyquist frequency.
        let fc = (0.45 * out_rate as f64 / in_rate as f64).min(0.5);
        let n = 63usize;
        let m = (n - 1) as f64 / 2.0;
        let mut taps: Vec<f32> = (0..n)
            .map(|i| {
                let x = i as f64 - m;
                let sinc = if x == 0.0 {
                    2.0 * fc
                } else {
                    (2.0 * std::f64::consts::PI * fc * x).sin() / (std::f64::consts::PI * x)
                };
                let w = 0.42 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos()
                    + 0.08 * (4.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos(); // Blackman
                (sinc * w) as f32
            })
            .collect();
        let sum: f32 = taps.iter().sum();
        taps.iter_mut().for_each(|t| *t /= sum);
        Self {
            step,
            pos: 0.0,
            fir_hist: vec![0.0; n - 1],
            taps,
            hist: Vec::new(),
            passthrough,
        }
    }

    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.passthrough {
            out.extend_from_slice(input);
            return;
        }
        // Low-pass filter.
        let n = self.taps.len();
        let mut x = std::mem::take(&mut self.fir_hist);
        x.extend_from_slice(input);
        for i in 0..input.len() {
            let w = &x[i..i + n];
            let mut acc = 0.0f32;
            for k in 0..n {
                acc += w[k] * self.taps[k];
            }
            self.hist.push(acc);
        }
        self.fir_hist = x[x.len() - (n - 1)..].to_vec();

        // Linear interpolation at `step`.
        while self.pos + 1.0 < self.hist.len() as f64 {
            let i = self.pos as usize;
            let f = (self.pos - i as f64) as f32;
            out.push(self.hist[i] * (1.0 - f) + self.hist[i + 1] * f);
            self.pos += self.step;
        }
        let consumed = (self.pos as usize).min(self.hist.len());
        self.hist.drain(..consumed);
        self.pos -= consumed as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_and_tone_preserved() {
        let mut r = Resampler::new(48_000, 16_000);
        let mut out = Vec::new();
        // 1 s of 440 Hz in 10 ms chunks.
        let tone: Vec<f32> = (0..48_000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48_000.0).sin())
            .collect();
        for c in tone.chunks(480) {
            r.process(c, &mut out);
        }
        assert!((out.len() as i64 - 16_000).abs() < 40, "len {}", out.len());
        let rms =
            (out[1000..].iter().map(|v| v * v).sum::<f32>() / (out.len() - 1000) as f32).sqrt();
        assert!((rms - 0.707).abs() < 0.03, "rms {rms}");
    }

    #[test]
    fn removes_content_above_nyquist() {
        let mut r = Resampler::new(48_000, 16_000);
        let mut out = Vec::new();
        let tone: Vec<f32> = (0..48_000)
            .map(|i| (2.0 * std::f32::consts::PI * 12_000.0 * i as f32 / 48_000.0).sin())
            .collect();
        r.process(&tone, &mut out);
        let rms =
            (out[1000..].iter().map(|v| v * v).sum::<f32>() / (out.len() - 1000) as f32).sqrt();
        assert!(rms < 0.02, "aliased rms {rms}");
    }
}
