//! 英文 G2P（字素 → Kokoro 音素），不依赖 espeak-ng（GPL）。
//!
//! 查找顺序（参考 misaki 的英文流程，做了精简）：
//! 1. 上下文相关的虚词（the / a / an / am / to / in / I），与 misaki 规则一致；
//!    “下一个词是否元音开头”按下一个词的**音素**判断（the hour → ði，the university → ðə），
//!    因此整句从右往左处理；
//! 2. misaki 词典（gold 优先，其次 silver；Apache-2.0）；
//! 3. 词形变化：-'s / -s / -es / -ed / -ing，按词干末音选择后缀发音；
//! 4. 连字符拆分、纯数字转英文、全大写缩写逐字母读；
//! 5. 复合词拆分（两个词典词拼接）；
//! 6. 兜底：基于拼写规则的字母到音素转换（专有名词等未登录词）。
//!
//! 输出音素使用 misaki 记号（A=eɪ, I=aɪ, O=oʊ, W=aʊ, Y=ɔɪ, ᵊ, ᵻ …），与 Kokoro 词表一致。
//!
//! 连读：Kokoro 按整句合成，词与词之间的衔接、同化、省音由声学模型从真实语音中学到；
//! 这里只负责与训练时一致的音素输入——虚词用不带重音的形式（模型会弱读），
//! 闪音写作 `T`（见 [`to_model_phonemes`]）。

use std::collections::HashMap;
use std::path::Path;

use es_core::text::normalize::cardinal;
use es_core::{CoreError, Result};

/// 词典条目：一个词的音素（多词性条目取 DEFAULT）。
type Dict = HashMap<String, String>;

pub struct Lexicon {
    gold: Dict,
    silver: Dict,
    british: bool,
}

/// Kokoro 词表中的标点，原样保留以提供韵律信息。
pub const PUNCT: &[char] = &[
    ';', ':', ',', '.', '!', '?', '—', '…', '"', '(', ')', '“', '”',
];

const VOWEL_PHONEMES: &str = "aeiouæɑɐɒɔəɚɛɜɪʊʌAIOWYQᵊᵻ";

/// misaki 判断“下一个音是否元音”所用的集合。
const CTX_VOWELS: &str = "AIOQWYaiuæɑɒɔəɛɜɪʊʌᵻ";
const CTX_CONSONANTS: &str = "bdfhjklmnpstvwzðŋɡɹɾʃʒʤʧθT";
/// 打断连读上下文的标点（引号除外）。
const CTX_BREAKS: &[char] = &[';', ':', ',', '.', '!', '?', '—', '…', '(', ')'];

/// 后一个词的开头：`Some(true)` 元音，`Some(false)` 辅音，`None` 后面是停顿/句末。
pub type NextSound = Option<bool>;

/// 由一段音素（含标点）求它对前一个词的上下文；没有可判断的字符时返回 `fallback`。
pub fn next_sound(phonemes: &str, fallback: NextSound) -> NextSound {
    for c in phonemes.chars() {
        if CTX_BREAKS.contains(&c) {
            return None;
        }
        if CTX_VOWELS.contains(c) {
            return Some(true);
        }
        if CTX_CONSONANTS.contains(c) {
            return Some(false);
        }
    }
    fallback
}

/// Kokoro v1.0 的训练音素约定（misaki 非 2.0 版）：闪音 ɾ 写作 T，喉塞音 ʔ 写作 t。
pub fn to_model_phonemes(p: &str) -> String {
    p.replace('ɾ', "T").replace('ʔ', "t")
}

impl Lexicon {
    pub fn load(dir: &Path, british: bool) -> Result<Self> {
        let prefix = if british { "gb" } else { "us" };
        Ok(Self {
            gold: load_dict(&dir.join(format!("{prefix}_gold.json")))?,
            silver: load_dict(&dir.join(format!("{prefix}_silver.json")))?,
            british,
        })
    }

    #[cfg(test)]
    pub fn from_entries(entries: &[(&str, &str)], british: bool) -> Self {
        Self {
            gold: entries
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            silver: Dict::new(),
            british,
        }
    }

    fn lookup(&self, word: &str) -> Option<&str> {
        let lower = word.to_lowercase();
        let capitalized = capitalize(&lower);
        [&self.gold, &self.silver]
            .into_iter()
            .find_map(|d| {
                d.get(word)
                    .or_else(|| d.get(&lower))
                    .or_else(|| d.get(&capitalized))
            })
            .map(String::as_str)
    }

    /// 一个词（不含标点）的音素。`next` 为后一个词的开头音，用于虚词弱读（同 misaki）。
    pub fn word(&self, word: &str, next: NextSound) -> String {
        let word = word.replace(['’', '‘'], "'");
        let word = word.trim_matches('\'');
        if word.is_empty() {
            return String::new();
        }
        let gold = |w: &str| self.gold.get(w).cloned().unwrap_or_default();
        match word {
            "the" | "The" | "THE" => return if next == Some(true) { "ði" } else { "ðə" }.into(),
            // 冠词；单独出现在句末的 A 是字母
            "a" => return "ɐ".into(),
            "A" if next.is_some() => return "ɐ".into(),
            "an" | "An" => return "ɐn".into(),
            "am" if next.is_some() => return "ɐm".into(),
            "I" => return "ˌI".into(),
            "to" | "To" => {
                return match next {
                    None => gold("to"),
                    Some(false) => "tə".into(),
                    Some(true) => "tʊ".into(),
                }
            }
            // 介词 in 在句中不重读
            "in" | "In" => return if next.is_none() { "ˈɪn" } else { "ɪn" }.into(),
            _ => {}
        }
        self.resolve(word, 0)
            .unwrap_or_else(|| letters_to_sound(word))
    }

    fn resolve(&self, word: &str, depth: u8) -> Option<String> {
        if let Some(p) = self.lookup(word) {
            return Some(p.to_string());
        }
        if word.chars().all(|c| c.is_ascii_digit()) {
            let n: u64 = word.parse().ok()?;
            let spoken = cardinal(n);
            return Some(
                spoken
                    .split([' ', '-'])
                    .filter_map(|w| self.resolve(w, depth + 1))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        if word.contains('-') {
            let parts: Vec<String> = word
                .split('-')
                .filter(|p| !p.is_empty())
                .map(|p| {
                    self.resolve(p, depth + 1)
                        .unwrap_or_else(|| letters_to_sound(p))
                })
                .collect();
            return Some(parts.join(" "));
        }
        if let Some(p) = self.stem(word) {
            return Some(p);
        }
        if is_acronym(word) {
            return Some(self.spell(word));
        }
        if depth == 0 {
            return self.compound(word);
        }
        None
    }

    /// -'s / -s / -es / -ed / -ing
    fn stem(&self, word: &str) -> Option<String> {
        let lower = word.to_lowercase();
        if let Some(base) = lower.strip_suffix("'s") {
            return self
                .lookup(base)
                .map(|p| format!("{p}{}", self.s_suffix(p)));
        }
        if lower.len() > 3 && lower.ends_with('s') && !lower.ends_with("ss") {
            if let Some(p) = self.lookup(&lower[..lower.len() - 1]) {
                return Some(format!("{p}{}", self.s_suffix(p)));
            }
            if let Some(base) = lower.strip_suffix("es") {
                if let Some(p) = self.lookup(base) {
                    return Some(format!("{p}{}", self.sibilant_suffix()));
                }
            }
            if let Some(base) = lower.strip_suffix("ies") {
                if let Some(p) = self.lookup(&format!("{base}y")) {
                    return Some(format!("{p}z"));
                }
            }
        }
        if lower.len() > 4 && lower.ends_with("ed") {
            let base = &lower[..lower.len() - 2];
            let candidates = [base.to_string(), format!("{base}e"), undouble(base)];
            if let Some(p) = candidates.iter().find_map(|b| self.lookup(b)) {
                return Some(format!("{p}{}", self.ed_suffix(p)));
            }
            if let Some(b) = base.strip_suffix('i') {
                if let Some(p) = self.lookup(&format!("{b}y")) {
                    return Some(format!("{p}d"));
                }
            }
        }
        if lower.len() > 5 && lower.ends_with("ing") {
            let base = &lower[..lower.len() - 3];
            let candidates = [base.to_string(), format!("{base}e"), undouble(base)];
            if let Some(p) = candidates.iter().find_map(|b| self.lookup(b)) {
                return Some(format!("{p}ɪŋ"));
            }
        }
        None
    }

    fn s_suffix(&self, phonemes: &str) -> &'static str {
        match phonemes.chars().last() {
            Some('s' | 'z' | 'ʃ' | 'ʒ' | 'ʧ' | 'ʤ') => self.sibilant_suffix(),
            Some('p' | 't' | 'k' | 'f' | 'θ') => "s",
            _ => "z",
        }
    }

    fn sibilant_suffix(&self) -> &'static str {
        if self.british {
            "ɪz"
        } else {
            "ᵻz"
        }
    }

    fn ed_suffix(&self, phonemes: &str) -> &'static str {
        match phonemes.chars().last() {
            Some('t' | 'd') if self.british => "ɪd",
            Some('t' | 'd') => "ᵻd",
            Some('p' | 'k' | 'f' | 'θ' | 'ʃ' | 'ʧ' | 's') => "t",
            _ => "d",
        }
    }

    /// 逐字母读未登录的全大写缩写（如 GPU → ʤˈi pˈi jˈu）。
    fn spell(&self, word: &str) -> String {
        word.chars()
            .filter_map(|c| letter_name(c.to_ascii_uppercase(), self.british))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// 两个词典词拼接（如 "keyboard" 未登录时拆为 key + board），左侧取最长。
    fn compound(&self, word: &str) -> Option<String> {
        let lower = word.to_lowercase();
        if lower.len() < 6 || !lower.is_ascii() {
            return None;
        }
        (3..=lower.len() - 3).rev().find_map(|i| {
            let (a, b) = lower.split_at(i);
            Some(format!(
                "{}{}",
                self.lookup(a)?,
                strip_primary_stress(self.lookup(b)?)
            ))
        })
    }
}

fn load_dict(path: &Path) -> Result<Dict> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CoreError::Engine(format!("read {}: {e}", path.display())))?;
    let raw: HashMap<String, serde_json::Value> = serde_json::from_str(&text)
        .map_err(|e| CoreError::Engine(format!("parse {}: {e}", path.display())))?;
    Ok(raw
        .into_iter()
        .filter_map(|(k, v)| {
            let p = match v {
                serde_json::Value::String(s) => s,
                serde_json::Value::Object(m) => m
                    .get("DEFAULT")
                    .and_then(|d| d.as_str())
                    .or_else(|| m.values().find_map(|v| v.as_str()))?
                    .to_string(),
                _ => return None,
            };
            Some((k, p))
        })
        .collect())
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn undouble(base: &str) -> String {
    let b = base.as_bytes();
    if b.len() >= 2 && b[b.len() - 1] == b[b.len() - 2] {
        base[..base.len() - 1].to_string()
    } else {
        base.to_string()
    }
}

fn is_acronym(word: &str) -> bool {
    (2..=5).contains(&word.len()) && word.chars().all(|c| c.is_ascii_uppercase())
}

fn strip_primary_stress(p: &str) -> String {
    p.replacen('ˈ', "ˌ", 1)
}

fn letter_name(c: char, british: bool) -> Option<&'static str> {
    Some(match c {
        'A' => "ˈA",
        'B' => "bˈi",
        'C' => "sˈi",
        'D' => "dˈi",
        'E' => "ˈi",
        'F' => "ˈɛf",
        'G' => "ʤˈi",
        'H' => "ˈAʧ",
        'I' => "ˈI",
        'J' => "ʤˈA",
        'K' => "kˈA",
        'L' => "ˈɛl",
        'M' => "ˈɛm",
        'N' => "ˈɛn",
        'O' if british => "ˈQ",
        'O' => "ˈO",
        'P' => "pˈi",
        'Q' => "kjˈu",
        'R' if british => "ˈɑː",
        'R' => "ˈɑɹ",
        'S' => "ˈɛs",
        'T' => "tˈi",
        'U' => "jˈu",
        'V' => "vˈi",
        'W' => "dˈʌbᵊljˌu",
        'X' => "ˈɛks",
        'Y' => "wˈI",
        'Z' if british => "zˈɛd",
        'Z' => "zˈi",
        _ => return None,
    })
}

/// 兜底的拼写规则（未登录词，主要是人名地名）。贪心匹配最长字母组合。
pub fn letters_to_sound(word: &str) -> String {
    const RULES: &[(&str, &str)] = &[
        ("tion", "ʃən"),
        ("sion", "ʒən"),
        ("ture", "ʧəɹ"),
        ("ough", "ɔ"),
        ("augh", "ɔ"),
        ("tch", "ʧ"),
        ("sch", "sk"),
        ("igh", "I"),
        ("eau", "O"),
        ("th", "θ"),
        ("sh", "ʃ"),
        ("ch", "ʧ"),
        ("ph", "f"),
        ("wh", "w"),
        ("ck", "k"),
        ("ng", "ŋ"),
        ("qu", "kw"),
        ("dg", "ʤ"),
        ("gh", ""),
        ("ee", "i"),
        ("ea", "i"),
        ("oo", "u"),
        ("ou", "W"),
        ("ow", "O"),
        ("oi", "Y"),
        ("oy", "Y"),
        ("ai", "A"),
        ("ay", "A"),
        ("au", "ɔ"),
        ("aw", "ɔ"),
        ("oa", "O"),
        ("ie", "i"),
        ("ei", "A"),
        ("ey", "A"),
        ("ew", "ju"),
        ("ue", "u"),
        ("ar", "ɑɹ"),
        ("er", "əɹ"),
        ("ir", "ɜɹ"),
        ("ur", "ɜɹ"),
        ("or", "ɔɹ"),
    ];
    let w: Vec<char> = word
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    let s: String = w.iter().collect();
    let n = w.len();
    let is_vowel = |c: char| "aeiouy".contains(c);
    // 末尾不发音的 e 与“magic e”：元音 + 辅音 + e 结尾时前面的元音读长音
    let silent_e = n > 2 && w[n - 1] == 'e' && !is_vowel(w[n - 2]);
    let magic = if silent_e && n >= 3 && is_vowel(w[n - 3]) {
        Some(n - 3)
    } else {
        None
    };

    let mut out = String::new();
    let mut i = 0;
    while i < n {
        if silent_e && i == n - 1 {
            break;
        }
        if i == 0 && (s.starts_with("kn") || s.starts_with("wr")) {
            i += 1;
            continue;
        }
        if let Some((pat, ph)) = RULES.iter().find(|(p, _)| s[i..].starts_with(p)) {
            out.push_str(ph);
            i += pat.len();
            continue;
        }
        let c = w[i];
        let next = w.get(i + 1).copied();
        if i > 0 && w[i - 1] == c && !is_vowel(c) {
            i += 1; // 双写辅音只读一次
            continue;
        }
        let soft = next.is_some_and(|x| "eiy".contains(x));
        let ph = match c {
            'a' if magic == Some(i) => "A",
            'e' if magic == Some(i) => "i",
            'i' if magic == Some(i) => "I",
            'o' if magic == Some(i) => "O",
            'u' if magic == Some(i) => "ju",
            'a' => "æ",
            'e' => "ɛ",
            'i' => "ɪ",
            'o' => "ɑ",
            'u' => "ʌ",
            'y' if i == 0 => "j",
            'y' => "i",
            'c' if soft => "s",
            'c' | 'k' | 'q' => "k",
            'g' if soft => "ʤ",
            'g' => "ɡ",
            'j' => "ʤ",
            'r' => "ɹ",
            'x' => "ks",
            other => {
                out.push(other);
                i += 1;
                continue;
            }
        };
        out.push_str(ph);
        i += 1;
    }
    // 在第一个元音前加主重音
    match out
        .char_indices()
        .find(|(_, c)| VOWEL_PHONEMES.contains(*c))
    {
        Some((pos, _)) => format!("{}ˈ{}", &out[..pos], &out[pos..]),
        None => out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex() -> Lexicon {
        Lexicon::from_entries(
            &[
                ("hello", "həlˈO"),
                ("cat", "kˈæt"),
                ("watch", "wˈɑʧ"),
                ("jump", "ʤˈʌmp"),
                ("want", "wˈɑnt"),
                ("make", "mˈAk"),
                ("stop", "stˈɑp"),
                ("key", "kˈi"),
                ("board", "bˈɔɹd"),
                ("twenty", "twˈɛnti"),
                ("four", "fˈɔɹ"),
                ("to", "tu"),
            ],
            false,
        )
    }

    #[test]
    fn dictionary_and_context() {
        let l = lex();
        assert_eq!(l.word("Hello", None), "həlˈO");
        assert_eq!(l.word("the", Some(true)), "ði");
        assert_eq!(l.word("the", Some(false)), "ðə");
        assert_eq!(l.word("to", Some(true)), "tʊ");
        assert_eq!(l.word("to", Some(false)), "tə");
        assert_eq!(l.word("to", None), "tu", "句末保留词典读音");
        assert_eq!(l.word("in", Some(false)), "ɪn");
        assert_eq!(l.word("in", None), "ˈɪn");
    }

    #[test]
    fn context_follows_phonemes_not_spelling() {
        // hour 以元音音素开头，university 以辅音 j 开头
        assert_eq!(next_sound("ˈWəɹ", None), Some(true));
        assert_eq!(next_sound("jˌunəvˈɜɹsəɾi", None), Some(false));
        assert_eq!(next_sound(", ðə", Some(true)), None, "逗号打断连读");
        assert_eq!(next_sound("“ˈɑpəl", None), Some(true), "引号不打断");
        assert_eq!(to_model_phonemes("wˈɔɾəɹ"), "wˈɔTəɹ");
    }

    #[test]
    fn morphology() {
        let l = lex();
        assert_eq!(l.word("cats", None), "kˈæts");
        assert_eq!(l.word("watches", None), "wˈɑʧᵻz");
        assert_eq!(l.word("cat's", None), "kˈæts");
        assert_eq!(l.word("jumped", None), "ʤˈʌmpt");
        assert_eq!(l.word("wanted", None), "wˈɑntᵻd");
        assert_eq!(l.word("making", None), "mˈAkɪŋ");
        assert_eq!(l.word("stopped", None), "stˈɑpt");
    }

    #[test]
    fn numbers_hyphens_acronyms_compounds() {
        let l = lex();
        assert_eq!(l.word("twenty-four", None), "twˈɛnti fˈɔɹ");
        assert_eq!(l.word("24", None), "twˈɛnti fˈɔɹ");
        assert_eq!(l.word("GPU", None), "ʤˈi pˈi jˈu");
        assert_eq!(l.word("keyboard", None), "kˈibˌɔɹd");
    }

    #[test]
    fn fallback_rules() {
        assert_eq!(letters_to_sound("Kokoro"), "kˈɑkɔɹɑ");
        assert_eq!(letters_to_sound("Blake"), "blˈAk");
        assert_eq!(letters_to_sound("Thompson"), "θˈɑmpsɑn");
        assert!(
            !letters_to_sound("George").contains('g'),
            "词表里只有 ɡ，没有 ASCII g"
        );
    }
}
