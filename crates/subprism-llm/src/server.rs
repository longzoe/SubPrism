use anyhow::{anyhow, Context, Result};
use rand::distr::{Alphanumeric, SampleString};
use reqwest::Client;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
#[cfg(target_os = "windows")]
use tracing::warn;
use tracing::{debug, info};
use subprism_core::{detect_hardware, GpuBackend, LlmConfig, ModelPaths};

#[derive(Debug, Clone)]
pub struct LlamaServerOptions {
    pub server_bin: PathBuf,
    pub model_gguf: PathBuf,
    pub gpu_layers: u32,
    pub context_size: u32,
    pub parallel_slots: u32,
}

impl LlamaServerOptions {
    pub fn from_config(paths: &ModelPaths, llm: &LlmConfig) -> Self {
        let hw = detect_hardware();
        let effective_ngl = match hw.backend {
            GpuBackend::Cuda | GpuBackend::Metal | GpuBackend::Vulkan => llm.gpu_layers,
            GpuBackend::Cpu => 0,
        };
        Self {
            server_bin: PathBuf::from(&paths.llama_server_bin),
            model_gguf: PathBuf::from(&paths.llm_model_gguf),
            gpu_layers: effective_ngl,
            context_size: llm.context_size.clamp(1024, 32768),
            parallel_slots: llm.parallel_slots.clamp(1, 4),
        }
    }
}

/// Manages the lifecycle of a local `llama-server` subprocess dedicated to `Spark-X2.5-4B`.
/// Ensures clean teardown on exit and uses a Windows Job Object (`KILL_ON_JOB_CLOSE`) to prevent
/// orphaned GPU processes if the parent app terminates unexpectedly.
pub struct LlamaServerHandle {
    child: Mutex<Option<Child>>,
    endpoint: String,
    api_key: String,
    version: String,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    #[cfg(target_os = "windows")]
    _job_guard: Option<win_job::JobObjectGuard>,
}

impl LlamaServerHandle {
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub async fn start(opts: LlamaServerOptions) -> Result<Self> {
        if !opts.server_bin.is_file() {
            return Err(anyhow!(
                "找不到 llama-server 可执行文件: {}",
                opts.server_bin.display()
            ));
        }
        if !opts.model_gguf.is_file() {
            return Err(anyhow!(
                "找不到 Spark-X2.5-4B GGUF 模型文件: {}",
                opts.model_gguf.display()
            ));
        }

        let version = probe_llama_version(&opts.server_bin).unwrap_or_else(|_| "unknown".into());
        info!("探测到 llama-server 版本: {version}");

        let port = allocate_free_port()?;
        let api_key = Alphanumeric.sample_string(&mut rand::rng(), 32);
        let endpoint = format!("http://127.0.0.1:{port}");

        let mut cmd = Command::new(&opts.server_bin);
        if let Some(parent_dir) = opts.server_bin.parent() {
            // Ensure companion DLLs (cudart64_*.dll, ggml-cuda.dll, etc.) resolve reliably
            cmd.current_dir(parent_dir);
        }

        cmd.args([
            "-m",
            &opts.model_gguf.to_string_lossy(),
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--api-key",
            &api_key,
            "-c",
            &opts.context_size.to_string(),
            "-ngl",
            &opts.gpu_layers.to_string(),
            "--parallel",
            &opts.parallel_slots.to_string(),
            "--cont-batching",
            "--flash-attn",
            "on",
            "--cache-reuse",
            "256",
            "--jinja",
            "--reasoning",
            "off",
            "--reasoning-budget",
            "0",
        ]);

        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        info!(
            "正在启动 llama-server ({endpoint}, ngl={}, ctx={}, slots={})...",
            opts.gpu_layers, opts.context_size, opts.parallel_slots
        );

        let mut child = cmd
            .spawn()
            .with_context(|| format!("启动 llama-server 失败: {}", opts.server_bin.display()))?;

        #[cfg(target_os = "windows")]
        let job_guard = match win_job::JobObjectGuard::bind_process(child.id()) {
            Ok(g) => Some(g),
            Err(e) => {
                warn!("创建 Windows Job Object 失败（将继续运行）: {e:#}");
                None
            }
        };

        let stderr_tail = Arc::new(Mutex::new(VecDeque::<String>::with_capacity(64)));
        if let Some(stderr) = child.stderr.take() {
            let tail_ref = stderr_tail.clone();
            std::thread::Builder::new()
                .name("llama-stderr-reader".to_string())
                .spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines().map_while(Result::ok) {
                        debug!(target: "llama_server", "{line}");
                        if let Ok(mut buf) = tail_ref.lock() {
                            if buf.len() >= 60 {
                                buf.pop_front();
                            }
                            buf.push_back(line);
                        }
                    }
                })
                .ok();
        }

        let handle = Self {
            child: Mutex::new(Some(child)),
            endpoint,
            api_key,
            version,
            stderr_tail,
            #[cfg(target_os = "windows")]
            _job_guard: job_guard,
        };

        handle.wait_until_ready(Duration::from_secs(90)).await?;
        info!(
            "Spark-X2.5-4B llama-server 已就绪: {}",
            handle.endpoint()
        );
        Ok(handle)
    }

    async fn wait_until_ready(&self, timeout: Duration) -> Result<()> {
        let client = Client::builder()
            .timeout(Duration::from_secs(2))
            .no_proxy()
            .build()?;
        let url = format!("{}/health", self.endpoint);
        let started = Instant::now();

        loop {
            // Check if child exited early
            if let Ok(mut guard) = self.child.lock() {
                if let Some(child) = guard.as_mut() {
                    if let Ok(Some(status)) = child.try_wait() {
                        let logs = self.recent_logs();
                        return Err(anyhow!(
                            "llama-server 启动后提前退出 ({status})。请确认 llama.cpp 版本 >= b10833 并包含匹配 CUDA 运行库。\n最近日志:\n{logs}"
                        ));
                    }
                }
            }

            let resp = client
                .get(&url)
                .bearer_auth(&self.api_key)
                .send()
                .await;

            if let Ok(r) = resp {
                if r.status().is_success() {
                    return Ok(());
                }
                // 503 means still loading weights into VRAM
            }

            if started.elapsed() > timeout {
                let logs = self.recent_logs();
                return Err(anyhow!(
                    "等待 llama-server 加载 Spark-X2.5-4B 超时 ({}s)。\n最近日志:\n{logs}",
                    timeout.as_secs()
                ));
            }

            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    pub fn is_alive(&self) -> bool {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(child) = guard.as_mut() {
                return matches!(child.try_wait(), Ok(None));
            }
        }
        false
    }

    pub fn recent_logs(&self) -> String {
        self.stderr_tail
            .lock()
            .map(|q| q.iter().cloned().collect::<Vec<_>>().join("\n"))
            .unwrap_or_default()
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
                info!("已关闭 llama-server 进程");
            }
        }
    }
}

impl Drop for LlamaServerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

fn allocate_free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("无法分配本地监听端口")?;
    Ok(listener.local_addr()?.port())
}

pub fn probe_llama_version(bin: &Path) -> Result<String> {
    let mut cmd = Command::new(bin);
    if let Some(parent) = bin.parent() {
        cmd.current_dir(parent);
    }
    cmd.arg("--version");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output()?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let line = combined
        .lines()
        .find(|l| l.to_lowercase().contains("version:"))
        .or_else(|| combined.lines().find(|l| !l.trim().is_empty()))
        .unwrap_or("unknown")
        .trim()
        .to_string();
    Ok(line)
}

#[cfg(target_os = "windows")]
mod win_job {
    use anyhow::{anyhow, Result};
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

    pub struct JobObjectGuard(HANDLE);

    unsafe impl Send for JobObjectGuard {}
    unsafe impl Sync for JobObjectGuard {}

    impl JobObjectGuard {
        pub fn bind_process(pid: u32) -> Result<Self> {
            unsafe {
                let job = CreateJobObjectW(None, None)
                    .map_err(|e| anyhow!("CreateJobObjectW failed: {e}"))?;

                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

                SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
                .map_err(|e| {
                    let _ = CloseHandle(job);
                    anyhow!("SetInformationJobObject failed: {e}")
                })?;

                let proc_handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid)
                    .map_err(|e| {
                        let _ = CloseHandle(job);
                        anyhow!("OpenProcess failed: {e}")
                    })?;

                let assign_res = AssignProcessToJobObject(job, proc_handle);
                let _ = CloseHandle(proc_handle);

                if let Err(e) = assign_res {
                    let _ = CloseHandle(job);
                    return Err(anyhow!("AssignProcessToJobObject failed: {e}"));
                }

                Ok(Self(job))
            }
        }
    }

    impl Drop for JobObjectGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}
