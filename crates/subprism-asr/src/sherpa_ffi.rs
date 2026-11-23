#![allow(non_camel_case_types, dead_code)]

use anyhow::{anyhow, Context, Result};
use libloading::Library;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::Path;
use std::ptr;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxFeatureConfig {
    pub sample_rate: i32,
    pub feature_dim: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineTransducerModelConfig {
    pub encoder: *const c_char,
    pub decoder: *const c_char,
    pub joiner: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineParaformerModelConfig {
    pub model: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineNemoEncDecCtcModelConfig {
    pub model: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineWhisperModelConfig {
    pub encoder: *const c_char,
    pub decoder: *const c_char,
    pub language: *const c_char,
    pub task: *const c_char,
    pub tail_paddings: i32,
    pub enable_token_timestamps: i32,
    pub enable_segment_timestamps: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineTdnnModelConfig {
    pub model: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineSenseVoiceModelConfig {
    pub model: *const c_char,
    pub language: *const c_char,
    pub use_itn: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineMoonshineModelConfig {
    pub preprocessor: *const c_char,
    pub encoder: *const c_char,
    pub uncached_decoder: *const c_char,
    pub cached_decoder: *const c_char,
    pub merged_decoder: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineFireRedAsrModelConfig {
    pub encoder: *const c_char,
    pub decoder: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineDolphinModelConfig {
    pub model: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineZipformerCtcModelConfig {
    pub model: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineCanaryModelConfig {
    pub encoder: *const c_char,
    pub decoder: *const c_char,
    pub src_lang: *const c_char,
    pub tgt_lang: *const c_char,
    pub use_pnc: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineWenetCtcModelConfig {
    pub model: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineOmnilingualAsrCtcModelConfig {
    pub model: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineMedAsrCtcModelConfig {
    pub model: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineFunASRNanoModelConfig {
    pub encoder_adaptor: *const c_char,
    pub llm: *const c_char,
    pub embedding: *const c_char,
    pub tokenizer: *const c_char,
    pub system_prompt: *const c_char,
    pub user_prompt: *const c_char,
    pub max_new_tokens: i32,
    pub temperature: f32,
    pub top_p: f32,
    pub seed: i32,
    pub language: *const c_char,
    pub itn: i32,
    pub hotwords: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineFireRedAsrCtcModelConfig {
    pub model: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineQwen3ASRModelConfig {
    pub conv_frontend: *const c_char,
    pub encoder: *const c_char,
    pub decoder: *const c_char,
    pub tokenizer: *const c_char,
    pub max_total_len: i32,
    pub max_new_tokens: i32,
    pub temperature: f32,
    pub top_p: f32,
    pub seed: i32,
    pub hotwords: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineCohereTranscribeModelConfig {
    pub encoder: *const c_char,
    pub decoder: *const c_char,
    pub language: *const c_char,
    pub use_punct: i32,
    pub use_itn: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineModelConfig {
    pub transducer: SherpaOnnxOfflineTransducerModelConfig,
    pub paraformer: SherpaOnnxOfflineParaformerModelConfig,
    pub nemo_ctc: SherpaOnnxOfflineNemoEncDecCtcModelConfig,
    pub whisper: SherpaOnnxOfflineWhisperModelConfig,
    pub tdnn: SherpaOnnxOfflineTdnnModelConfig,
    pub tokens: *const c_char,
    pub num_threads: i32,
    pub debug: i32,
    pub provider: *const c_char,
    pub model_type: *const c_char,
    pub modeling_unit: *const c_char,
    pub bpe_vocab: *const c_char,
    pub telespeech_ctc: *const c_char,
    pub sense_voice: SherpaOnnxOfflineSenseVoiceModelConfig,
    pub moonshine: SherpaOnnxOfflineMoonshineModelConfig,
    pub fire_red_asr: SherpaOnnxOfflineFireRedAsrModelConfig,
    pub dolphin: SherpaOnnxOfflineDolphinModelConfig,
    pub zipformer_ctc: SherpaOnnxOfflineZipformerCtcModelConfig,
    pub canary: SherpaOnnxOfflineCanaryModelConfig,
    pub wenet_ctc: SherpaOnnxOfflineWenetCtcModelConfig,
    pub omnilingual: SherpaOnnxOfflineOmnilingualAsrCtcModelConfig,
    pub medasr: SherpaOnnxOfflineMedAsrCtcModelConfig,
    pub funasr_nano: SherpaOnnxOfflineFunASRNanoModelConfig,
    pub fire_red_asr_ctc: SherpaOnnxOfflineFireRedAsrCtcModelConfig,
    pub qwen3_asr: SherpaOnnxOfflineQwen3ASRModelConfig,
    pub cohere_transcribe: SherpaOnnxOfflineCohereTranscribeModelConfig,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineLMConfig {
    pub model: *const c_char,
    pub scale: f32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxHomophoneReplacerConfig {
    pub dict_dir: *const c_char,
    pub lexicon: *const c_char,
    pub rule_fsts: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineCtcFstDecoderConfig {
    pub graph: *const c_char,
    pub max_active: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxOfflineRecognizerConfig {
    pub feat_config: SherpaOnnxFeatureConfig,
    pub model_config: SherpaOnnxOfflineModelConfig,
    pub lm_config: SherpaOnnxOfflineLMConfig,
    pub decoding_method: *const c_char,
    pub max_active_paths: i32,
    pub hotwords_file: *const c_char,
    pub hotwords_score: f32,
    pub rule_fsts: *const c_char,
    pub rule_fars: *const c_char,
    pub blank_penalty: f32,
    pub hr: SherpaOnnxHomophoneReplacerConfig,
    pub ctc_fst_decoder_config: SherpaOnnxOfflineCtcFstDecoderConfig,
}

#[repr(C)]
pub struct SherpaOnnxOfflineRecognizerResult {
    pub text: *const c_char,
    pub timestamps: *mut f32,
    pub count: i32,
    pub tokens: *const c_char,
    pub tokens_arr: *const *const c_char,
    pub json: *const c_char,
    pub lang: *const c_char,
    pub emotion: *const c_char,
    pub event: *const c_char,
    pub durations: *mut f32,
    pub ys_log_probs: *mut f32,
    pub segment_timestamps: *const f32,
    pub segment_durations: *const f32,
    pub segment_texts: *const c_char,
    pub segment_texts_arr: *const *const c_char,
    pub segment_count: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxSileroVadModelConfig {
    pub model: *const c_char,
    pub threshold: f32,
    pub min_silence_duration: f32,
    pub min_speech_duration: f32,
    pub window_size: i32,
    pub max_speech_duration: f32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxTenVadModelConfig {
    pub model: *const c_char,
    pub threshold: f32,
    pub min_silence_duration: f32,
    pub min_speech_duration: f32,
    pub window_size: i32,
    pub max_speech_duration: f32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SherpaOnnxVadModelConfig {
    pub silero_vad: SherpaOnnxSileroVadModelConfig,
    pub sample_rate: i32,
    pub num_threads: i32,
    pub provider: *const c_char,
    pub debug: i32,
    pub ten_vad: SherpaOnnxTenVadModelConfig,
}

#[repr(C)]
pub struct SherpaOnnxSpeechSegment {
    pub start: i64,
    pub samples: *mut f32,
    pub n: i32,
}

pub type OpaqueRecognizer = c_void;
pub type OpaqueStream = c_void;
pub type OpaqueVad = c_void;

type FnCreateOfflineRecognizer =
    unsafe extern "C" fn(*const SherpaOnnxOfflineRecognizerConfig) -> *const OpaqueRecognizer;
type FnDestroyOfflineRecognizer = unsafe extern "C" fn(*const OpaqueRecognizer);
type FnCreateOfflineStream = unsafe extern "C" fn(*const OpaqueRecognizer) -> *const OpaqueStream;
type FnDestroyOfflineStream = unsafe extern "C" fn(*const OpaqueStream);
type FnAcceptWaveformOffline = unsafe extern "C" fn(*const OpaqueStream, i32, *const f32, i32);
type FnDecodeOfflineStream = unsafe extern "C" fn(*const OpaqueRecognizer, *const OpaqueStream);
type FnGetOfflineStreamResult =
    unsafe extern "C" fn(*const OpaqueStream) -> *const SherpaOnnxOfflineRecognizerResult;
type FnDestroyOfflineRecognizerResult =
    unsafe extern "C" fn(*const SherpaOnnxOfflineRecognizerResult);

type FnCreateVad =
    unsafe extern "C" fn(*const SherpaOnnxVadModelConfig, f32) -> *const OpaqueVad;
type FnDestroyVad = unsafe extern "C" fn(*const OpaqueVad);
type FnVadAcceptWaveform = unsafe extern "C" fn(*const OpaqueVad, *const f32, i32);
type FnVadEmpty = unsafe extern "C" fn(*const OpaqueVad) -> i32;
type FnVadDetected = unsafe extern "C" fn(*const OpaqueVad) -> i32;
type FnVadPop = unsafe extern "C" fn(*const OpaqueVad);
type FnVadFront = unsafe extern "C" fn(*const OpaqueVad) -> *const SherpaOnnxSpeechSegment;
type FnDestroySpeechSegment = unsafe extern "C" fn(*const SherpaOnnxSpeechSegment);
type FnVadFlush = unsafe extern "C" fn(*const OpaqueVad);
type FnVadReset = unsafe extern "C" fn(*const OpaqueVad);

pub struct SherpaApi {
    // Keep preloaded dependency libraries alive for the lifetime of the C-API library
    _dep_libs: Vec<Library>,
    _lib: Library,
    create_offline_recognizer: FnCreateOfflineRecognizer,
    destroy_offline_recognizer: FnDestroyOfflineRecognizer,
    create_offline_stream: FnCreateOfflineStream,
    destroy_offline_stream: FnDestroyOfflineStream,
    accept_waveform_offline: FnAcceptWaveformOffline,
    decode_offline_stream: FnDecodeOfflineStream,
    get_offline_stream_result: FnGetOfflineStreamResult,
    destroy_offline_recognizer_result: FnDestroyOfflineRecognizerResult,
    create_vad: FnCreateVad,
    destroy_vad: FnDestroyVad,
    vad_accept_waveform: FnVadAcceptWaveform,
    vad_empty: FnVadEmpty,
    vad_detected: FnVadDetected,
    vad_pop: FnVadPop,
    vad_front: FnVadFront,
    destroy_speech_segment: FnDestroySpeechSegment,
    vad_flush: FnVadFlush,
    vad_reset: FnVadReset,
}

unsafe impl Send for SherpaApi {}
unsafe impl Sync for SherpaApi {}

impl SherpaApi {
    pub fn load(c_api_lib_path: &Path) -> Result<Self> {
        if !c_api_lib_path.is_file() {
            return Err(anyhow!(
                "找不到 sherpa-onnx C-API 动态库: {}",
                c_api_lib_path.display()
            ));
        }

        // Preload sibling onnxruntime shared libraries from the same directory so OS loader
        // resolves imports cleanly regardless of PATH / working directory.
        let mut dep_libs = Vec::new();
        if let Some(dir) = c_api_lib_path.parent() {
            let sibling_names: &[&str] = if cfg!(target_os = "windows") {
                &["onnxruntime_providers_shared.dll", "onnxruntime.dll"]
            } else if cfg!(target_os = "macos") {
                &["libonnxruntime.dylib"]
            } else {
                &["libonnxruntime.so"]
            };
            for name in sibling_names {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    if let Ok(lib) = unsafe { Library::new(&candidate) } {
                        dep_libs.push(lib);
                    }
                }
            }
        }

        let lib = unsafe {
            Library::new(c_api_lib_path).with_context(|| {
                format!(
                    "加载 sherpa-onnx C-API 动态库失败: {}",
                    c_api_lib_path.display()
                )
            })
        }?;

        unsafe {
            let create_offline_recognizer = *lib.get(b"SherpaOnnxCreateOfflineRecognizer\0")?;
            let destroy_offline_recognizer = *lib.get(b"SherpaOnnxDestroyOfflineRecognizer\0")?;
            let create_offline_stream = *lib.get(b"SherpaOnnxCreateOfflineStream\0")?;
            let destroy_offline_stream = *lib.get(b"SherpaOnnxDestroyOfflineStream\0")?;
            let accept_waveform_offline = *lib.get(b"SherpaOnnxAcceptWaveformOffline\0")?;
            let decode_offline_stream = *lib.get(b"SherpaOnnxDecodeOfflineStream\0")?;
            let get_offline_stream_result = *lib.get(b"SherpaOnnxGetOfflineStreamResult\0")?;
            let destroy_offline_recognizer_result =
                *lib.get(b"SherpaOnnxDestroyOfflineRecognizerResult\0")?;

            let create_vad = *lib.get(b"SherpaOnnxCreateVoiceActivityDetector\0")?;
            let destroy_vad = *lib.get(b"SherpaOnnxDestroyVoiceActivityDetector\0")?;
            let vad_accept_waveform =
                *lib.get(b"SherpaOnnxVoiceActivityDetectorAcceptWaveform\0")?;
            let vad_empty = *lib.get(b"SherpaOnnxVoiceActivityDetectorEmpty\0")?;
            let vad_detected = *lib.get(b"SherpaOnnxVoiceActivityDetectorDetected\0")?;
            let vad_pop = *lib.get(b"SherpaOnnxVoiceActivityDetectorPop\0")?;
            let vad_front = *lib.get(b"SherpaOnnxVoiceActivityDetectorFront\0")?;
            let destroy_speech_segment = *lib.get(b"SherpaOnnxDestroySpeechSegment\0")?;
            let vad_flush = *lib.get(b"SherpaOnnxVoiceActivityDetectorFlush\0")?;
            let vad_reset = *lib.get(b"SherpaOnnxVoiceActivityDetectorReset\0")?;

            Ok(Self {
                _dep_libs: dep_libs,
                _lib: lib,
                create_offline_recognizer,
                destroy_offline_recognizer,
                create_offline_stream,
                destroy_offline_stream,
                accept_waveform_offline,
                decode_offline_stream,
                get_offline_stream_result,
                destroy_offline_recognizer_result,
                create_vad,
                destroy_vad,
                vad_accept_waveform,
                vad_empty,
                vad_detected,
                vad_pop,
                vad_front,
                destroy_speech_segment,
                vad_flush,
                vad_reset,
            })
        }
    }

    pub fn create_sense_voice_recognizer(
        &self,
        model_path: &Path,
        tokens_path: &Path,
        language: &str,
        num_threads: i32,
    ) -> Result<*const OpaqueRecognizer> {
        let c_model = path_to_cstring(model_path)?;
        let c_tokens = path_to_cstring(tokens_path)?;
        let c_lang = CString::new(if language.trim().is_empty() {
            "auto"
        } else {
            language.trim()
        })?;
        let c_provider = CString::new("cpu")?;
        let c_decoding = CString::new("greedy_search")?;
        let c_empty = CString::new("")?;

        let mut cfg: SherpaOnnxOfflineRecognizerConfig = unsafe { std::mem::zeroed() };
        cfg.feat_config.sample_rate = 16_000;
        cfg.feat_config.feature_dim = 80;
        cfg.model_config.sense_voice.model = c_model.as_ptr();
        cfg.model_config.sense_voice.language = c_lang.as_ptr();
        cfg.model_config.sense_voice.use_itn = 1;
        cfg.model_config.tokens = c_tokens.as_ptr();
        cfg.model_config.num_threads = num_threads.clamp(1, 16);
        cfg.model_config.debug = 0;
        cfg.model_config.provider = c_provider.as_ptr();
        cfg.model_config.model_type = c_empty.as_ptr();
        cfg.decoding_method = c_decoding.as_ptr();
        cfg.max_active_paths = 4;

        let handle = unsafe { (self.create_offline_recognizer)(&cfg) };
        if handle.is_null() {
            return Err(anyhow!(
                "初始化 SenseVoice 识别器失败，请检查模型文件 {} 与 {}",
                model_path.display(),
                tokens_path.display()
            ));
        }
        Ok(handle)
    }

    pub fn destroy_recognizer(&self, handle: *const OpaqueRecognizer) {
        if !handle.is_null() {
            unsafe { (self.destroy_offline_recognizer)(handle) };
        }
    }

    pub fn decode_offline(
        &self,
        recognizer: *const OpaqueRecognizer,
        samples_16k: &[f32],
    ) -> Result<(String, String)> {
        if samples_16k.is_empty() {
            return Ok((String::new(), String::new()));
        }

        let stream = unsafe { (self.create_offline_stream)(recognizer) };
        if stream.is_null() {
            return Err(anyhow!("创建离线 ASR Stream 失败"));
        }

        unsafe {
            (self.accept_waveform_offline)(
                stream,
                16_000,
                samples_16k.as_ptr(),
                samples_16k.len() as i32,
            );
            (self.decode_offline_stream)(recognizer, stream);
            let res_ptr = (self.get_offline_stream_result)(stream);
            let (text, lang) = if res_ptr.is_null() {
                (String::new(), String::new())
            } else {
                let res = &*res_ptr;
                let t = cstr_to_string(res.text);
                let l = cstr_to_string(res.lang);
                (self.destroy_offline_recognizer_result)(res_ptr);
                (t, l)
            };
            (self.destroy_offline_stream)(stream);
            Ok((text, lang))
        }
    }

    pub fn create_silero_vad(
        &self,
        vad_model_path: &Path,
        threshold: f32,
        min_silence_sec: f32,
        max_speech_sec: f32,
        num_threads: i32,
    ) -> Result<*const OpaqueVad> {
        let c_model = path_to_cstring(vad_model_path)?;
        let c_provider = CString::new("cpu")?;

        let mut cfg: SherpaOnnxVadModelConfig = unsafe { std::mem::zeroed() };
        cfg.silero_vad.model = c_model.as_ptr();
        cfg.silero_vad.threshold = threshold.clamp(0.1, 0.95);
        cfg.silero_vad.min_silence_duration = min_silence_sec.clamp(0.15, 2.5);
        cfg.silero_vad.min_speech_duration = 0.22;
        cfg.silero_vad.window_size = 512;
        cfg.silero_vad.max_speech_duration = max_speech_sec.clamp(2.5, 20.0);
        cfg.sample_rate = 16_000;
        cfg.num_threads = num_threads.clamp(1, 4);
        cfg.provider = c_provider.as_ptr();
        cfg.debug = 0;

        let handle = unsafe { (self.create_vad)(&cfg, 30.0) };
        if handle.is_null() {
            return Err(anyhow!(
                "初始化 Silero VAD 失败，请检查模型文件 {}",
                vad_model_path.display()
            ));
        }
        Ok(handle)
    }

    pub fn destroy_vad(&self, handle: *const OpaqueVad) {
        if !handle.is_null() {
            unsafe { (self.destroy_vad)(handle) };
        }
    }

    pub fn vad_accept_waveform(&self, vad: *const OpaqueVad, samples: &[f32]) {
        if !samples.is_empty() {
            unsafe { (self.vad_accept_waveform)(vad, samples.as_ptr(), samples.len() as i32) };
        }
    }

    pub fn vad_is_detected(&self, vad: *const OpaqueVad) -> bool {
        unsafe { (self.vad_detected)(vad) != 0 }
    }

    pub fn vad_is_empty(&self, vad: *const OpaqueVad) -> bool {
        unsafe { (self.vad_empty)(vad) != 0 }
    }

    pub fn vad_pop_segment(&self, vad: *const OpaqueVad) -> Option<Vec<f32>> {
        if self.vad_is_empty(vad) {
            return None;
        }
        unsafe {
            let seg_ptr = (self.vad_front)(vad);
            let samples = if !seg_ptr.is_null() {
                let seg = &*seg_ptr;
                let out = if !seg.samples.is_null() && seg.n > 0 {
                    std::slice::from_raw_parts(seg.samples, seg.n as usize).to_vec()
                } else {
                    Vec::new()
                };
                (self.destroy_speech_segment)(seg_ptr);
                out
            } else {
                Vec::new()
            };
            (self.vad_pop)(vad);
            if samples.is_empty() {
                None
            } else {
                Some(samples)
            }
        }
    }

    pub fn vad_flush(&self, vad: *const OpaqueVad) {
        unsafe { (self.vad_flush)(vad) };
    }

    pub fn vad_reset(&self, vad: *const OpaqueVad) {
        unsafe { (self.vad_reset)(vad) };
    }
}

fn path_to_cstring(p: &Path) -> Result<CString> {
    let s = p
        .to_str()
        .ok_or_else(|| anyhow!("路径包含非法 UTF-8 字符: {}", p.display()))?;
    Ok(CString::new(s)?)
}

unsafe fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr == ptr::null() {
        return String::new();
    }
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}
