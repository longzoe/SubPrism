use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StylePreset {
    #[default]
    General,
    Tech,
    Cinema,
    Anime,
}

impl StylePreset {
    pub fn description(self) -> &'static str {
        match self {
            StylePreset::General => "自然流畅、通俗易懂的日常视频字幕风格",
            StylePreset::Tech => {
                "严谨专业，保留常见英文技术缩写与代码术语（如 API、Rust、CUDA、KV Cache）"
            }
            StylePreset::Cinema => "贴合影视人物情绪与口吻，富有画面感与自然网感",
            StylePreset::Anime => "贴合二次元与 ACG 圈层常用译法，保留角色语气与称谓",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GlassTheme {
    #[default]
    SmokedDark,
    CrystalLight,
    MinimalText,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryEntry {
    pub source: String,
    pub target: String,
}

/// User-configurable local runtime and model paths.
/// In production releases, users download models/binaries and pick their paths in Settings.
/// During local development, empty fields can auto-discover from `local_dev_assets/`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelPaths {
    /// Path to `llama-server` executable (`llama-server.exe` on Windows, `llama-server` on macOS)
    pub llama_server_bin: String,
    /// Path to `Spark-X2.5-4B-*.gguf` model weights
    pub llm_model_gguf: String,
    /// Path to `sherpa-onnx` C-API shared library (`sherpa-onnx-c-api.dll` / `libsherpa-onnx-c-api.dylib`)
    pub sherpa_lib_path: String,
    /// Path to SenseVoice ONNX model directory (containing `model.int8.onnx` or `model.onnx` and `tokens.txt`)
    pub asr_model_dir: String,
    /// Path to `silero_vad_v5.onnx` (or `silero_vad.onnx`)
    pub vad_model_path: String,
}

impl ModelPaths {
    pub fn validate(&self) -> Vec<String> {
        let mut missing = Vec::new();
        if self.llama_server_bin.trim().is_empty() || !Path::new(&self.llama_server_bin).is_file() {
            missing.push("llama-server 可执行文件未配置或不存在".to_string());
        }
        if self.llm_model_gguf.trim().is_empty() || !Path::new(&self.llm_model_gguf).is_file() {
            missing.push("Spark-X2.5-4B GGUF 模型未配置或不存在".to_string());
        }
        if self.sherpa_lib_path.trim().is_empty() || !Path::new(&self.sherpa_lib_path).is_file() {
            missing.push("sherpa-onnx C-API 动态库未配置或不存在".to_string());
        }
        if self.asr_model_dir.trim().is_empty() || !Path::new(&self.asr_model_dir).is_dir() {
            missing.push("SenseVoice 本地语音识别模型目录未配置或不存在".to_string());
        }
        if self.vad_model_path.trim().is_empty() || !Path::new(&self.vad_model_path).is_file() {
            missing.push("Silero VAD 模型未配置或不存在".to_string());
        }
        missing
    }

    pub fn is_complete(&self) -> bool {
        self.validate().is_empty()
    }

    /// Fills empty path fields if a local dev assets directory exists (never bundled in releases).
    pub fn fill_from_dev_assets_if_present(&mut self) -> bool {
        let Some(root) = find_dev_assets_root() else {
            return false;
        };
        let mut changed = false;

        if self.llama_server_bin.trim().is_empty() || !Path::new(&self.llama_server_bin).is_file() {
            let candidate = if cfg!(target_os = "windows") {
                root.join("llama-server").join("llama-server.exe")
            } else {
                root.join("llama-server").join("llama-server")
            };
            if candidate.is_file() {
                self.llama_server_bin = candidate.to_string_lossy().into_owned();
                changed = true;
            }
        }

        if self.llm_model_gguf.trim().is_empty() || !Path::new(&self.llm_model_gguf).is_file() {
            if let Some(gguf) = find_first_with_ext(&root.join("models"), "gguf") {
                self.llm_model_gguf = gguf.to_string_lossy().into_owned();
                changed = true;
            }
        }

        if self.sherpa_lib_path.trim().is_empty() || !Path::new(&self.sherpa_lib_path).is_file() {
            let candidates = if cfg!(target_os = "windows") {
                vec![
                    root.join("asr").join("runtime").join("lib").join("sherpa-onnx-c-api.dll"),
                    root.join("asr").join("runtime").join("bin").join("sherpa-onnx-c-api.dll"),
                ]
            } else {
                vec![
                    root.join("asr").join("runtime").join("lib").join("libsherpa-onnx-c-api.dylib"),
                ]
            };
            for c in candidates {
                if c.is_file() {
                    self.sherpa_lib_path = c.to_string_lossy().into_owned();
                    changed = true;
                    break;
                }
            }
        }

        if self.asr_model_dir.trim().is_empty() || !Path::new(&self.asr_model_dir).is_dir() {
            let models_dir = root.join("asr").join("models");
            if let Ok(entries) = fs::read_dir(&models_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.join("tokens.txt").is_file() {
                        self.asr_model_dir = path.to_string_lossy().into_owned();
                        changed = true;
                        break;
                    }
                }
            }
        }

        if self.vad_model_path.trim().is_empty() || !Path::new(&self.vad_model_path).is_file() {
            for name in ["silero_vad_v5.onnx", "silero_vad.onnx"] {
                let candidate = root.join("asr").join("models").join(name);
                if candidate.is_file() {
                    self.vad_model_path = candidate.to_string_lossy().into_owned();
                    changed = true;
                    break;
                }
            }
        }

        changed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    pub gpu_layers: u32,
    pub context_size: u32,
    pub parallel_slots: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub repeat_penalty: f32,
    pub context_turns: usize,
    pub translate_partials: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            gpu_layers: 99,
            context_size: 4096,
            parallel_slots: 2,
            temperature: 0.25,
            top_p: 0.9,
            repeat_penalty: 1.05,
            context_turns: 3,
            translate_partials: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrConfig {
    /// "auto" | "zh" | "en" | "ja" | "ko" | "yue"
    pub source_language: String,
    pub num_threads: i32,
    pub vad_threshold: f32,
    pub min_silence_ms: u32,
    pub max_utterance_ms: u32,
    pub partial_interval_ms: u32,
    /// Automatically boosts whispered / low-volume dialogue before VAD & ASR
    #[serde(default = "default_true")]
    pub soft_speech_boost: bool,
    /// Maximum automatic gain in dB (0..=24 dB) for quiet characters
    #[serde(default = "default_agc_max_gain_db")]
    pub agc_max_gain_db: f32,
}

fn default_true() -> bool {
    true
}

fn default_agc_max_gain_db() -> f32 {
    18.0
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            source_language: "auto".to_string(),
            num_threads: 4,
            vad_threshold: 0.26,
            min_silence_ms: 460,
            max_utterance_ms: 7500,
            partial_interval_ms: 500,
            soft_speech_boost: true,
            agc_max_gain_db: 18.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceConfig {
    pub theme: GlassTheme,
    pub glass_opacity: f32,
    pub translated_font_size: u32,
    pub source_font_size: u32,
    pub show_source: bool,
    pub source_on_top: bool,
    pub max_visible_lines: usize,
    pub click_through: bool,
    pub overlay_width: f64,
    pub overlay_height: f64,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: GlassTheme::SmokedDark,
            glass_opacity: 0.44,
            translated_font_size: 23,
            source_font_size: 13,
            show_source: true,
            source_on_top: true,
            max_visible_lines: 2,
            click_through: false,
            overlay_width: 920.0,
            overlay_height: 124.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubPrismConfig {
    pub target_language: String,
    pub style_preset: StylePreset,
    pub audio_source_id: String,
    pub glossary: Vec<GlossaryEntry>,
    pub paths: ModelPaths,
    pub llm: LlmConfig,
    pub asr: AsrConfig,
    pub appearance: AppearanceConfig,
}

impl Default for SubPrismConfig {
    fn default() -> Self {
        let mut paths = ModelPaths::default();
        paths.fill_from_dev_assets_if_present();
        Self {
            target_language: "简体中文".to_string(),
            style_preset: StylePreset::General,
            audio_source_id: "system".to_string(),
            glossary: Vec::new(),
            paths,
            llm: LlmConfig::default(),
            asr: AsrConfig::default(),
            appearance: AppearanceConfig::default(),
        }
    }
}

impl SubPrismConfig {
    pub fn config_file_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("app", "subprism", "SubPrism")
            .context("无法定位系统配置目录")?;
        let dir = dirs.config_dir();
        fs::create_dir_all(dir)?;
        Ok(dir.join("config.json"))
    }

    pub fn load_or_default() -> Self {
        let Ok(path) = Self::config_file_path() else {
            return Self::default();
        };

        let mut cfg = if path.is_file() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str::<SubPrismConfig>(&s).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        };

        // If any path is still empty and dev assets exist locally, auto-fill for dev convenience.
        if cfg.paths.fill_from_dev_assets_if_present() {
            let _ = cfg.save();
        }
        cfg
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_file_path()?;
        let data = serde_json::to_string_pretty(self)?;
        fs::write(path, data)?;
        Ok(())
    }
}

fn find_dev_assets_root() -> Option<PathBuf> {
    if let Ok(env_dir) = std::env::var("SUBPRISM_DEV_ASSETS_DIR") {
        let p = PathBuf::from(env_dir);
        if p.is_dir() {
            return Some(p);
        }
    }

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("local_dev_assets"));
        if let Some(parent) = cwd.parent() {
            candidates.push(parent.join("local_dev_assets"));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent();
        for _ in 0..5 {
            if let Some(dir) = cur {
                candidates.push(dir.join("local_dev_assets"));
                cur = dir.parent();
            }
        }
    }

    candidates.into_iter().find(|p| p.is_dir())
}

fn find_first_with_ext(dir: &Path, ext: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file()
            && p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        {
            return Some(p);
        }
    }
    None
}
