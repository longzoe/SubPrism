pub mod resampler;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use subprism_core::AudioSourceInfo;

#[derive(Debug, Clone)]
pub struct AudioChunk {
    /// 16 kHz mono f32 PCM samples ready for Silero VAD & SenseVoice ASR
    pub samples_16k: Vec<f32>,
    /// Root-mean-square signal amplitude in [0.0, 1.0] for reactive UI meter
    pub rms: f32,
}

/// Handle controlling an active system audio loopback capture stream.
pub struct CaptureHandle {
    stop_flag: Arc<AtomicBool>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl CaptureHandle {
    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Lists available audio capture sources (default system loopback + active apps where supported).
pub fn list_audio_sources() -> Vec<AudioSourceInfo> {
    #[cfg(target_os = "windows")]
    {
        windows::list_audio_sources()
    }
    #[cfg(target_os = "macos")]
    {
        macos::list_audio_sources()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        vec![AudioSourceInfo {
            id: "system".to_string(),
            name: "系统全局内录音频".to_string(),
            kind: "system".to_string(),
            pid: None,
        }]
    }
}

/// Starts capturing system playback audio (zero virtual audio driver required)
/// and streams 16 kHz mono f32 PCM chunks into the returned receiver.
pub fn start_system_loopback(
    source_id: &str,
) -> Result<(CaptureHandle, mpsc::Receiver<AudioChunk>)> {
    let (tx, rx) = mpsc::channel::<AudioChunk>(64);
    let stop_flag = Arc::new(AtomicBool::new(false));
    let source = source_id.to_string();

    #[cfg(target_os = "windows")]
    {
        let handle = windows::start_loopback_thread(source, tx, stop_flag.clone())?;
        Ok((
            CaptureHandle {
                stop_flag,
                join_handle: Some(handle),
            },
            rx,
        ))
    }

    #[cfg(target_os = "macos")]
    {
        let handle = macos::start_loopback_thread(source, tx, stop_flag.clone())?;
        Ok((
            CaptureHandle {
                stop_flag,
                join_handle: Some(handle),
            },
            rx,
        ))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (tx, source);
        anyhow::bail!("当前操作系统暂不支持系统内录音频采集")
    }
}
