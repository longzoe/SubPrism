/// Streaming Automatic Gain Control (AGC) + soft limiter tuned for cinema / anime audio.
/// Smoothly lifts quiet whispered dialogue into Silero VAD's optimal sensitivity range
/// without amplifying pure digital silence or clipping loud BGM/SFX peaks.
pub struct StreamingAgc {
    enabled: bool,
    target_rms: f32,
    noise_gate_rms: f32,
    max_gain: f32,
    current_gain: f32,
}

impl StreamingAgc {
    pub fn new(enabled: bool, max_gain_db: f32) -> Self {
        let clamped_db = max_gain_db.clamp(0.0, 26.0);
        let max_gain = 10.0_f32.powf(clamped_db / 20.0);
        Self {
            enabled,
            target_rms: 0.085,
            noise_gate_rms: 0.0007, // ~ -63 dBFS: keep pure digital silence untouched
            max_gain,
            current_gain: 1.0,
        }
    }

    /// Processes a 32ms frame (512 samples @ 16 kHz) in-place.
    pub fn process_frame(&mut self, frame: &mut [f32]) {
        if !self.enabled || self.max_gain <= 1.01 || frame.is_empty() {
            return;
        }

        let sum_sq: f32 = frame.iter().map(|s| s * s).sum();
        let rms = (sum_sq / frame.len() as f32).sqrt();

        let desired_gain = if rms <= self.noise_gate_rms {
            // Slowly relax toward 1.0 during dead silence so we don't amplify noise floor
            1.0
        } else {
            (self.target_rms / rms).clamp(0.65, self.max_gain)
        };

        // Fast attack (protect against sudden loud SFX/shouts), gentle release (lift whispers smoothly)
        let alpha = if desired_gain < self.current_gain {
            0.42
        } else {
            0.12
        };
        let prev_gain = self.current_gain;
        self.current_gain = prev_gain + alpha * (desired_gain - prev_gain);

        // Linearly interpolate gain across the frame to avoid click artifacts
        let inv_n = 1.0 / frame.len() as f32;
        for (i, sample) in frame.iter_mut().enumerate() {
            let g = prev_gain + (self.current_gain - prev_gain) * (i as f32 * inv_n);
            *sample = soft_limit(*sample * g);
        }
    }
}

/// Normalizes a VAD-extracted speech segment before feeding into SenseVoice-Small
/// so very quiet whispers still yield crisp CTC posteriors.
pub fn normalize_segment_for_asr(samples: &[f32], enabled: bool, max_gain_db: f32) -> Vec<f32> {
    if !enabled || samples.is_empty() {
        return samples.to_vec();
    }

    let peak = samples
        .iter()
        .fold(0.0_f32, |acc, &s| acc.max(s.abs()));

    // Ignore near-zero silence; only boost genuinely quiet segments
    if peak < 0.0025 || peak >= 0.55 {
        return samples.to_vec();
    }

    let max_gain = 10.0_f32.powf(max_gain_db.clamp(0.0, 26.0) / 20.0);
    let gain = (0.70 / peak).min(max_gain);
    samples.iter().map(|&s| soft_limit(s * gain)).collect()
}

#[inline]
fn soft_limit(x: f32) -> f32 {
    if x.abs() <= 0.85 {
        x
    } else {
        // Smooth tanh knee between 0.85 and 1.0
        x.signum() * (0.85 + 0.15 * ((x.abs() - 0.85) / 0.15).tanh())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agc_lifts_whispered_signal_without_amplifying_silence() {
        let mut agc = StreamingAgc::new(true, 18.0);

        // Digital silence stays strictly zero
        let mut silence = vec![0.0_f32; 512];
        agc.process_frame(&mut silence);
        assert!(silence.iter().all(|&s| s == 0.0));

        // Soft -36 dB whisper (~0.015 peak) gets boosted smoothly over a few frames
        let mut final_rms = 0.0;
        for _ in 0..15 {
            let mut whisper: Vec<f32> = (0..512)
                .map(|i| (i as f32 * 0.12).sin() * 0.015)
                .collect();
            agc.process_frame(&mut whisper);
            let sum_sq: f32 = whisper.iter().map(|s| s * s).sum();
            final_rms = (sum_sq / whisper.len() as f32).sqrt();
        }
        assert!(
            final_rms > 0.045,
            "Expected boosted whisper RMS > 0.045, got {final_rms}"
        );
    }

    #[test]
    fn segment_normalizer_boosts_quiet_speech_safely() {
        let quiet: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.1).sin() * 0.02).collect();
        let normed = normalize_segment_for_asr(&quiet, true, 18.0);
        let peak = normed.iter().fold(0.0_f32, |a, &b| a.max(b.abs()));
        assert!(peak > 0.12 && peak <= 0.99, "Unexpected peak: {peak}");
    }
}
