use anyhow::{anyhow, Result};
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use subprism_asr::{AsrEngine, TranscriptEvent};
use subprism_audio::{start_system_loopback, CaptureHandle};
use subprism_core::{
    EnginePhase, EngineStatus, PipelineMetrics, SubtitleEntry, SubtitleEvent, SubtitleStatus,
    SubPrismConfig,
};
use subprism_llm::{LlamaServerHandle, LlamaServerOptions, SparkTranslator};
use uuid::Uuid;

pub const SUBTITLE_EVENT_NAME: &str = "subprism://subtitle-event";

struct ActiveSession {
    cancel: CancellationToken,
    paused: Arc<AtomicBool>,
    capture_handle: Option<CaptureHandle>,
}

pub struct PipelineController {
    app: AppHandle,
    config: Arc<RwLock<SubPrismConfig>>,
    status: Arc<RwLock<EngineStatus>>,
    metrics: Arc<RwLock<PipelineMetrics>>,
    history: Arc<RwLock<VecDeque<SubtitleEntry>>>,
    llm_server: Arc<Mutex<Option<LlamaServerHandle>>>,
    translator: Arc<Mutex<Option<Arc<SparkTranslator>>>>,
    session: Arc<Mutex<Option<ActiveSession>>>,
}

impl PipelineController {
    pub fn new(app: AppHandle, initial_config: SubPrismConfig) -> Self {
        let initial_phase = if initial_config.paths.is_complete() {
            EnginePhase::Idle
        } else {
            EnginePhase::NeedsSetup
        };
        let initial_msg = if initial_phase == EnginePhase::NeedsSetup {
            "请先在设置中指定 llama-server、Spark-X2.5-4B 模型及本地 ASR 模型路径".to_string()
        } else {
            "就绪 — 点击开始监听视频字幕".to_string()
        };

        Self {
            app,
            config: Arc::new(RwLock::new(initial_config)),
            status: Arc::new(RwLock::new(EngineStatus {
                phase: initial_phase,
                message: initial_msg,
                ..Default::default()
            })),
            metrics: Arc::new(RwLock::new(PipelineMetrics::default())),
            history: Arc::new(RwLock::new(VecDeque::with_capacity(512))),
            llm_server: Arc::new(Mutex::new(None)),
            translator: Arc::new(Mutex::new(None)),
            session: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn get_config(&self) -> SubPrismConfig {
        self.config.read().await.clone()
    }

    pub async fn update_config(&self, new_cfg: SubPrismConfig) -> Result<()> {
        let prev = self.config.read().await.clone();
        let llm_paths_changed = prev.paths.llama_server_bin != new_cfg.paths.llama_server_bin
            || prev.paths.llm_model_gguf != new_cfg.paths.llm_model_gguf
            || prev.llm.gpu_layers != new_cfg.llm.gpu_layers
            || prev.llm.context_size != new_cfg.llm.context_size;

        let target_changed = prev.target_language != new_cfg.target_language
            || prev.style_preset != new_cfg.style_preset;

        let asr_or_audio_changed = prev.audio_source_id != new_cfg.audio_source_id
            || prev.asr.source_language != new_cfg.asr.source_language
            || (prev.asr.vad_threshold - new_cfg.asr.vad_threshold).abs() > 0.001
            || prev.asr.min_silence_ms != new_cfg.asr.min_silence_ms
            || prev.asr.soft_speech_boost != new_cfg.asr.soft_speech_boost
            || (prev.asr.agc_max_gain_db - new_cfg.asr.agc_max_gain_db).abs() > 0.1
            || prev.paths.asr_model_dir != new_cfg.paths.asr_model_dir
            || prev.paths.vad_model_path != new_cfg.paths.vad_model_path;

        new_cfg.save()?;
        *self.config.write().await = new_cfg.clone();

        if target_changed {
            if let Some(tr) = self.translator.lock().await.as_ref() {
                tr.clear_history().await;
            }
        }

        if llm_paths_changed {
            // Drop old llama-server so next start reloads with updated binary/model
            self.shutdown_llm().await;
        }

        // Update Idle / NeedsSetup phase, or hot-reload ASR if listening with changed VAD/AGC settings
        let is_running = self.session.lock().await.is_some();
        if is_running && asr_or_audio_changed {
            self.stop_listening().await;
            if let Err(e) = self.start_listening().await {
                warn!("热重载 ASR/音频采集失败: {e:#}");
            }
        } else if !is_running {
            if new_cfg.paths.is_complete() {
                self.set_status(|s| {
                    s.phase = EnginePhase::Idle;
                    s.message = "模型配置已就绪 — 点击开始监听".to_string();
                })
                .await;
            } else {
                self.set_status(|s| {
                    s.phase = EnginePhase::NeedsSetup;
                    s.message = "请先完善本地模型与运行库路径".to_string();
                })
                .await;
            }
        }

        Ok(())
    }

    pub async fn get_status(&self) -> EngineStatus {
        self.status.read().await.clone()
    }

    pub async fn get_metrics(&self) -> PipelineMetrics {
        self.metrics.read().await.clone()
    }

    pub async fn get_history(&self) -> Vec<SubtitleEntry> {
        self.history.read().await.iter().cloned().collect()
    }

    pub async fn clear_history(&self) {
        self.history.write().await.clear();
        if let Some(tr) = self.translator.lock().await.as_ref() {
            tr.clear_history().await;
        }
    }

    /// Ensures the local `llama-server` hosting `Spark-X2.5-4B` is running and healthy.
    pub async fn ensure_llm_started(&self) -> Result<Arc<SparkTranslator>> {
        // Fast path: reuse alive server
        {
            let srv_guard = self.llm_server.lock().await;
            let tr_guard = self.translator.lock().await;
            if let (Some(srv), Some(tr)) = (srv_guard.as_ref(), tr_guard.as_ref()) {
                if srv.is_alive() {
                    return Ok(tr.clone());
                }
            }
        }

        let cfg = self.config.read().await.clone();
        self.set_status(|s| {
            s.phase = EnginePhase::StartingLlm;
            s.llm_ready = false;
            s.message = "正在自动拉起 llama-server 并加载 Spark-X2.5-4B...".to_string();
        })
        .await;

        let opts = LlamaServerOptions::from_config(&cfg.paths, &cfg.llm);
        let handle = match LlamaServerHandle::start(opts).await {
            Ok(h) => h,
            Err(e) => {
                let msg = format!("启动 Spark-X2.5-4B 服务失败: {e:#}");
                self.set_status(|s| {
                    s.phase = EnginePhase::Error;
                    s.llm_ready = false;
                    s.message = msg.clone();
                })
                .await;
                self.emit(SubtitleEvent::Error { message: msg });
                return Err(e);
            }
        };

        let endpoint = handle.endpoint().to_string();
        let version = handle.version().to_string();
        let translator = Arc::new(SparkTranslator::new(&endpoint, handle.api_key())?);

        *self.llm_server.lock().await = Some(handle);
        *self.translator.lock().await = Some(translator.clone());

        self.set_status(|s| {
            s.llm_ready = true;
            s.llama_endpoint = Some(endpoint);
            s.llama_version = Some(version);
        })
        .await;

        Ok(translator)
    }

    pub async fn start_listening(&self) -> Result<()> {
        if self.session.lock().await.is_some() {
            // If paused, simply resume
            self.set_paused(false).await;
            return Ok(());
        }

        let cfg = self.config.read().await.clone();
        let missing = cfg.paths.validate();
        if !missing.is_empty() {
            let msg = format!("本地模型未配置完整：{}", missing.join("；"));
            self.set_status(|s| {
                s.phase = EnginePhase::NeedsSetup;
                s.message = msg.clone();
            })
            .await;
            return Err(anyhow!(msg));
        }

        // 1. Auto-start llama-server with Spark-X2.5-4B
        let translator = self.ensure_llm_started().await?;

        // 2. Load local Silero VAD + SenseVoice-Small ASR on blocking pool
        self.set_status(|s| {
            s.phase = EnginePhase::LoadingAsr;
            s.asr_ready = false;
            s.message = "正在加载本地 Silero VAD + SenseVoice 语音引擎...".to_string();
        })
        .await;

        let paths_clone = cfg.paths.clone();
        let asr_cfg_clone = cfg.asr.clone();
        let asr_engine = tokio::task::spawn_blocking(move || AsrEngine::load(&paths_clone, &asr_cfg_clone))
            .await
            .map_err(|e| anyhow!("加载 ASR 线程异常: {e}"))??;

        self.set_status(|s| {
            s.asr_ready = true;
            s.message = "正在启动系统内录音频采集...".to_string();
        })
        .await;

        // 3. Start platform system audio loopback (Win WASAPI / Mac ScreenCaptureKit)
        let (capture_handle, mut audio_rx) = start_system_loopback(&cfg.audio_source_id)?;

        let cancel = CancellationToken::new();
        let paused = Arc::new(AtomicBool::new(false));
        let (transcript_tx, mut transcript_rx) = mpsc::channel::<TranscriptEvent>(32);

        // 4. Dedicated OS thread for CPU-bound VAD + SenseVoice decoding
        let asr_cancel = cancel.clone();
        let asr_paused = paused.clone();
        let app_for_meter = self.app.clone();
        std::thread::Builder::new()
            .name("subprism-asr-worker".to_string())
            .spawn(move || {
                let mut engine = asr_engine;
                let mut meter_tick: u32 = 0;

                while !asr_cancel.is_cancelled() {
                    let Some(chunk) = audio_rx.blocking_recv() else {
                        break;
                    };

                    if asr_paused.load(Ordering::Relaxed) {
                        engine.reset();
                        continue;
                    }

                    match engine.push_audio(&chunk.samples_16k) {
                        Ok((speaking, events)) => {
                            meter_tick = meter_tick.wrapping_add(1);
                            if meter_tick % 3 == 0 {
                                let _ = app_for_meter.emit(
                                    SUBTITLE_EVENT_NAME,
                                    SubtitleEvent::AudioLevel {
                                        rms: chunk.rms,
                                        speaking,
                                    },
                                );
                            }
                            for ev in events {
                                if transcript_tx.blocking_send(ev).is_err() {
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("ASR 推理帧异常: {e:#}");
                        }
                    }
                }
                info!("ASR 工作线程已退出");
            })?;

        // 5. Async translation coordinator (streaming Spark-X2.5-4B with partial cancellation)
        let tr_cancel = cancel.clone();
        let tr_config = self.config.clone();
        let tr_metrics = self.metrics.clone();
        let tr_history = self.history.clone();
        let app_for_tr = self.app.clone();

        tokio::spawn(async move {
            let mut active_partial_cancel: Option<CancellationToken> = None;

            loop {
                let ev = tokio::select! {
                    _ = tr_cancel.cancelled() => break,
                    item = transcript_rx.recv() => match item {
                        Some(ev) => ev,
                        None => break,
                    }
                };

                let live_cfg = tr_config.read().await.clone();

                match ev {
                    TranscriptEvent::Partial {
                        utterance_id,
                        text,
                        lang,
                        asr_ms,
                    } => {
                        if let Some(prev) = active_partial_cancel.take() {
                            prev.cancel();
                        }

                        // Immediately show live source text on overlay
                        let ts = now_ms();
                        let _ = app_for_tr.emit(
                            SUBTITLE_EVENT_NAME,
                            SubtitleEvent::Upsert {
                                entry: SubtitleEntry {
                                    id: utterance_id.clone(),
                                    timestamp_ms: ts,
                                    source_lang: lang.clone(),
                                    source_text: text.clone(),
                                    translated_text: String::new(),
                                    status: SubtitleStatus::Partial,
                                    asr_latency_ms: Some(asr_ms),
                                    ttft_ms: None,
                                    translate_latency_ms: None,
                                },
                            },
                        );

                        if !live_cfg.llm.translate_partials || text.chars().count() < 2 {
                            continue;
                        }

                        let child_cancel = tr_cancel.child_token();
                        active_partial_cancel = Some(child_cancel.clone());
                        let tr_clone = translator.clone();
                        let app_clone = app_for_tr.clone();

                        tokio::spawn(async move {
                            let _ = tr_clone
                                .translate_streaming(&text, &live_cfg, child_cancel, |upd| {
                                    let _ = app_clone.emit(
                                        SUBTITLE_EVENT_NAME,
                                        SubtitleEvent::Upsert {
                                            entry: SubtitleEntry {
                                                id: utterance_id.clone(),
                                                timestamp_ms: ts,
                                                source_lang: lang.clone(),
                                                source_text: text.clone(),
                                                translated_text: upd.text,
                                                status: SubtitleStatus::Partial,
                                                asr_latency_ms: Some(asr_ms),
                                                ttft_ms: Some(upd.ttft_ms),
                                                translate_latency_ms: None,
                                            },
                                        },
                                    );
                                })
                                .await;
                        });
                    }

                    TranscriptEvent::Final {
                        utterance_id,
                        text,
                        lang,
                        asr_ms,
                    } => {
                        // Supersede any in-flight partial for this utterance
                        if let Some(prev) = active_partial_cancel.take() {
                            prev.cancel();
                        }

                        let ts = now_ms();
                        let _ = app_for_tr.emit(
                            SUBTITLE_EVENT_NAME,
                            SubtitleEvent::Upsert {
                                entry: SubtitleEntry {
                                    id: utterance_id.clone(),
                                    timestamp_ms: ts,
                                    source_lang: lang.clone(),
                                    source_text: text.clone(),
                                    translated_text: String::new(),
                                    status: SubtitleStatus::Translating,
                                    asr_latency_ms: Some(asr_ms),
                                    ttft_ms: None,
                                    translate_latency_ms: None,
                                },
                            },
                        );

                        let res = translator
                            .translate_streaming(&text, &live_cfg, tr_cancel.clone(), |upd| {
                                let _ = app_for_tr.emit(
                                    SUBTITLE_EVENT_NAME,
                                    SubtitleEvent::Upsert {
                                        entry: SubtitleEntry {
                                            id: utterance_id.clone(),
                                            timestamp_ms: ts,
                                            source_lang: lang.clone(),
                                            source_text: text.clone(),
                                            translated_text: upd.text,
                                            status: SubtitleStatus::Translating,
                                            asr_latency_ms: Some(asr_ms),
                                            ttft_ms: Some(upd.ttft_ms),
                                            translate_latency_ms: None,
                                        },
                                    },
                                );
                            })
                            .await;

                        match res {
                            Ok(Some(outcome)) => {
                                let final_entry = SubtitleEntry {
                                    id: utterance_id.clone(),
                                    timestamp_ms: ts,
                                    source_lang: lang.clone(),
                                    source_text: text.clone(),
                                    translated_text: outcome.text.clone(),
                                    status: SubtitleStatus::Final,
                                    asr_latency_ms: Some(asr_ms),
                                    ttft_ms: Some(outcome.ttft_ms),
                                    translate_latency_ms: Some(outcome.total_ms),
                                };

                                let _ = app_for_tr.emit(
                                    SUBTITLE_EVENT_NAME,
                                    SubtitleEvent::Upsert {
                                        entry: final_entry.clone(),
                                    },
                                );

                                // Commit to Spark-X2.5-4B rolling context window
                                translator
                                    .commit_turn(
                                        text,
                                        outcome.text,
                                        live_cfg.llm.context_turns,
                                    )
                                    .await;

                                // Store in session history for Settings viewer & SRT export
                                {
                                    let mut hist = tr_history.write().await;
                                    if hist.len() >= 500 {
                                        hist.pop_front();
                                    }
                                    hist.push_back(final_entry);
                                }

                                // Update telemetry
                                let snap = {
                                    let mut m = tr_metrics.write().await;
                                    m.last_asr_ms = asr_ms;
                                    m.last_ttft_ms = outcome.ttft_ms;
                                    m.last_translate_ms = outcome.total_ms;
                                    m.tokens_per_sec = outcome.tokens_per_sec;
                                    m.total_utterances += 1;
                                    m.clone()
                                };
                                let _ = app_for_tr.emit(
                                    SUBTITLE_EVENT_NAME,
                                    SubtitleEvent::Metrics { metrics: snap },
                                );
                            }
                            Ok(None) => {}
                            Err(e) => {
                                error!("Spark-X2.5-4B 翻译请求失败: {e:#}");
                                let _ = app_for_tr.emit(
                                    SUBTITLE_EVENT_NAME,
                                    SubtitleEvent::Error {
                                        message: format!("翻译异常: {e}"),
                                    },
                                );
                            }
                        }
                    }
                }
            }
            info!("翻译协程已停止");
        });

        *self.session.lock().await = Some(ActiveSession {
            cancel,
            paused,
            capture_handle: Some(capture_handle),
        });

        self.set_status(|s| {
            s.phase = EnginePhase::Listening;
            s.audio_active = true;
            s.message = "正在监听内播音频并实时翻译...".to_string();
        })
        .await;

        Ok(())
    }

    pub async fn set_paused(&self, pause: bool) {
        let guard = self.session.lock().await;
        if let Some(sess) = guard.as_ref() {
            sess.paused.store(pause, Ordering::SeqCst);
            drop(guard);
            self.set_status(|s| {
                s.phase = if pause {
                    EnginePhase::Paused
                } else {
                    EnginePhase::Listening
                };
                s.audio_active = !pause;
                s.message = if pause {
                    "已暂停监听".to_string()
                } else {
                    "正在监听内播音频并实时翻译...".to_string()
                };
            })
            .await;
        }
    }

    pub async fn toggle_listening(&self) -> Result<()> {
        let state = {
            let guard = self.session.lock().await;
            guard
                .as_ref()
                .map(|s| s.paused.load(Ordering::Relaxed))
        };
        match state {
            None => self.start_listening().await,
            Some(true) => {
                self.set_paused(false).await;
                Ok(())
            }
            Some(false) => {
                self.set_paused(true).await;
                Ok(())
            }
        }
    }

    pub async fn stop_listening(&self) {
        let mut guard = self.session.lock().await;
        if let Some(mut sess) = guard.take() {
            sess.cancel.cancel();
            if let Some(cap) = sess.capture_handle.take() {
                let _ = tokio::task::spawn_blocking(move || cap.stop()).await;
            }
        }
        drop(guard);

        self.set_status(|s| {
            s.phase = EnginePhase::Idle;
            s.audio_active = false;
            s.message = "已停止监听（Spark-X2.5-4B 服务保持待命）".to_string();
        })
        .await;
    }

    pub async fn shutdown_all(&self) {
        self.stop_listening().await;
        self.shutdown_llm().await;
    }

    async fn shutdown_llm(&self) {
        *self.translator.lock().await = None;
        let server = self.llm_server.lock().await.take();
        if let Some(srv) = server {
            let _ = tokio::task::spawn_blocking(move || srv.stop()).await;
        }
        self.set_status(|s| {
            s.llm_ready = false;
            s.llama_endpoint = None;
        })
        .await;
    }

    /// Runs a live one-shot translation through Spark-X2.5-4B (also pushes to overlay for preview).
    pub async fn test_translate(&self, sample_text: String) -> Result<SubtitleEntry> {
        let translator = self.ensure_llm_started().await?;
        let cfg = self.config.read().await.clone();
        let id = Uuid::new_v4().to_string();
        let ts = now_ms();

        let app_clone = self.app.clone();
        let id_clone = id.clone();
        let src_clone = sample_text.clone();

        let outcome = translator
            .translate_streaming(&sample_text, &cfg, CancellationToken::new(), |upd| {
                let _ = app_clone.emit(
                    SUBTITLE_EVENT_NAME,
                    SubtitleEvent::Upsert {
                        entry: SubtitleEntry {
                            id: id_clone.clone(),
                            timestamp_ms: ts,
                            source_lang: "test".to_string(),
                            source_text: src_clone.clone(),
                            translated_text: upd.text,
                            status: SubtitleStatus::Translating,
                            asr_latency_ms: None,
                            ttft_ms: Some(upd.ttft_ms),
                            translate_latency_ms: None,
                        },
                    },
                );
            })
            .await?
            .ok_or_else(|| anyhow!("测试翻译被取消"))?;

        let entry = SubtitleEntry {
            id,
            timestamp_ms: ts,
            source_lang: "test".to_string(),
            source_text: sample_text,
            translated_text: outcome.text,
            status: SubtitleStatus::Final,
            asr_latency_ms: None,
            ttft_ms: Some(outcome.ttft_ms),
            translate_latency_ms: Some(outcome.total_ms),
        };

        self.emit(SubtitleEvent::Upsert {
            entry: entry.clone(),
        });

        let snap = {
            let mut m = self.metrics.write().await;
            m.last_ttft_ms = outcome.ttft_ms;
            m.last_translate_ms = outcome.total_ms;
            m.tokens_per_sec = outcome.tokens_per_sec;
            m.clone()
        };
        self.emit(SubtitleEvent::Metrics { metrics: snap });

        Ok(entry)
    }

    pub async fn export_srt(&self, target_path: &Path, include_source: bool) -> Result<usize> {
        let items = self.get_history().await;
        if items.is_empty() {
            return Err(anyhow!("当前会话暂无可导出的字幕记录"));
        }

        let base_ts = items.first().map(|i| i.timestamp_ms).unwrap_or(0);
        let mut buf = String::new();

        for (idx, item) in items.iter().enumerate() {
            let start = item.timestamp_ms.saturating_sub(base_ts);
            let end = items
                .get(idx + 1)
                .map(|next| next.timestamp_ms.saturating_sub(base_ts).min(start + 5500))
                .unwrap_or(start + 3500)
                .max(start + 900);

            buf.push_str(&format!(
                "{}\n{} --> {}\n",
                idx + 1,
                format_srt_time(start),
                format_srt_time(end)
            ));
            if include_source && !item.source_text.is_empty() {
                buf.push_str(&item.source_text);
                buf.push('\n');
            }
            buf.push_str(&item.translated_text);
            buf.push_str("\n\n");
        }

        fs::write(target_path, buf)?;
        Ok(items.len())
    }

    async fn set_status<F: FnOnce(&mut EngineStatus)>(&self, mutate: F) {
        let snapshot = {
            let mut guard = self.status.write().await;
            mutate(&mut guard);
            guard.clone()
        };
        self.emit(SubtitleEvent::StatusChanged { status: snapshot });
    }

    fn emit(&self, ev: SubtitleEvent) {
        let _ = self.app.emit(SUBTITLE_EVENT_NAME, ev);
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn format_srt_time(ms: u64) -> String {
    let hours = ms / 3_600_000;
    let mins = (ms % 3_600_000) / 60_000;
    let secs = (ms % 60_000) / 1000;
    let millis = ms % 1000;
    format!("{hours:02}:{mins:02}:{secs:02},{millis:03}")
}
