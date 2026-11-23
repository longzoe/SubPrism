use crate::config::{GlossaryEntry, StylePreset};

#[derive(Debug, Clone)]
pub struct ContextTurn {
    pub source: String,
    pub translated: String,
}

/// Builds the fixed System Prompt optimized for Spark-X2.5-4B (Non-Thinking mode)
/// and KV-cache prefix reuse (`cache_prompt: true`).
pub fn build_system_prompt(
    target_lang: &str,
    style: StylePreset,
    glossary: &[GlossaryEntry],
) -> String {
    let style_desc = style.description();

    let glossary_block = if glossary.is_empty() {
        String::new()
    } else {
        let pairs: Vec<String> = glossary
            .iter()
            .filter(|g| !g.source.trim().is_empty() && !g.target.trim().is_empty())
            .map(|g| format!("{} -> {}", g.source.trim(), g.target.trim()))
            .collect();
        if pairs.is_empty() {
            String::new()
        } else {
            format!("优先遵守以下专有名词对照表：{}。", pairs.join("；"))
        }
    };

    format!(
        "你是一位顶尖的实时视频字幕同声传译专家。你的任务是将视频语音识别（ASR）产生的实时口语文本准确、自然、简洁地翻译为【{target_lang}】。\n\
\n\
必须严格遵守以下规则：\n\
1. 【纯净输出】只输出最终译文本身。严禁输出任何解释、思考过程、前缀标签（如“译文：”）、引号、括号注释或 Markdown 格式。\n\
2. 【仅译当前句】用户会提供 `<context>`（前文语境，仅供参考）和 `<current>`（当前待译句子）。你只能翻译 `<current>` 中的内容，绝对不要重复翻译 `<context>`。\n\
3. 【ASR 纠错与语境连贯】输入来自实时语音识别，可能存在同音错别字、漏标点或断句偏差。请结合 `<context>` 自动修正明显误识别，保持人名、专有名词、代词指代和语气前后一致。\n\
4. 【字幕级地道表达】译文应符合母语观众看字幕的习惯：自然口语化、精炼易读、去冗余语气词（如无意义的 uh/um/you know/那个），避免生硬的逐字机翻。\n\
5. 【残句顺译】若 `<current>` 语义未完结，仅就已出现的部分自然顺译，不要自行臆测或补全未说出的内容。\n\
6. 【同语种处理】如果 `<current>` 已经是目标语言，仅做轻微的 ASR 错字与标点顺滑后直接输出。\n\
7. 【风格与术语】当前视频场景风格：{style_desc}。{glossary_block}"
    )
}

/// Builds the XML-delimited User Prompt separating rolling context from the live utterance,
/// preventing the model from re-translating previous sentences.
pub fn build_user_prompt(context: &[ContextTurn], current_text: &str) -> String {
    let cleaned = current_text.trim();
    if context.is_empty() {
        return format!("<context>\n(无前文)\n</context>\n<current>\n{cleaned}\n</current>");
    }

    let mut ctx_lines = String::new();
    for (idx, turn) in context.iter().enumerate() {
        ctx_lines.push_str(&format!(
            "[{}] {} => {}\n",
            idx + 1,
            turn.source.trim(),
            turn.translated.trim()
        ));
    }

    format!(
        "<context>\n{}</context>\n<current>\n{cleaned}\n</current>",
        ctx_lines
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_contains_target_and_glossary() {
        let p = build_system_prompt(
            "简体中文",
            StylePreset::Tech,
            &[GlossaryEntry {
                source: "KV Cache".into(),
                target: "KV 缓存".into(),
            }],
        );
        assert!(p.contains("【简体中文】"));
        assert!(p.contains("KV Cache -> KV 缓存"));
        assert!(p.contains("<current>"));
    }

    #[test]
    fn user_prompt_isolates_current() {
        let u = build_user_prompt(
            &[ContextTurn {
                source: "Hello world".into(),
                translated: "你好世界".into(),
            }],
            "Let's dive in.",
        );
        assert!(u.contains("[1] Hello world => 你好世界"));
        assert!(u.contains("<current>\nLet's dive in.\n</current>"));
    }
}
