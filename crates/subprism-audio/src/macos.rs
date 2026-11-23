use crate::resampler::MonoResampler16k;
use crate::AudioChunk;
use anyhow::{anyhow, Context, Result};
use screencapturekit::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, warn};
use subprism_core::AudioSourceInfo;

pub fn list_audio_sources() -> Vec<AudioSourceInfo> {
    let mut sources = vec![AudioSourceInfo {
        id: "system".to_string(),
        name: "macOS 系统全局内录音频 (ScreenCaptureKit)".to_string(),
        kind: "system".to_string(),
        pid: None,
    }];

    if let Ok(content) = SCShareableContent::get() {
        for app in content.applications() {
            let name = app.application_name();
            let pid = app.process_id();
            if !name.is_empty() {
                sources.push(AudioSourceInfo {
                    id: format!("pid:{pid}"),
                    name: format!("应用: {name}"),
                    kind: "process".to_string(),
                    pid: Some(pid as u32),
                });
            }
        }
    }

    sources
}

pub fn start_loopback_thread(
    source_id: String,
    tx: mpsc::Sender<AudioChunk>,
    stop_flag: Arc<AtomicBool>,
) -> Result<JoinHandle<()>> {
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<Result<()>>(1);
    let thread_stop = stop_flag.clone();

    let handle = std::thread::Builder::new()
        .name("subprism-macos-sckit".to_string())
        .spawn(move || {
            let res = (|| -> Result<SCStream> {
                let content = SCShareableContent::get().map_err(|e| {
                    anyhow!("无法获取 macOS ScreenCaptureKit 权限，请在“系统设置 → 隐私与安全性 → 录屏与系统录音”中授权本应用: {e:?}")
                })?;

                let displays = content.displays();
                let display = displays
                    .first()
                    .ok_or_else(|| anyhow!("未检测到可用显示器"))?;

                let filter = if let Some(pid_str) = source_id.strip_prefix("pid:") {
                    if let Ok(target_pid) = pid_str.parse::<i32>() {
                        let apps: Vec<SCRunningApplication> = content
                            .applications()
                            .into_iter()
                            .filter(|a| a.process_id() == target_pid)
                            .collect();
                        if !apps.is_empty() {
                            let app_refs: Vec<&SCRunningApplication> = apps.iter().collect();
                            SCContentFilter::create()
                                .with_display(display)
                                .with_including_applications(&app_refs, &[])
                                .build()
                        } else {
                            SCContentFilter::create()
                                .with_display(display)
                                .with_excluding_windows(&[])
                                .build()
                        }
                    } else {
                        SCContentFilter::create()
                            .with_display(display)
                            .with_excluding_windows(&[])
                            .build()
                    }
                } else {
                    SCContentFilter::create()
                        .with_display(display)
                        .with_excluding_windows(&[])
                        .build()
                };

                let config = SCStreamConfiguration::new()
                    .with_captures_audio(true)
                    .with_excludes_current_process_audio(true)
                    .with_sample_rate(48_000_i32)
                    .with_channel_count(2_i32)
                    .with_width(2)
                    .with_height(2);

                let mut stream = SCStream::new(&filter, &config);
                let resampler = Arc::new(Mutex::new(MonoResampler16k::new(48_000, 1)));
                let tx_clone = tx.clone();

                stream.add_output_handler(
                    move |sample_buffer: CMSampleBuffer, of_type: SCStreamOutputType| {
                        if of_type != SCStreamOutputType::Audio {
                            return;
                        }
                        let Some(audio_list) = sample_buffer.audio_buffer_list() else {
                            return;
                        };
                        let num_bufs = audio_list.num_buffers();
                        if num_bufs == 0 {
                            return;
                        }

                        let mut mono_samples: Vec<f32> = Vec::new();
                        if num_bufs == 1 {
                            if let Some(buf) = audio_list.get(0) {
                                let raw = buf.data();
                                mono_samples.reserve(raw.len() / 4);
                                for chunk in raw.chunks_exact(4) {
                                    mono_samples.push(f32::from_le_bytes([
                                        chunk[0], chunk[1], chunk[2], chunk[3],
                                    ]));
                                }
                            }
                        } else if let Some(first) = audio_list.get(0) {
                            let frames = first.data().len() / 4;
                            mono_samples.resize(frames, 0.0);
                            for b_idx in 0..num_bufs {
                                if let Some(buf) = audio_list.get(b_idx) {
                                    for (idx, chunk) in
                                        buf.data().chunks_exact(4).take(frames).enumerate()
                                    {
                                        mono_samples[idx] += f32::from_le_bytes([
                                            chunk[0], chunk[1], chunk[2], chunk[3],
                                        ]) / num_bufs as f32;
                                    }
                                }
                            }
                        }

                        if let Ok(mut rs) = resampler.lock() {
                            let (samples_16k, rms) = rs.process_interleaved(&mono_samples);
                            if !samples_16k.is_empty() {
                                let _ = tx_clone.try_send(AudioChunk { samples_16k, rms });
                            }
                        }
                    },
                    SCStreamOutputType::Audio,
                );

                stream
                    .start_capture()
                    .map_err(|e| anyhow!("启动 ScreenCaptureKit 音频采集失败: {e:?}"))?;
                info!("macOS ScreenCaptureKit 系统音频内录已启动");
                Ok(stream)
            })();

            let stream = match res {
                Ok(s) => {
                    let _ = ready_tx.send(Ok(()));
                    s
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };

            while !thread_stop.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(50));
            }

            if let Err(e) = stream.stop_capture() {
                warn!("停止 ScreenCaptureKit 采集时出现警告: {e:?}");
            }
            info!("macOS ScreenCaptureKit 音频内录已停止");
        })
        .context("无法创建 macOS 音频采集线程")?;

    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => Ok(handle),
        Ok(Err(e)) => {
            stop_flag.store(true, Ordering::SeqCst);
            let _ = handle.join();
            Err(e)
        }
        Err(_) => {
            stop_flag.store(true, Ordering::SeqCst);
            Err(anyhow!("等待 macOS ScreenCaptureKit 初始化超时"))
        }
    }
}
