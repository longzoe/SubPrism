use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuBackend {
    Cuda,
    Metal,
    Vulkan,
    Cpu,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub os: String,
    pub arch: String,
    pub backend: GpuBackend,
    pub gpu_name: Option<String>,
    pub vram_mb: Option<u64>,
    pub cuda_version: Option<String>,
    pub recommended_ngl: u32,
    pub recommended_threads: usize,
}

/// Probes local GPU/accelerator capabilities (NVIDIA CUDA on Windows, Metal on macOS, etc.).
pub fn detect_hardware() -> HardwareInfo {
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let recommended_threads = (cpus / 2).clamp(2, 8);

    #[cfg(target_os = "macos")]
    {
        let is_apple_silicon = arch == "aarch64";
        let gpu_name = if is_apple_silicon {
            Some("Apple Silicon Unified GPU (Metal)".to_string())
        } else {
            Some("macOS GPU (Metal)".to_string())
        };
        return HardwareInfo {
            os,
            arch,
            backend: GpuBackend::Metal,
            gpu_name,
            vram_mb: None,
            cuda_version: None,
            recommended_ngl: 99,
            recommended_threads,
        };
    }

    #[cfg(target_os = "windows")]
    {
        if let Some((gpu_name, vram_mb, cuda_version)) = probe_nvidia_smi() {
            return HardwareInfo {
                os,
                arch,
                backend: GpuBackend::Cuda,
                gpu_name: Some(gpu_name),
                vram_mb,
                cuda_version,
                recommended_ngl: 99,
                recommended_threads,
            };
        }

        // Fallback check for Vulkan-capable GPUs via PowerShell CIM query if needed, else CPU
        return HardwareInfo {
            os,
            arch,
            backend: GpuBackend::Cpu,
            gpu_name: None,
            vram_mb: None,
            cuda_version: None,
            recommended_ngl: 0,
            recommended_threads,
        };
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        HardwareInfo {
            os,
            arch,
            backend: GpuBackend::Cpu,
            gpu_name: None,
            vram_mb: None,
            cuda_version: None,
            recommended_ngl: 0,
            recommended_threads,
        }
    }
}

#[cfg(target_os = "windows")]
fn probe_nvidia_smi() -> Option<(String, Option<u64>, Option<String>)> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let query = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;

    if !query.status.success() {
        return None;
    }

    let line = String::from_utf8_lossy(&query.stdout)
        .lines()
        .next()?
        .trim()
        .to_string();
    if line.is_empty() {
        return None;
    }

    let mut parts = line.split(',');
    let name = parts.next()?.trim().to_string();
    let vram_mb = parts.next().and_then(|s| s.trim().parse::<u64>().ok());

    // Probe CUDA UMD / runtime version from nvidia-smi banner
    let cuda_version = Command::new("nvidia-smi")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stdout);
            extract_cuda_version(&text)
        });

    Some((name, vram_mb, cuda_version))
}

#[cfg(target_os = "windows")]
fn extract_cuda_version(banner: &str) -> Option<String> {
    for marker in ["CUDA UMD Version:", "CUDA Version:"] {
        if let Some(idx) = banner.find(marker) {
            let rest = banner[idx + marker.len()..].trim_start();
            let ver: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if !ver.is_empty() {
                return Some(ver);
            }
        }
    }
    None
}
