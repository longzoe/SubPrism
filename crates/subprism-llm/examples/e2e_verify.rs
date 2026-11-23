use anyhow::{anyhow, Result};
use std::path::Path;
use tokio_util::sync::CancellationToken;
use subprism_asr::{AsrEngine, TranscriptEvent};
use subprism_core::{detect_hardware, SubPrismConfig};
use subprism_llm::{LlamaServerHandle, LlamaServerOptions, SparkTranslator};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let hw = detect_hardware();
    println!("=== 1. Hardware Probe ===");
    println!(
        "OS={} ARCH={} Backend={:?} GPU={:?} VRAM={:?}MB CUDA={:?} NGL={}",
        hw.os, hw.arch, hw.backend, hw.gpu_name, hw.vram_mb, hw.cuda_version, hw.recommended_ngl
    );

    let cfg = SubPrismConfig::default();
    let missing = cfg.paths.validate();
    if !missing.is_empty() {
        return Err(anyhow!("Local dev assets incomplete: {:?}", missing));
    }
    println!("\n=== 2. Local Dev Assets Resolved ===");
    println!("llama-server : {}", cfg.paths.llama_server_bin);
    println!("Spark GGUF   : {}", cfg.paths.llm_model_gguf);
    println!("sherpa-onnx  : {}", cfg.paths.sherpa_lib_path);
    println!("SenseVoice   : {}", cfg.paths.asr_model_dir);
    println!("Silero VAD   : {}", cfg.paths.vad_model_path);

    println!("\n=== 3. Silero VAD v5 + SenseVoice-Small Real WAV Test ===");
    let wav_path = Path::new("local_dev_assets/asr/models/test_silero_vad.wav");
    let samples = read_wav_16k_mono(wav_path)?;
    println!(
        "Loaded WAV: {} samples ({:.2}s @ 16kHz)",
        samples.len(),
        samples.len() as f32 / 16_000.0
    );

    let mut asr = AsrEngine::load(&cfg.paths, &cfg.asr)?;
    let mut sentences = Vec::new();
    for chunk in samples.chunks(1600) {
        let (_speaking, events) = asr.push_audio(chunk)?;
        for ev in events {
            if let TranscriptEvent::Final {
                text, lang, asr_ms, ..
            } = ev
            {
                println!("  [ASR Final | {asr_ms}ms | lang={lang}] {text}");
                sentences.push(text);
            }
        }
    }
    for ev in asr.flush()? {
        if let TranscriptEvent::Final {
            text, lang, asr_ms, ..
        } = ev
        {
            println!("  [ASR Flush | {asr_ms}ms | lang={lang}] {text}");
            sentences.push(text);
        }
    }
    if sentences.is_empty() {
        return Err(anyhow!("Expected at least 1 speech segment from test WAV"));
    }

    println!("\n=== 4. Auto-Launch llama-server + Spark-X2.5-4B (Non-Thinking CUDA) ===");
    let opts = LlamaServerOptions::from_config(&cfg.paths, &cfg.llm);
    let server = LlamaServerHandle::start(opts).await?;
    println!(
        "llama-server READY at {} ({})",
        server.endpoint(),
        server.version()
    );

    let translator = SparkTranslator::new(server.endpoint(), server.api_key())?;

    // Translate real ASR sentences + realistic video subtitle test lines
    let mut test_lines = sentences;
    test_lines.push("So today we are running Spark-X2.5-4B locally on an RTX 5080 with llama.cpp for zero-latency subtitles.".to_string());
    test_lines.push("Notice how it keeps the context from the previous sentence and sounds completely natural.".to_string());

    for (idx, line) in test_lines.iter().enumerate() {
        let outcome = translator
            .translate_streaming(line, &cfg, CancellationToken::new(), |_| {})
            .await?
            .ok_or_else(|| anyhow!("cancelled"))?;

        println!(
            "\n[Turn {}]\n  SRC : {}\n  TR  : {}\n  PERF: TTFT={}ms | Total={}ms | Speed={:.1} tok/s",
            idx + 1,
            line,
            outcome.text,
            outcome.ttft_ms,
            outcome.total_ms,
            outcome.tokens_per_sec
        );

        assert!(
            !outcome.text.contains("<think>") && !outcome.text.contains("</think>"),
            "Think tag leaked into output: {}",
            outcome.text
        );
        assert!(!outcome.text.trim().is_empty(), "Empty translation output");

        translator
            .commit_turn(line.clone(), outcome.text, cfg.llm.context_turns)
            .await;
    }

    println!("\n=== 5. Graceful Shutdown ===");
    server.stop();
    println!("ALL END-TO-END CHECKS PASSED!");
    Ok(())
}

fn read_wav_16k_mono(path: &Path) -> Result<Vec<f32>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(anyhow!("Invalid WAV header: {}", path.display()));
    }
    let mut pos = 12usize;
    let mut channels = 1u16;
    let mut bits_per_sample = 16u16;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into()?) as usize;
        pos += 8;
        if pos + size > bytes.len() {
            break;
        }
        if id == b"fmt " && size >= 16 {
            channels = u16::from_le_bytes(bytes[pos + 2..pos + 4].try_into()?);
            bits_per_sample = u16::from_le_bytes(bytes[pos + 14..pos + 16].try_into()?);
        } else if id == b"data" {
            if bits_per_sample != 16 {
                return Err(anyhow!("Only 16-bit PCM WAV supported in test helper"));
            }
            let ch = channels.max(1) as usize;
            let frame_bytes = ch * 2;
            let frames = size / frame_bytes;
            let mut out = Vec::with_capacity(frames);
            for f in 0..frames {
                let mut acc = 0.0f32;
                for c in 0..ch {
                    let o = pos + f * frame_bytes + c * 2;
                    let s = i16::from_le_bytes([bytes[o], bytes[o + 1]]) as f32 / 32768.0;
                    acc += s;
                }
                out.push(acc / ch as f32);
            }
            return Ok(out);
        }
        pos += size + (size % 2);
    }
    Err(anyhow!("WAV data chunk not found"))
}
