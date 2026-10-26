//! Opus-MT 英 → 中机器翻译（Helsinki-NLP/opus-mt-en-zh，MarianMT，Apache-2.0 / CC-BY 4.0）。
//!
//! - 模型：Xenova 导出的 ONNX int8 量化版，编码器 + 带 KV 缓存的合并解码器（共约 108MB）；
//! - 分词：SentencePiece Unigram（tokenizer.json 中的词表与分数），这里自己做 Viterbi 切分，
//!   不引入 `tokenizers`（它的正则依赖在 iOS / Android 交叉编译时很麻烦）；
//! - 解码：贪心搜索 + KV 缓存，一句约几十毫秒（桌面）到几百毫秒（手机）。
//!
//! 模型目录：
//! ```text
//! <model_dir>/opus-mt-en-zh/
//!   encoder_model.onnx
//!   decoder_model_merged.onnx
//!   tokenizer.json
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use es_core::ports::Translator;
use es_core::{CoreError, Result};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::{Session, SessionInputValue};
use ort::value::{DynValue, Tensor};

const ENCODER: &str = "encoder_model.onnx";
const DECODER: &str = "decoder_model_merged.onnx";
const TOKENIZER: &str = "tokenizer.json";

const EOS: i64 = 0;
const UNK: i64 = 1;
/// 解码起始符，同时是 pad（生成时禁止输出）。
const PAD: i64 = 65000;
const LAYERS: usize = 6;
const HEADS: i64 = 8;
const HEAD_DIM: i64 = 64;
/// 模型最大长度 512；超长输入（极少，核心层已把句子切到 ≤300 字符）截断。
const MAX_INPUT: usize = 400;

fn mt_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Translation(e.to_string())
}

/// SentencePiece Unigram 分词（只做编码；解码直接查表）。
pub struct Unigram {
    pieces: HashMap<String, (i64, f32)>,
    id_to_piece: Vec<String>,
    unk_score: f32,
    max_piece_chars: usize,
}

impl Unigram {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(mt_err)?;
        let json: serde_json::Value = serde_json::from_str(&text).map_err(mt_err)?;
        let vocab = json["model"]["vocab"]
            .as_array()
            .ok_or_else(|| mt_err("tokenizer.json has no Unigram model.vocab"))?;
        let mut pieces = HashMap::with_capacity(vocab.len());
        let mut id_to_piece = Vec::with_capacity(vocab.len());
        let mut min_score = 0f32;
        let mut max_piece_chars = 1;
        for (id, entry) in vocab.iter().enumerate() {
            let piece = entry[0].as_str().unwrap_or_default().to_string();
            let score = entry[1].as_f64().unwrap_or(0.0) as f32;
            min_score = min_score.min(score);
            max_piece_chars = max_piece_chars.max(piece.chars().count());
            id_to_piece.push(piece.clone());
            // 特殊符号不参与切分
            if !piece.starts_with('<') || !piece.ends_with('>') {
                pieces.entry(piece).or_insert((id as i64, score));
            }
        }
        Ok(Self {
            pieces,
            id_to_piece,
            unk_score: min_score - 10.0,
            max_piece_chars: max_piece_chars.min(24),
        })
    }

    /// 与 Marian 的 SentencePiece 一致：按空白切词，每个词前加 ▁，再做 Viterbi 切分；末尾加 </s>。
    pub fn encode(&self, text: &str) -> Vec<i64> {
        let mut ids = Vec::new();
        for word in normalize(text).split_whitespace() {
            self.encode_word(&format!("▁{word}"), &mut ids);
        }
        ids.truncate(MAX_INPUT - 1);
        ids.push(EOS);
        ids
    }

    fn encode_word(&self, word: &str, out: &mut Vec<i64>) {
        let chars: Vec<(usize, char)> = word.char_indices().collect();
        let n = chars.len();
        let byte = |i: usize| chars.get(i).map_or(word.len(), |c| c.0);
        // best[i]：前 i 个字符的最优 (分数, 上一个切分点, 该段 id)
        let mut best: Vec<(f32, usize, i64)> = vec![(f32::NEG_INFINITY, 0, UNK); n + 1];
        best[0].0 = 0.0;
        for end in 1..=n {
            for start in end.saturating_sub(self.max_piece_chars)..end {
                if best[start].0 == f32::NEG_INFINITY {
                    continue;
                }
                let piece = &word[byte(start)..byte(end)];
                let (id, score) = match self.pieces.get(piece) {
                    Some(&(id, s)) => (id, s),
                    // 单个未登录字符兜底，保证总能切完
                    None if end - start == 1 => (UNK, self.unk_score),
                    None => continue,
                };
                let total = best[start].0 + score;
                if total > best[end].0 {
                    best[end] = (total, start, id);
                }
            }
        }
        let mut seg = Vec::new();
        let mut end = n;
        while end > 0 {
            let (_, start, id) = best[end];
            seg.push(id);
            end = start;
        }
        seg.reverse();
        // 连续的未登录字符合并为一个 <unk>（同 SentencePiece）
        seg.dedup_by(|a, b| *a == UNK && *b == UNK);
        out.extend(seg);
    }

    pub fn decode(&self, ids: &[i64]) -> String {
        let text: String = ids
            .iter()
            .filter(|&&id| id != EOS && id != PAD && id != UNK)
            .filter_map(|&id| self.id_to_piece.get(id as usize))
            .map(String::as_str)
            .collect();
        tidy_chinese(&text.replace('▁', " "))
    }
}

/// 训练语料按 NFKC 规范化过；这里只处理文章中常见的几类排版字符。
fn normalize(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '‘' | '’' | '′' => '\'',
            '“' | '”' | '″' => '"',
            '–' | '—' => '-',
            '\u{a0}' | '\u{2009}' | '\u{202f}' => ' ',
            c => c,
        })
        .collect()
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x3000..=0x303F | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF)
}

/// 去掉中文字符之间（含中文标点）多余的空格，保留中英混排处的空格。
fn tidy_chinese(text: &str) -> String {
    let chars: Vec<char> = text.trim().chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        if c == ' ' {
            let prev = out.chars().next_back();
            let next = chars[i + 1..].iter().find(|c| **c != ' ').copied();
            match (prev, next) {
                (Some(p), Some(n)) if is_cjk(p) && is_cjk(n) => continue,
                (Some(' '), _) | (None, _) | (_, None) => continue,
                _ => {}
            }
        }
        out.push(c);
    }
    out
}

struct Runtime {
    encoder: Mutex<Session>,
    decoder: Mutex<Session>,
    tokenizer: Unigram,
    _env: ort::environment::Environment,
}

/// Opus-MT 引擎。构造立即返回，会话在后台线程加载（约 1 秒），首次翻译时等待。
pub struct OpusMt {
    runtime: Arc<OnceLock<std::result::Result<Runtime, String>>>,
}

impl OpusMt {
    pub fn model_root(model_dir: &Path) -> PathBuf {
        model_dir.join("opus-mt-en-zh")
    }

    pub fn is_available(model_dir: &Path) -> bool {
        let root = Self::model_root(model_dir);
        [ENCODER, DECODER, TOKENIZER]
            .iter()
            .all(|f| root.join(f).is_file())
    }

    /// `threads`：每个会话的推理线程数。与 TTS 同时运行，移动端建议 2。
    pub fn load(model_dir: &Path, threads: usize) -> Result<Self> {
        let root = Self::model_root(model_dir);
        if !Self::is_available(model_dir) {
            return Err(mt_err(format!(
                "missing Opus-MT files under {}",
                root.display()
            )));
        }
        let runtime = Arc::new(OnceLock::new());
        let slot = runtime.clone();
        std::thread::Builder::new()
            .name("opus-mt-load".into())
            .spawn(move || {
                let t = std::time::Instant::now();
                let result = Runtime::load(&root, threads).map_err(|e| e.to_string());
                log::info!("opus-mt loaded in {:?}: ok={}", t.elapsed(), result.is_ok());
                let _ = slot.set(result);
            })
            .map_err(mt_err)?;
        Ok(Self { runtime })
    }

    fn runtime(&self) -> Result<&Runtime> {
        self.runtime.wait().as_ref().map_err(mt_err)
    }
}

fn session(env: &ort::environment::Environment, path: &Path, threads: usize) -> Result<Session> {
    Session::builder(env)
        .map_err(mt_err)?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(mt_err)?
        .with_intra_threads(threads.max(1))
        .map_err(mt_err)?
        .commit_from_file(path)
        .map_err(mt_err)
}

impl Runtime {
    fn load(root: &Path, threads: usize) -> Result<Self> {
        let env = es_onnx::shared_env().map_err(mt_err)?;
        Ok(Self {
            encoder: Mutex::new(session(&env, &root.join(ENCODER), threads)?),
            decoder: Mutex::new(session(&env, &root.join(DECODER), threads)?),
            tokenizer: Unigram::load(&root.join(TOKENIZER))?,
            _env: env,
        })
    }

    fn translate(&self, text: &str) -> Result<String> {
        let ids = self.tokenizer.encode(text);
        let n = ids.len();
        let mask = vec![1i64; n];

        let hidden: DynValue = {
            let mut enc = self.encoder.lock().unwrap();
            let mut out = enc
                .run(ort::inputs![
                    "input_ids" => Tensor::from_array((vec![1i64, n as i64], ids)).map_err(mt_err)?,
                    "attention_mask" => Tensor::from_array((vec![1i64, n as i64], mask.clone())).map_err(mt_err)?,
                ])
                .map_err(mt_err)?;
            out.remove("last_hidden_state")
                .ok_or_else(|| mt_err("encoder has no last_hidden_state"))?
        };

        let mut dec = self.decoder.lock().unwrap();
        // 每层 4 个缓存：decoder.key / decoder.value / encoder.key / encoder.value
        let mut past: Vec<DynValue> = Vec::with_capacity(LAYERS * 4);
        for _ in 0..LAYERS * 4 {
            past.push(
                Tensor::<f32>::from_array((vec![1i64, HEADS, 0, HEAD_DIM], Vec::<f32>::new()))
                    .map_err(mt_err)?
                    .into_dyn(),
            );
        }
        let max_len = (n * 3 + 10).min(512);
        let mut out_ids: Vec<i64> = Vec::new();
        let mut token = PAD;
        for step in 0..max_len {
            let mut inputs: Vec<(String, SessionInputValue<'_>)> = vec![
                (
                    "input_ids".into(),
                    Tensor::from_array((vec![1i64, 1], vec![token]))
                        .map_err(mt_err)?
                        .into(),
                ),
                (
                    "encoder_attention_mask".into(),
                    Tensor::from_array((vec![1i64, n as i64], mask.clone()))
                        .map_err(mt_err)?
                        .into(),
                ),
                (
                    "encoder_hidden_states".into(),
                    SessionInputValue::from(&hidden),
                ),
                (
                    "use_cache_branch".into(),
                    Tensor::from_array((vec![1i64], vec![step > 0]))
                        .map_err(mt_err)?
                        .into(),
                ),
            ];
            for (i, value) in past.iter().enumerate() {
                inputs.push((
                    past_name("past_key_values", i),
                    SessionInputValue::from(value),
                ));
            }
            let mut outputs = dec.run(inputs).map_err(mt_err)?;
            let (_, logits) = outputs["logits"]
                .try_extract_tensor::<f32>()
                .map_err(mt_err)?;
            token = argmax(logits);
            // 交叉注意力的缓存只在第一步计算；之后的 present.*.encoder 不更新，沿用第一步的
            let mut next = Vec::with_capacity(LAYERS * 4);
            for (i, old) in past.iter_mut().enumerate() {
                let is_encoder = i % 4 >= 2;
                if step > 0 && is_encoder {
                    next.push(std::mem::replace(
                        old,
                        Tensor::<f32>::from_array((vec![0i64], Vec::<f32>::new()))
                            .map_err(mt_err)?
                            .into_dyn(),
                    ));
                } else {
                    next.push(
                        outputs
                            .remove(past_name("present", i))
                            .ok_or_else(|| mt_err("decoder output missing present"))?,
                    );
                }
            }
            drop(outputs);
            past = next;
            if token == EOS {
                break;
            }
            out_ids.push(token);
            if looping(&out_ids) {
                break;
            }
        }
        Ok(polish(text, &self.tokenizer.decode(&out_ids)))
    }
}

/// 标点整理：中文语境里的半角标点换成全角，合并重复的句末标点，补上原文有而译文漏掉的句末标点。
fn polish(source: &str, text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let prev = out.chars().next_back();
        let next = chars[i + 1..].iter().find(|c| **c != ' ').copied();
        let full = match c {
            ',' => Some('，'),
            '?' => Some('？'),
            '!' => Some('！'),
            ':' => Some('：'),
            ';' => Some('；'),
            _ => None,
        };
        let chinese_context =
            prev.is_some_and(is_cjk) || next.is_some_and(is_cjk) || next.is_none();
        match full {
            Some(f)
                if chinese_context && prev.is_some_and(|p| !p.is_ascii_digit() || f != '：') =>
            {
                out.push(f);
                // 全角标点自带间距，去掉其后的空格
                while chars.get(i + 1) == Some(&' ') {
                    i += 1;
                }
            }
            _ => out.push(c),
        }
        i += 1;
    }
    // 合并重复的句末标点（“。。。” → “。”），保留省略号“……”
    let mut merged = String::with_capacity(out.len());
    for c in out.chars() {
        if matches!(c, '。' | '！' | '？' | '，') && merged.ends_with(c) {
            continue;
        }
        merged.push(c);
    }
    let end = match source.trim_end().chars().next_back() {
        Some('.') => Some('。'),
        Some('?') => Some('？'),
        Some('!') => Some('！'),
        _ => None,
    };
    let last = merged.chars().next_back();
    if let (Some(end), Some(last)) = (end, last) {
        if is_cjk(last) && !matches!(last, '。' | '！' | '？' | '…' | '」' | '”' | '）') {
            merged.push(end);
        }
    }
    merged
}

fn past_name(prefix: &str, i: usize) -> String {
    let kind = [
        "decoder.key",
        "decoder.value",
        "encoder.key",
        "encoder.value",
    ][i % 4];
    format!("{prefix}.{}.{kind}", i / 4)
}

/// 贪心：取分数最高的 token，跳过 pad（generation_config 中的 bad_words_ids）。
fn argmax(logits: &[f32]) -> i64 {
    let mut best = (EOS, f32::NEG_INFINITY);
    for (id, &v) in logits.iter().enumerate() {
        if id as i64 != PAD && v > best.1 {
            best = (id as i64, v);
        }
    }
    best.0
}

/// 贪心解码偶尔陷入重复（“的的的…” / 同一短语反复）：末尾同一片段连续出现 3 次就停。
fn looping(ids: &[i64]) -> bool {
    (1..=8).any(|len| {
        ids.len() >= len * 3 && {
            let tail = &ids[ids.len() - len..];
            (1..3).all(|k| &ids[ids.len() - len * (k + 1)..ids.len() - len * k] == tail)
        }
    })
}

impl Translator for OpusMt {
    fn id(&self) -> &str {
        "opus-mt-en-zh"
    }

    fn translate(&self, text: &str) -> Result<String> {
        if text.trim().is_empty() {
            return Ok(String::new());
        }
        self.runtime()?.translate(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tidies_spaces_between_chinese() {
        assert_eq!(
            tidy_chinese(" 我们 今天 去 Apple 商店 。 "),
            "我们今天去 Apple 商店。"
        );
        assert_eq!(tidy_chinese("使用 GPT 4  模型"), "使用 GPT 4 模型");
    }

    #[test]
    fn polishes_punctuation() {
        assert_eq!(polish("Hello, world.", "你好,世界。。。"), "你好，世界。");
        assert_eq!(polish("I'd like coffee.", "我想喝咖啡"), "我想喝咖啡。");
        assert_eq!(polish("Why?", "为什么?"), "为什么？");
        assert_eq!(polish("At 10:30, go.", "10:30, 走"), "10:30，走。");
        assert_eq!(polish("Use GPT-4, ok", "使用 GPT-4, 好"), "使用 GPT-4，好");
    }

    #[test]
    fn detects_loops() {
        assert!(looping(&[5, 1, 2, 1, 2, 1, 2]));
        assert!(looping(&[7, 7, 7]));
        assert!(!looping(&[1, 2, 3, 1, 2]));
    }
}
