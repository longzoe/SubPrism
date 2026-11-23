use crate::resampler::MonoResampler16k;
use crate::AudioChunk;
use anyhow::{anyhow, Context, Result};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use subprism_core::AudioSourceInfo;
use wasapi::{
    get_default_device, initialize_mta, Direction, SampleType, ShareMode, WaveFormat,
};

pub fn list_audio_sources() -> Vec<AudioSourceInfo> {
    let mut sources = vec![AudioSourceInfo {
        id: "system".to_string(),
        name: "系统全部播放声音 (WASAPI Loopback)".to_string(),
        kind: "system".to_string(),
        pid: None,
    }];

    // Enumerate active Windows audio render endpoints so user can pick specific speakers/headphones
    if initialize_mta().is_ok() {
        if let Ok(collection) = wasapi::DeviceCollection::new(&Direction::Render) {
            if let Ok(count) = collection.get_nbr_devices() {
                for idx in 0..count {
                    if let Ok(dev) = collection.get_device_at_index(idx) {
                        if let (Ok(id), Ok(name)) = (dev.get_id(), dev.get_friendlyname()) {
                            sources.push(AudioSourceInfo {
                                id: format!("device:{id}"),
                                name: format!("输出设备: {name}"),
                                kind: "device".to_string(),
                                pid: None,
                            });
                        }
                    }
                }
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
        .name("subprism-wasapi-loopback".to_string())
        .spawn(move || {
            if let Err(err) = run_wasapi_loop(&source_id, &tx, &thread_stop, ready_tx) {
                warn!("WASAPI loopback capture exited with error: {err:#}");
            }
        })
        .context("无法创建 WASAPI 音频采集线程")?;

    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => Ok(handle),
        Ok(Err(e)) => {
            stop_flag.store(true, Ordering::SeqCst);
            let _ = handle.join();
            Err(e)
        }
        Err(_) => {
            stop_flag.store(true, Ordering::SeqCst);
            Err(anyhow!("等待 WASAPI 音频回环初始化超时"))
        }
    }
}

fn resolve_render_device(source_id: &str) -> Result<wasapi::Device> {
    if let Some(dev_id) = source_id.strip_prefix("device:") {
        let collection = wasapi::DeviceCollection::new(&Direction::Render)
            .map_err(|e| anyhow!("枚举渲染设备失败: {e}"))?;
        let count = collection
            .get_nbr_devices()
            .map_err(|e| anyhow!("获取设备数量失败: {e}"))?;
        for idx in 0..count {
            if let Ok(dev) = collection.get_device_at_index(idx) {
                if dev.get_id().ok().as_deref() == Some(dev_id) {
                    return Ok(dev);
                }
            }
        }
        warn!("未找到指定输出设备 {dev_id}，将回退到系统默认输出设备");
    }

    get_default_device(&Direction::Render)
        .map_err(|e| anyhow!("获取默认播放音频设备失败: {e}"))
}

fn run_wasapi_loop(
    source_id: &str,
    tx: &mpsc::Sender<AudioChunk>,
    stop_flag: &Arc<AtomicBool>,
    ready_tx: std::sync::mpsc::SyncSender<Result<()>>,
) -> Result<()> {
    let _ = initialize_mta();

    let init_result: Result<(wasapi::AudioClient, wasapi::AudioCaptureClient, wasapi::Handle, u32, usize)> = (|| {
        let device = resolve_render_device(source_id)?;
        let dev_name = device
            .get_friendlyname()
            .unwrap_or_else(|_| "Default Render Device".to_string());

        let mut audio_client = device
            .get_iaudioclient()
            .map_err(|e| anyhow!("创建 IAudioClient 失败: {e}"))?;

        let mix_format = audio_client
            .get_mixformat()
            .map_err(|e| anyhow!("读取混音格式失败: {e}"))?;

        let sample_rate = mix_format.get_samplespersec();
        let channels = mix_format.get_nchannels() as usize;

        // Request 32-bit float PCM at device mix sample rate with autoconvert enabled
        let desired_format = WaveFormat::new(
            32,
            32,
            &SampleType::Float,
            sample_rate as usize,
            channels,
            None,
        );

        let (_def_time, min_time) = audio_client
            .get_periods()
            .map_err(|e| anyhow!("获取 WASAPI 周期失败: {e}"))?;
        let buffer_hns = min_time.max(200_000); // >= 20ms

        // Render device + Capture direction activates WASAPI Loopback automatically
        audio_client
            .initialize_client(
                &desired_format,
                buffer_hns,
                &Direction::Capture,
                &ShareMode::Shared,
                true,
            )
            .map_err(|e| anyhow!("初始化 WASAPI Loopback 失败: {e}"))?;

        let event_handle = audio_client
            .set_get_eventhandle()
            .map_err(|e| anyhow!("注册 WASAPI 事件句柄失败: {e}"))?;

        let capture_client = audio_client
            .get_audiocaptureclient()
            .map_err(|e| anyhow!("获取 IAudioCaptureClient 失败: {e}"))?;

        audio_client
            .start_stream()
            .map_err(|e| anyhow!("启动 WASAPI 音频流失败: {e}"))?;

        info!(
            "WASAPI Loopback 已启动: device=\"{dev_name}\", rate={sample_rate}Hz, ch={channels}"
        );

        Ok((audio_client, capture_client, event_handle, sample_rate, channels))
    })();

    let (audio_client, capture_client, event_handle, sample_rate, channels) = match init_result {
        Ok(tuple) => {
            let _ = ready_tx.send(Ok(()));
            tuple
        }
        Err(e) => {
            let _ = ready_tx.send(Err(anyhow!("{e:#}")));
            return Err(e);
        }
    };

    let mut resampler = MonoResampler16k::new(sample_rate, channels);
    let mut byte_queue: VecDeque<u8> = VecDeque::with_capacity(sample_rate as usize * channels * 4);
    let mut float_buf: Vec<f32> = Vec::with_capacity(4096);
    let bytes_per_frame = channels * std::mem::size_of::<f32>();

    while !stop_flag.load(Ordering::Relaxed) {
        // Wait up to 80ms so we stay responsive even when system audio is completely silent
        if let Err(e) = event_handle.wait_for_event(80) {
            debug!("WASAPI wait timeout/idle: {e}");
            // Send tiny silence chunk on idle so VAD can advance silence timers cleanly
            let silent = vec![0.0f32; 512];
            let _ = tx.try_send(AudioChunk {
                samples_16k: silent,
                rms: 0.0,
            });
            continue;
        }

        if let Err(e) = capture_client.read_from_device_to_deque(&mut byte_queue) {
            warn!("读取 WASAPI 音频包失败: {e}");
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }

        let available_frames = byte_queue.len() / bytes_per_frame;
        if available_frames == 0 {
            continue;
        }

        let total_samples = available_frames * channels;
        float_buf.clear();
        float_buf.reserve(total_samples);

        for _ in 0..total_samples {
            let b0 = byte_queue.pop_front().unwrap_or(0);
            let b1 = byte_queue.pop_front().unwrap_or(0);
            let b2 = byte_queue.pop_front().unwrap_or(0);
            let b3 = byte_queue.pop_front().unwrap_or(0);
            float_buf.push(f32::from_le_bytes([b0, b1, b2, b3]));
        }

        let (samples_16k, rms) = resampler.process_interleaved(&float_buf);
        if !samples_16k.is_empty() {
            // Non-blocking send: drop oldest only if downstream is saturated
            let _ = tx.try_send(AudioChunk { samples_16k, rms });
        }
    }

    let _ = audio_client.stop_stream();
    info!("WASAPI Loopback 已停止");
    Ok(())
}
