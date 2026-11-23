use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use subprism_core::{build_system_prompt, build_user_prompt, ContextTurn, SubPrismConfig};

#[derive(Debug, Clone)]
pub struct StreamUpdate {
    pub text: String,
    pub ttft_ms: u64,
}

#[derive(Debug, Clone)]
pub struct TranslationOutcome {
    pub text: String,
    pub ttft_ms: u64,
    pub total_ms: u64,
    pub tokens_per_sec: f32,
}

/// Low-latency streaming translator targeting local `llama-server` running `Spark-X2.5-4B`.
/// Enforces Non-Thinking mode (`enable_thinking: false`), KV-cache prompt reuse (`cache_prompt: true`),
/// streaming `<think>` tag guard, and a rolling context window for natural video subtitle translation.
pub struct SparkTranslator {
    client: Client,
    endpoint: String,
    api_key: String,
    history: Mutex<VecDeque<ContextTurn>>,
}

impl SparkTranslator {
    pub fn new(endpoint: impl Into<String>, api_key: impl Into<String>) -> Result<Self> {
        let client = Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(45))
            .build()?;

        Ok(Self {
            client,
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            history: Mutex::new(VecDeque::with_capacity(8)),
        })
    }

    pub async fn clear_history(&self) {
        self.history.lock().await.clear();
    }

    pub async fn commit_turn(&self, source: String, translated: String, max_turns: usize) {
        if source.trim().is_empty() || translated.trim().is_empty() || max_turns == 0 {
            return;
        }
        let mut guard = self.history.lock().await;
        while guard.len() >= max_turns {
            guard.pop_front();
        }
        guard.push_back(ContextTurn { source, translated });
    }

    /// Streams translation tokens for `source_text`, invoking `on_update` for every incremental piece.
    /// Returns `Ok(None)` if cancelled via `cancel` before completion.
    pub async fn translate_streaming<F>(
        &self,
        source_text: &str,
        cfg: &SubPrismConfig,
        cancel: CancellationToken,
        mut on_update: F,
    ) -> Result<Option<TranslationOutcome>>
    where
        F: FnMut(StreamUpdate),
    {
        let src = source_text.trim();
        if src.is_empty() {
            return Ok(Some(TranslationOutcome {
                text: String::new(),
                ttft_ms: 0,
                total_ms: 0,
                tokens_per_sec: 0.0,
            }));
        }

        let context_snapshot: Vec<ContextTurn> = {
            let guard = self.history.lock().await;
            let keep = cfg.llm.context_turns.min(guard.len());
            guard.iter().rev().take(keep).rev().cloned().collect()
        };

        let system_prompt =
            build_system_prompt(&cfg.target_language, cfg.style_preset, &cfg.glossary);
        let user_prompt = build_user_prompt(&context_snapshot, src);

        let max_tokens = ((src.chars().count() * 3).clamp(48, 220)) as u32;

        let payload = json!({
            "model": "spark-x2.5-4b",
            "stream": true,
            "cache_prompt": true,
            "temperature": cfg.llm.temperature,
            "top_p": cfg.llm.top_p,
            "repeat_penalty": cfg.llm.repeat_penalty,
            "max_tokens": max_tokens,
            "chat_template_kwargs": {
                "enable_thinking": false
            },
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ]
        });

        let started = Instant::now();
        let url = format!("{}/v1/chat/completions", self.endpoint);

        let resp_fut = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send();

        let resp = tokio::select! {
            _ = cancel.cancelled() => return Ok(None),
            r = resp_fut => r?,
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("llama-server 返回错误 ({status}): {body}"));
        }

        let mut stream = resp.bytes_stream();
        let mut sse_buf = String::new();
        let mut stripper = ThinkStripper::default();
        let mut output = String::new();
        let mut ttft_ms: Option<u64> = None;
        let mut token_chunks: u32 = 0;

        loop {
            let next_chunk = tokio::select! {
                _ = cancel.cancelled() => return Ok(None),
                item = stream.next() => item,
            };

            let Some(chunk_res) = next_chunk else {
                break;
            };
            let bytes = chunk_res?;
            sse_buf.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(newline_pos) = sse_buf.find('\n') {
                let line = sse_buf[..newline_pos].trim().to_string();
                sse_buf.drain(..=newline_pos);

                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let data = data.trim();
                if data.is_empty() || data == "[DONE]" {
                    continue;
                }

                if let Ok(parsed) = serde_json::from_str::<ChatChunk>(data) {
                    if let Some(choice) = parsed.choices.first() {
                        if let Some(piece) = &choice.delta.content {
                            if !piece.is_empty() {
                                token_chunks += 1;
                                let clean = stripper.push(piece);
                                if !clean.is_empty() {
                                    let first_ms = *ttft_ms
                                        .get_or_insert_with(|| started.elapsed().as_millis() as u64);
                                    output.push_str(&clean);
                                    on_update(StreamUpdate {
                                        text: clean_subtitle_line(&output),
                                        ttft_ms: first_ms,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        let tail = stripper.finish();
        if !tail.is_empty() {
            output.push_str(&tail);
        }

        let final_text = clean_subtitle_line(&output);
        let total_ms = started.elapsed().as_millis() as u64;
        let ttft = ttft_ms.unwrap_or(total_ms);
        let gen_secs = (total_ms.saturating_sub(ttft).max(1) as f32) / 1000.0;
        let tokens_per_sec = if token_chunks > 1 {
            (token_chunks as f32) / gen_secs
        } else {
            0.0
        };

        Ok(Some(TranslationOutcome {
            text: final_text,
            ttft_ms: ttft,
            total_ms,
            tokens_per_sec,
        }))
    }
}

#[derive(Deserialize)]
struct ChatChunk {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    delta: ChatDelta,
}

#[derive(Deserialize)]
struct ChatDelta {
    content: Option<String>,
}

/// Stateful streaming filter that removes any accidental `<think>...</think>` reasoning trace
/// if a model template ignores `enable_thinking: false`.
#[derive(Default)]
struct ThinkStripper {
    in_think: bool,
    pending: String,
}

impl ThinkStripper {
    fn push(&mut self, piece: &str) -> String {
        self.pending.push_str(piece);
        let mut emitted = String::new();

        loop {
            if self.in_think {
                if let Some(end_idx) = self.pending.find("</think>") {
                    self.pending.drain(..end_idx + "</think>".len());
                    self.in_think = false;
                } else {
                    // Keep last 8 chars in case `</think>` spans chunk boundary
                    if self.pending.len() > 8 {
                        let keep_from = floor_char_boundary(&self.pending, self.pending.len() - 8);
                        self.pending.drain(..keep_from);
                    }
                    break;
                }
            } else if let Some(start_idx) = self.pending.find("<think>") {
                emitted.push_str(&self.pending[..start_idx]);
                self.pending.drain(..start_idx + "<think>".len());
                self.in_think = true;
            } else {
                // Hold potential partial `<think>` prefix at tail
                let hold = partial_tag_suffix_len(&self.pending, "<think>");
                let emit_len = self.pending.len() - hold;
                if emit_len > 0 {
                    emitted.push_str(&self.pending[..emit_len]);
                    self.pending.drain(..emit_len);
                }
                break;
            }
        }

        emitted
    }

    fn finish(&mut self) -> String {
        if self.in_think {
            self.pending.clear();
            String::new()
        } else {
            std::mem::take(&mut self.pending)
        }
    }
}

fn partial_tag_suffix_len(s: &str, tag: &str) -> usize {
    let max_check = s.len().min(tag.len() - 1);
    for len in (1..=max_check).rev() {
        if s.is_char_boundary(s.len() - len) && tag.starts_with(&s[s.len() - len..]) {
            return len;
        }
    }
    0
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn clean_subtitle_line(raw: &str) -> String {
    raw.trim()
        .replace('\r', "")
        .replace('\n', " ")
        .trim_matches('"')
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stripper_removes_split_think_blocks() {
        let mut s = ThinkStripper::default();
        let mut out = String::new();
        for piece in ["<thi", "nk>let me ", "think...</th", "ink>\n你好", "世界！"] {
            out.push_str(&s.push(piece));
        }
        out.push_str(&s.finish());
        assert_eq!(clean_subtitle_line(&out), "你好世界！");
    }
}
