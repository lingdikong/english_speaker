//! 术语表：在句子里找出命中的术语，并在机器翻译前后做占位保护。
//!
//! - 智能模式（Qwen）：命中的术语写进提示词，由模型遵守；
//! - 机器翻译（Opus-MT）：模型不认提示词，先把术语换成占位符再翻译，译完换回固定译法。
//!   占位符在译文里丢失或被改写时，调用方退回不带保护的普通翻译。

use crate::model::GlossaryTerm;

/// 句中命中的术语（整词匹配，不区分大小写），按出现位置排序，不重叠，长术语优先。
pub fn matches<'a>(
    text: &str,
    glossary: &'a [GlossaryTerm],
) -> Vec<(usize, usize, &'a GlossaryTerm)> {
    let lower = text.to_lowercase();
    // to_lowercase 可能改变字节长度（极少见的字符）；此时放弃匹配，保证区间对得上原文
    if lower.len() != text.len() {
        return Vec::new();
    }
    let mut terms: Vec<&GlossaryTerm> = glossary
        .iter()
        .filter(|t| !t.term.trim().is_empty())
        .collect();
    terms.sort_by_key(|t| std::cmp::Reverse(t.term.len()));
    let mut found: Vec<(usize, usize, &GlossaryTerm)> = Vec::new();
    for t in terms {
        let needle = t.term.trim().to_lowercase();
        let mut from = 0;
        while let Some(pos) = lower[from..].find(&needle) {
            let start = from + pos;
            let end = start + needle.len();
            from = start + needle.len().max(1);
            if !is_boundary(&lower, start, end) {
                continue;
            }
            if found.iter().any(|&(s, e, _)| start < e && s < end) {
                continue;
            }
            found.push((start, end, t));
        }
    }
    found.sort_by_key(|&(s, ..)| s);
    found
}

fn is_boundary(text: &str, start: usize, end: usize) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let before = text[..start].chars().next_back().is_none_or(|c| !word(c));
    let after = text[end..].chars().next().is_none_or(|c| !word(c));
    before && after
}

/// 用于机器翻译的占位保护结果。
pub struct Protected {
    pub text: String,
    /// (占位符, 固定译法)
    pub slots: Vec<(String, String)>,
}

/// 把命中的术语换成占位符。占位符选用模型会原样照抄的专有名词式写法（实测见 es-translate 测试）。
pub fn protect(text: &str, glossary: &[GlossaryTerm]) -> Option<Protected> {
    let hits = matches(text, glossary);
    if hits.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut slots = Vec::new();
    let mut last = 0;
    for (i, (start, end, term)) in hits.into_iter().enumerate() {
        out.push_str(&text[last..start]);
        let slot = placeholder(i);
        out.push_str(&slot);
        slots.push((slot, term.translation.clone()));
        last = end;
    }
    out.push_str(&text[last..]);
    Some(Protected { text: out, slots })
}

fn placeholder(i: usize) -> String {
    format!("XQ{}", (b'A' + (i % 26) as u8) as char)
}

/// 把译文里的占位符换回固定译法；任一占位符缺失时返回 `None`。
pub fn restore(translated: &str, slots: &[(String, String)]) -> Option<String> {
    let mut out = translated.to_string();
    for (slot, zh) in slots {
        if !out.contains(slot.as_str()) {
            return None;
        }
        out = out.replace(slot.as_str(), zh);
    }
    Some(out)
}

/// 给智能模式提示词用的术语清单（每行 `term = 译法`）。
pub fn prompt_lines(text: &str, glossary: &[GlossaryTerm]) -> String {
    let mut seen = Vec::new();
    for (_, _, t) in matches(text, glossary) {
        if !seen
            .iter()
            .any(|s: &&GlossaryTerm| s.term.eq_ignore_ascii_case(&t.term))
        {
            seen.push(t);
        }
    }
    seen.iter()
        .map(|t| format!("{} = {}", t.term.trim(), t.translation.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(term: &str, zh: &str) -> GlossaryTerm {
        GlossaryTerm {
            term: term.into(),
            translation: zh.into(),
            created_at_ms: 0,
        }
    }

    #[test]
    fn matches_whole_words_case_insensitively() {
        let gl = [
            g("token", "令牌"),
            g("App", "应用"),
            g("access token", "访问令牌"),
        ];
        let text = "Tokens differ: the access token lets the app in.";
        let hits: Vec<_> = matches(text, &gl)
            .into_iter()
            .map(|(s, e, t)| (&text[s..e], t.translation.as_str()))
            .collect();
        assert_eq!(hits, [("access token", "访问令牌"), ("app", "应用")]);
    }

    #[test]
    fn protect_and_restore_round_trip() {
        let gl = [g("Kokoro", "心音"), g("app", "应用")];
        let p = protect("Kokoro powers this app.", &gl).unwrap();
        assert_eq!(p.text, "XQA powers this XQB.");
        assert_eq!(
            restore("XQA 驱动着这个 XQB。", &p.slots).unwrap(),
            "心音 驱动着这个 应用。"
        );
        assert!(restore("驱动着这个应用。", &p.slots).is_none());
        assert!(protect("nothing here", &gl).is_none());
    }

    #[test]
    fn prompt_lines_dedupes() {
        let gl = [g("token", "令牌")];
        assert_eq!(prompt_lines("Token after token.", &gl), "token = 令牌");
    }
}
