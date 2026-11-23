pub mod config;
pub mod hardware;
pub mod prompt;
pub mod types;

pub use config::{
    AppearanceConfig, AsrConfig, GlassTheme, GlossaryEntry, LlmConfig, ModelPaths, StylePreset,
    SubPrismConfig,
};
pub use hardware::{detect_hardware, GpuBackend, HardwareInfo};
pub use prompt::{build_system_prompt, build_user_prompt, ContextTurn};
pub use types::{
    AudioSourceInfo, EnginePhase, EngineStatus, PipelineMetrics, SubtitleEntry, SubtitleEvent,
    SubtitleStatus,
};
