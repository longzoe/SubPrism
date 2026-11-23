use std::f32::consts::PI;

pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Streaming anti-aliased windowed-sinc resampler that converts arbitrary sample rate
/// interleaved multi-channel f32 audio into 16 kHz mono f32 audio with near-zero latency.
pub struct MonoResampler16k {
    input_rate: u32,
    channels: usize,
    ratio: f64,
    pos: f64,
    history: Vec<f32>,
    half_taps: usize,
}

impl MonoResampler16k {
    pub fn new(input_rate: u32, channels: usize) -> Self {
        let channels = channels.max(1);
        let ratio = input_rate as f64 / TARGET_SAMPLE_RATE as f64;
        Self {
            input_rate,
            channels,
            ratio,
            pos: 0.0,
            history: Vec::with_capacity(4096),
            half_taps: 8,
        }
    }

    /// Mixes interleaved multi-channel samples to mono, returns `(resampled_16k, rms)`.
    pub fn process_interleaved(&mut self, interleaved: &[f32]) -> (Vec<f32>, f32) {
        if interleaved.is_empty() {
            return (Vec::new(), 0.0);
        }

        let frames = interleaved.len() / self.channels;
        if frames == 0 {
            return (Vec::new(), 0.0);
        }

        let mut sum_sq = 0.0f64;
        let inv_ch = 1.0 / self.channels as f32;

        if self.input_rate == TARGET_SAMPLE_RATE {
            let mut out = Vec::with_capacity(frames);
            for f in 0..frames {
                let base = f * self.channels;
                let mut acc = 0.0f32;
                for c in 0..self.channels {
                    acc += interleaved[base + c];
                }
                let s = (acc * inv_ch).clamp(-1.0, 1.0);
                sum_sq += (s as f64) * (s as f64);
                out.push(s);
            }
            let rms = (sum_sq / frames as f64).sqrt() as f32;
            return (out, rms);
        }

        self.history.reserve(frames);
        for f in 0..frames {
            let base = f * self.channels;
            let mut acc = 0.0f32;
            for c in 0..self.channels {
                acc += interleaved[base + c];
            }
            let s = (acc * inv_ch).clamp(-1.0, 1.0);
            sum_sq += (s as f64) * (s as f64);
            self.history.push(s);
        }
        let rms = (sum_sq / frames as f64).sqrt() as f32;

        let cutoff = (TARGET_SAMPLE_RATE as f32 / self.input_rate as f32).min(1.0) * 0.92;
        let half = self.half_taps as isize;
        let len = self.history.len() as isize;

        if len <= half * 2 + 1 {
            return (Vec::new(), rms);
        }

        if self.pos < half as f64 {
            self.pos = half as f64;
        }

        let limit = (len - half - 1) as f64;
        let estimated = ((limit - self.pos) / self.ratio).max(0.0) as usize + 2;
        let mut out = Vec::with_capacity(estimated);

        while self.pos < limit {
            let center = self.pos.floor() as isize;
            let frac = (self.pos - center as f64) as f32;

            let mut acc = 0.0f32;
            let mut norm = 0.0f32;
            for k in -half..=half {
                let idx = (center + k) as usize;
                let x = k as f32 - frac;
                let w = windowed_sinc(x, cutoff, self.half_taps as f32);
                acc += self.history[idx] * w;
                norm += w;
            }
            let sample = if norm.abs() > 1e-6 { acc / norm } else { 0.0 };
            out.push(sample.clamp(-1.0, 1.0));
            self.pos += self.ratio;
        }

        // Discard consumed prefix while preserving tap margin
        let consume = (self.pos.floor() as usize).saturating_sub(self.half_taps);
        if consume > 0 && consume <= self.history.len() {
            self.history.drain(0..consume);
            self.pos -= consume as f64;
        }

        (out, rms)
    }
}

#[inline]
fn windowed_sinc(x: f32, cutoff: f32, half_taps: f32) -> f32 {
    let ax = x.abs();
    if ax >= half_taps {
        return 0.0;
    }
    let sinc = if ax < 1e-6 {
        cutoff
    } else {
        let px = PI * x;
        (px * cutoff).sin() / px
    };
    // Hann / Blackman hybrid window for strong stopband attenuation
    let u = (x + half_taps) / (2.0 * half_taps);
    let win = 0.42 - 0.5 * (2.0 * PI * u).cos() + 0.08 * (4.0 * PI * u).cos();
    sinc * win
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resamples_48k_stereo_to_16k_mono() {
        let mut rs = MonoResampler16k::new(48_000, 2);
        // 4800 stereo frames = 100ms at 48kHz -> ~1600 mono frames at 16kHz
        let mut input = Vec::with_capacity(4800 * 2);
        for i in 0..4800 {
            let v = (2.0 * PI * 440.0 * i as f32 / 48_000.0).sin() * 0.5;
            input.push(v);
            input.push(v);
        }
        let (out, rms) = rs.process_interleaved(&input);
        assert!(rms > 0.25 && rms < 0.45);
        assert!(out.len() > 1550 && out.len() <= 1605);
    }
}
