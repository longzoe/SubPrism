use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleStatus {
    /// Live speech still in progress (interim ASR / draft translation)
    Partial,
    /// Utterance finalized by VAD; Spark-X2.5-4B is streaming tokens
    Translating,
    /// Translation complete
    Final,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleEntry {
    pub id: String,
    pub timestamp_ms: u64,
    pub source_lang: String,
    pub source_text: String,
    pub translated_text: String,
    pub status: SubtitleStatus,
    pub asr_latency_ms: Option<u64>,
    pub ttft_ms: Option<u64>,
    pub translate_latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SubtitleEvent {
    Upsert { entry: SubtitleEntry },
    AudioLevel { rms: f32, speaking: bool },
    StatusChanged { status: EngineStatus },
    Metrics { metrics: PipelineMetrics },
    Error { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnginePhase {
    Idle,
    NeedsSetup,
    StartingLlm,
    LoadingAsr,
    Listening,
    Paused,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub phase: EnginePhase,
    pub message: String,
    pub llm_ready: bool,
    pub asr_ready: bool,
    pub audio_active: bool,
    pub llama_endpoint: Option<String>,
    pub llama_version: Option<String>,
}

impl Default for EngineStatus {
    fn default() -> Self {
        Self {
            phase: EnginePhase::Idle,
            message: "就绪".to_string(),
            llm_ready: false,
            asr_ready: false,
            audio_active: false,
            llama_endpoint: None,
            llama_version: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineMetrics {
    pub last_asr_ms: u64,
    pub last_ttft_ms: u64,
    pub last_translate_ms: u64,
    pub tokens_per_sec: f32,
    pub total_utterances: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSourceInfo {
    pub id: String,
    pub name: String,
    pub kind: String, // "system" | "process"
    pub pid: Option<u32>,
}
