pub mod gain;
pub mod sherpa_ffi;

use anyhow::{anyhow, Result};
use gain::{normalize_segment_for_asr, StreamingAgc};
use sherpa_ffi::{OpaqueRecognizer, OpaqueVad, SherpaApi};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info};
use subprism_core::{AsrConfig, ModelPaths};
use uuid::Uuid;

const VAD_WINDOW_SAMPLES: usize = 512; // 32ms @ 16kHz
const SAMPLE_RATE: usize = 16_000;

#[derive(Debug, Clone)]
pub struct AsrOutput {
    pub text: String,
    pub lang: String,
    pub latency_ms: u64,
}

#[derive(Debug, Clone)]
pub enum TranscriptEvent {
    /// Live partial recognition while the speaker is still talking
    Partial {
        utterance_id: String,
        text: String,
        lang: String,
        asr_ms: u64,
    },
    /// Completed sentence bounded by Silero VAD pause or max utterance length
    Final {
        utterance_id: String,
        text: String,
        lang: String,
        asr_ms: u64,
    },
}

/// Real-time streaming VAD + SenseVoice-Small speech recognition engine.
/// Runs entirely on CPU ONNX via dynamic `sherpa-onnx-c-api` so GPU VRAM stays 100% free for Spark-X2.5-4B.
pub struct AsrEngine {
    api: Arc<SherpaApi>,
    recognizer: *const OpaqueRecognizer,
    vad: *const OpaqueVad,
    agc: StreamingAgc,
    soft_speech_boost: bool,
    agc_max_gain_db: f32,
    vad_pending: Vec<f32>,
    live_speech: Vec<f32>,
    current_utterance_id: String,
    last_partial_at: Instant,
    last_partial_samples: usize,
    last_partial_text: String,
    partial_interval: Duration,
    max_live_samples: usize,
}

// Handles are owned exclusively by the dedicated ASR worker thread (moved once on spawn).
unsafe impl Send for AsrEngine {}

impl AsrEngine {
    pub fn load(paths: &ModelPaths, asr_cfg: &AsrConfig) -> Result<Self> {
        let lib_path = Path::new(&paths.sherpa_lib_path);
        let vad_path = Path::new(&paths.vad_model_path);
        let asr_dir = Path::new(&paths.asr_model_dir);

        let model_onnx = resolve_sensevoice_onnx(asr_dir)?;
        let tokens_txt = asr_dir.join("tokens.txt");
        if !tokens_txt.is_file() {
            return Err(anyhow!(
                "SenseVoice 模型目录缺少 tokens.txt: {}",
                asr_dir.display()
            ));
        }

        let api = Arc::new(SherpaApi::load(lib_path)?);
        let recognizer = api.create_sense_voice_recognizer(
            &model_onnx,
            &tokens_txt,
            &asr_cfg.source_language,
            asr_cfg.num_threads,
        )?;
        let vad = api.create_silero_vad(
            vad_path,
            asr_cfg.vad_threshold,
            asr_cfg.min_silence_ms as f32 / 1000.0,
            asr_cfg.max_utterance_ms as f32 / 1000.0,
            2,
        )?;

        info!(
            "本地 ASR 引擎已就绪: SenseVoice={}, SileroVAD={}",
            model_onnx.display(),
            vad_path.display()
        );

        Ok(Self {
            api,
            recognizer,
            vad,
            agc: StreamingAgc::new(asr_cfg.soft_speech_boost, asr_cfg.agc_max_gain_db),
            soft_speech_boost: asr_cfg.soft_speech_boost,
            agc_max_gain_db: asr_cfg.agc_max_gain_db,
            vad_pending: Vec::with_capacity(2048),
            live_speech: Vec::with_capacity(SAMPLE_RATE * 10),
            current_utterance_id: Uuid::new_v4().to_string(),
            last_partial_at: Instant::now(),
            last_partial_samples: 0,
            last_partial_text: String::new(),
            partial_interval: Duration::from_millis(asr_cfg.partial_interval_ms.max(250) as u64),
            max_live_samples: SAMPLE_RATE * (asr_cfg.max_utterance_ms.max(2500) as usize) / 1000,
        })
    }

    /// Direct offline transcription of a 16 kHz mono f32 buffer (useful for WAV testing & batch decoding).
    pub fn transcribe_samples(&self, samples_16k: &[f32]) -> Result<AsrOutput> {
        let start = Instant::now();
        let leveled =
            normalize_segment_for_asr(samples_16k, self.soft_speech_boost, self.agc_max_gain_db);
        let (raw_text, raw_lang) = self.api.decode_offline(self.recognizer, &leveled)?;
        let latency_ms = start.elapsed().as_millis() as u64;
        let text = sanitize_transcript(&raw_text);
        let lang = normalize_lang(&raw_lang);
        Ok(AsrOutput {
            text,
            lang,
            latency_ms,
        })
    }

    /// Feeds incremental 16 kHz mono audio frames into Silero VAD + SenseVoice,
    /// returning `(speaking_now, emitted_events)`.
    pub fn push_audio(&mut self, samples_16k: &[f32]) -> Result<(bool, Vec<TranscriptEvent>)> {
        self.vad_pending.extend_from_slice(samples_16k);
        let mut events = Vec::new();
        let mut speaking_now = false;

        while self.vad_pending.len() >= VAD_WINDOW_SAMPLES {
            let mut window: Vec<f32> = self.vad_pending.drain(0..VAD_WINDOW_SAMPLES).collect();
            self.agc.process_frame(&mut window);
            self.api.vad_accept_waveform(self.vad, &window);
            speaking_now = self.api.vad_is_detected(self.vad);

            if speaking_now {
                if self.live_speech.len() + window.len() <= self.max_live_samples {
                    self.live_speech.extend_from_slice(&window);
                }
            }

            // Drain any speech segments finalized by Silero VAD
            while let Some(segment) = self.api.vad_pop_segment(self.vad) {
                let out = self.transcribe_samples(&segment)?;
                let uid = std::mem::replace(
                    &mut self.current_utterance_id,
                    Uuid::new_v4().to_string(),
                );
                self.live_speech.clear();
                self.last_partial_samples = 0;
                self.last_partial_text.clear();

                if !out.text.is_empty() {
                    debug!(
                        "ASR Final [{} | {}ms | {}]: {}",
                        uid, out.latency_ms, out.lang, out.text
                    );
                    events.push(TranscriptEvent::Final {
                        utterance_id: uid,
                        text: out.text,
                        lang: out.lang,
                        asr_ms: out.latency_ms,
                    });
                }
            }
        }

        // Emit low-latency Partial subtitle while speaker is mid-sentence
        if speaking_now
            && self.live_speech.len() >= (SAMPLE_RATE * 55) / 100 // >= 0.55s speech
            && self.live_speech.len() > self.last_partial_samples + (SAMPLE_RATE / 4)
            && self.last_partial_at.elapsed() >= self.partial_interval
        {
            self.last_partial_at = Instant::now();
            self.last_partial_samples = self.live_speech.len();
            let out = self.transcribe_samples(&self.live_speech)?;
            if !out.text.is_empty() && out.text != self.last_partial_text {
                self.last_partial_text = out.text.clone();
                events.push(TranscriptEvent::Partial {
                    utterance_id: self.current_utterance_id.clone(),
                    text: out.text,
                    lang: out.lang,
                    asr_ms: out.latency_ms,
                });
            }
        }

        Ok((speaking_now, events))
    }

    /// Flushes any trailing audio held inside Silero VAD (e.g., on pause/stop or EOF of WAV).
    pub fn flush(&mut self) -> Result<Vec<TranscriptEvent>> {
        self.api.vad_flush(self.vad);
        let mut events = Vec::new();
        while let Some(segment) = self.api.vad_pop_segment(self.vad) {
            let out = self.transcribe_samples(&segment)?;
            let uid = std::mem::replace(
                &mut self.current_utterance_id,
                Uuid::new_v4().to_string(),
            );
            if !out.text.is_empty() {
                events.push(TranscriptEvent::Final {
                    utterance_id: uid,
                    text: out.text,
                    lang: out.lang,
                    asr_ms: out.latency_ms,
                });
            }
        }
        self.live_speech.clear();
        self.last_partial_samples = 0;
        self.last_partial_text.clear();
        self.vad_pending.clear();
        Ok(events)
    }

    pub fn reset(&mut self) {
        self.api.vad_reset(self.vad);
        self.vad_pending.clear();
        self.live_speech.clear();
        self.last_partial_samples = 0;
        self.last_partial_text.clear();
        self.current_utterance_id = Uuid::new_v4().to_string();
    }
}

impl Drop for AsrEngine {
    fn drop(&mut self) {
        self.api.destroy_vad(self.vad);
        self.api.destroy_recognizer(self.recognizer);
    }
}

fn resolve_sensevoice_onnx(dir: &Path) -> Result<PathBuf> {
    for name in ["model.int8.onnx", "model.onnx"] {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(anyhow!(
        "在 {} 中未找到 model.int8.onnx 或 model.onnx",
        dir.display()
    ))
}

fn normalize_lang(raw: &str) -> String {
    let cleaned = raw
        .trim()
        .trim_matches(|c| c == '<' || c == '>' || c == '|')
        .to_lowercase();
    if cleaned.is_empty() {
        "auto".to_string()
    } else {
        cleaned
    }
}

/// Strips any residual SenseVoice special tokens (e.g. `<|en|>`, `<|NEUTRAL|>`) and trims noise.
fn sanitize_transcript(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' && chars.peek() == Some(&'|') {
            while let Some(nc) = chars.next() {
                if nc == '>' {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }

    let trimmed = out.trim();
    // Filter pure punctuation / noise hallucinations
    let has_content = trimmed
        .chars()
        .any(|c| c.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(&c));
    if !has_content {
        return String::new();
    }
    trimmed.to_string()
}
