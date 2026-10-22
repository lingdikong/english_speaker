//! 文本流水线：原文 → 段落 → 句子 → 词元（带朗读形式）。
//!
//! 这一层与具体 TTS 模型无关；G2P（字素→音素）属于模型相关逻辑，放在引擎里。

pub mod connected;
pub mod ingest;
pub mod normalize;
pub mod segment;

pub use normalize::Token;

/// 单次送入 TTS 的最大字符数。Kokoro 单次输入上限约 510 个音素 token，
/// 300 字符左右的英文留有余量。
pub const MAX_CHUNK_CHARS: usize = 300;

#[derive(Debug, Clone)]
pub struct PlannedSentence {
    pub paragraph: u32,
    /// 原文（用于显示与高亮）。
    pub text: String,
    pub tokens: Vec<Token>,
}

impl PlannedSentence {
    /// 送入 TTS 的朗读文本。
    pub fn spoken(&self) -> String {
        self.tokens
            .iter()
            .map(|t| t.spoken.as_str())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn spoken_words(&self) -> Vec<String> {
        self.tokens.iter().map(|t| t.spoken.clone()).collect()
    }
}

/// 导入时估算朗读总时长（每个字母/数字约 62ms，加句间停顿），合成完成前用于进度条。
pub fn estimate_ms(plan: &[PlannedSentence]) -> u64 {
    let chars: usize = plan.iter().flat_map(|s| &s.tokens).map(Token::weight).sum();
    chars as u64 * 62 + plan.len() as u64 * 300
}

/// 把整篇正文规划成按顺序合成的句子列表。
pub fn plan(body: &str) -> Vec<PlannedSentence> {
    let mut out = Vec::new();
    for (p_idx, para) in ingest::paragraphs(body).iter().enumerate() {
        for range in segment::sentences(para) {
            for chunk in segment::split_long(&para[range], MAX_CHUNK_CHARS) {
                let tokens = normalize::tokenize(chunk);
                if tokens.iter().all(|t| t.spoken.is_empty()) {
                    continue;
                }
                out.push(PlannedSentence {
                    paragraph: p_idx as u32,
                    text: chunk.to_string(),
                    tokens,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_splits_paragraphs_and_sentences() {
        let body = "# Title\n\nDr. Smith paid $5 for it. It was 50% off!\n\nThe end.";
        let p = plan(body);
        let texts: Vec<_> = p.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(
            texts,
            vec![
                "Title",
                "Dr. Smith paid $5 for it.",
                "It was 50% off!",
                "The end."
            ]
        );
        assert_eq!(p[1].spoken(), "doctor Smith paid five dollars for it.");
        assert_eq!(p[2].spoken(), "It was fifty percent off!");
        assert_eq!(p[3].paragraph, 2);
    }
}
