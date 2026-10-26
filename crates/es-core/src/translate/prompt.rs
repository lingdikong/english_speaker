//! 智能模式（本地大模型）的提示词、少样本示例、输出语法与结果解析。
//!
//! 面向 0.5B 级别的小模型，实测得出的做法：
//! - **少样本胜过说明**：每类任务给 1–2 轮示例对话，模型照着示例的格式与粒度回答；
//! - **不给上文**：附上一句原文时，0.5B 模型经常去翻上文（实测 5 句错 3 句），所以整句翻译只看本句；
//! - **机器译文作参考**：整句精翻与句子解析附上 Opus-MT 的译文。不给参考时译文更顺，
//!   但会凭空添加内容（“放在冰箱里”）；给了参考，模型在其上修正标点、术语与语序，稳定得多；
//! - **GBNF 约束结构**：词性限定为枚举、释义限长，JSON 结构必然合法；解析失败时调用方退回词典。

use serde::Deserialize;

use crate::model::{DictEntry, ExampleSentence, GrammarPoint, SentenceAnalysis, WordInsight};
use crate::ports::LlmRequest;

/// 一次生成请求的全部内容（示例为静态常量）。
pub struct Prompt {
    pub system: &'static str,
    pub shots: &'static [(&'static str, &'static str)],
    pub user: String,
    pub grammar: Option<String>,
    pub max_tokens: u32,
}

impl Prompt {
    pub fn request(&self) -> LlmRequest<'_> {
        LlmRequest {
            system: self.system,
            shots: self.shots,
            user: &self.user,
            max_tokens: self.max_tokens,
            grammar: self.grammar.as_deref(),
        }
    }
}

const TRANSLATE_SYSTEM: &str = "你是专业的英译中翻译。把“原文”翻译成自然、流畅的简体中文。\
有“术语”时必须使用指定译法；可以参考“机器译文”，但要改正其中的错误，不要添加原文没有的内容。只输出译文。";

const TRANSLATE_SHOTS: &[(&str, &str)] = &[
    (
        "原文：It was expensive, but he loved it.\n机器译文：这是昂贵的，但他爱它。",
        "它挺贵的，但他很喜欢。",
    ),
    (
        "术语：app = 应用\n原文：Open the app and sign in with your email.\n机器译文：打开程序，用你的电子邮件签名。",
        "打开应用，用你的邮箱登录。",
    ),
];

/// 整句翻译。`reference` 为机器译文，`glossary` 为 `term = 译法` 的多行文本。
pub fn translate(sentence: &str, reference: Option<&str>, glossary: &str) -> Prompt {
    let mut user = String::new();
    if !glossary.is_empty() {
        user.push_str("术语：");
        user.push_str(&glossary.lines().collect::<Vec<_>>().join("；"));
        user.push('\n');
    }
    user.push_str("原文：");
    user.push_str(sentence.trim());
    if let Some(r) = reference.map(str::trim).filter(|r| !r.is_empty()) {
        user.push_str("\n机器译文：");
        user.push_str(r);
    }
    Prompt {
        system: TRANSLATE_SYSTEM,
        shots: TRANSLATE_SHOTS,
        user,
        grammar: None,
        max_tokens: (sentence.len() as u32 * 2 + 32).min(384),
    }
}

/// 清理模型输出的译文：取第一段，去掉引号与“译文：”之类的前缀。
pub fn clean_translation(raw: &str) -> String {
    let text = raw
        .trim()
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let text = ["译文：", "译文:", "翻译：", "翻译:", "中文：", "中文:"]
        .iter()
        .find_map(|p| text.strip_prefix(p))
        .unwrap_or(text)
        .trim();
    text.trim_matches(|c| matches!(c, '"' | '“' | '”' | '「' | '」'))
        .trim()
        .to_string()
}

const WORD_SYSTEM: &str = "你是耐心的英语老师。根据句子，说明“单词”在这句话里的词性和意思（从词典释义里选最贴切的一个，简短），\
再给 3 个包含这个词的常用搭配和 2 个新的地道例句（不要照抄原句）。只输出 JSON。";

const WORD_SHOTS: &[(&str, &str)] = &[(
    "句子：She caught the last train home.\n单词：caught\n词典：vt. 捕捉，抓住；赶上；感染",
    r#"{"pos":"v.","meaning":"赶上（车）","collocations":["catch a train","catch a cold","catch up with"],"examples":[{"en":"We ran to catch the bus.","zh":"我们跑着去赶公交车。"},{"en":"I caught a cold last week.","zh":"我上周感冒了。"}]}"#,
)];

pub fn word(word: &str, sentence: &str, dict: Option<&DictEntry>) -> Prompt {
    let mut user = format!("句子：{}\n单词：{}", sentence.trim(), word);
    if let Some(d) = dict.filter(|d| !d.senses.is_empty()) {
        let senses: Vec<String> = d
            .senses
            .iter()
            .filter(|s| !s.pos.starts_with('['))
            .take(4)
            .map(|s| format!("{} {}", s.pos, s.meaning).trim().to_string())
            .collect();
        user.push_str("\n词典：");
        user.push_str(&senses.join("；"));
    }
    Prompt {
        system: WORD_SYSTEM,
        shots: WORD_SHOTS,
        user,
        grammar: Some(word_grammar()),
        max_tokens: 320,
    }
}

const SENTENCE_SYSTEM: &str = "你是耐心的英语老师，帮中国学生读懂英文句子。给出通顺的意译（可参考机器译文并改正错误）、\
一句话说明句子主干与从句结构、1 到 3 个值得学习的短语或语法点（text 为英文原文片段，note 为中文讲解）。只输出 JSON。";

const SENTENCE_SHOTS: &[(&str, &str)] = &[(
    "句子：Although it was raining, they decided to go for a walk.\n机器译文：虽然下雨，他们决定去散步。",
    r#"{"translation":"虽然在下雨，他们还是决定去散散步。","structure":"主句 they decided to go for a walk，前面是 although 引导的让步状语从句。","points":[{"text":"Although it was raining","note":"although 引导让步状语从句，意为“尽管”，不能再和 but 连用。"},{"text":"go for a walk","note":"固定搭配，去散步。"}]}"#,
)];

pub fn sentence(sentence: &str, reference: Option<&str>, glossary: &str) -> Prompt {
    let mut user = format!("句子：{}", sentence.trim());
    if let Some(r) = reference.map(str::trim).filter(|r| !r.is_empty()) {
        user.push_str("\n机器译文：");
        user.push_str(r);
    }
    if !glossary.is_empty() {
        user.push_str("\n术语：");
        user.push_str(&glossary.lines().collect::<Vec<_>>().join("；"));
    }
    Prompt {
        system: SENTENCE_SYSTEM,
        shots: SENTENCE_SHOTS,
        user,
        grammar: Some(sentence_grammar()),
        max_tokens: 480,
    }
}

/// 公共 GBNF 规则：JSON 字符串（不含控制字符）、限长短字符串、有限的空白（避免无限空白循环）。
const GBNF_COMMON: &str = r#"
str ::= "\"" chr{1,160} "\""
short ::= "\"" chr{1,24} "\""
chr ::= [^"\\\x00-\x1f] | "\\" ["\\/nt]
ws ::= [ \n]?
"#;

fn word_grammar() -> String {
    format!(
        r#"root ::= "{{" ws "\"pos\":" ws pos "," ws "\"meaning\":" ws short "," ws "\"collocations\":" ws strs "," ws "\"examples\":" ws exs ws "}}"
pos ::= "\"" ( "n." | "v." | "adj." | "adv." | "prep." | "conj." | "pron." | "num." | "art." | "int." | "aux." | "phr." ) "\""
strs ::= "[" ws str ( "," ws str ){{0,3}} ws "]"
exs ::= "[" ws ex ( "," ws ex ){{0,2}} ws "]"
ex ::= "{{" ws "\"en\":" ws str "," ws "\"zh\":" ws str ws "}}"
{GBNF_COMMON}"#
    )
}

fn sentence_grammar() -> String {
    format!(
        r#"root ::= "{{" ws "\"translation\":" ws str "," ws "\"structure\":" ws str "," ws "\"points\":" ws pts ws "}}"
pts ::= "[" ws pt ( "," ws pt ){{0,2}} ws "]"
pt ::= "{{" ws "\"text\":" ws str "," ws "\"note\":" ws str ws "}}"
{GBNF_COMMON}"#
    )
}

#[derive(Deserialize)]
struct RawWord {
    #[serde(default)]
    pos: String,
    #[serde(default)]
    meaning: String,
    #[serde(default)]
    collocations: Vec<String>,
    #[serde(default)]
    examples: Vec<RawExample>,
}

#[derive(Deserialize)]
struct RawExample {
    #[serde(default)]
    en: String,
    #[serde(default)]
    zh: String,
}

#[derive(Deserialize)]
struct RawSentence {
    #[serde(default)]
    translation: String,
    #[serde(default)]
    structure: String,
    #[serde(default)]
    points: Vec<RawPoint>,
}

#[derive(Deserialize)]
struct RawPoint {
    #[serde(default)]
    text: String,
    #[serde(default)]
    note: String,
}

/// 从模型输出中截取 JSON 对象（容忍前后多余的文字）。
fn json_slice(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (end > start).then(|| &raw[start..=end])
}

fn non_empty(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in items {
        let s = s.trim().to_string();
        if !s.is_empty() && !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

pub fn parse_word(word: &str, raw: &str) -> Option<WordInsight> {
    let r: RawWord = serde_json::from_str(json_slice(raw)?).ok()?;
    let meaning = r.meaning.trim().to_string();
    if meaning.is_empty() {
        return None;
    }
    Some(WordInsight {
        word: word.to_string(),
        pos: r.pos.trim().to_string(),
        meaning,
        // 搭配须包含这个词（或其变形的词干），过滤掉模型凑数的无关词
        collocations: non_empty(r.collocations)
            .into_iter()
            .filter(|c| c.contains(' ') && related(c, word))
            .collect(),
        examples: r
            .examples
            .into_iter()
            .map(|e| ExampleSentence {
                en: e.en.trim().to_string(),
                zh: e.zh.trim().to_string(),
            })
            .filter(|e| !e.en.is_empty() && !e.zh.is_empty())
            .collect(),
    })
}

/// 搭配里有没有这个词：比较前 4 个字母（caught / catch 这类不规则变形放过，不强求）。
fn related(phrase: &str, word: &str) -> bool {
    let w = word.to_lowercase();
    let stem: String = w.chars().take(4.min(w.chars().count())).collect();
    let p = phrase.to_lowercase();
    p.split_whitespace().any(|t| t.starts_with(&stem)) || stem.chars().count() < 3 || w.len() <= 4
}

/// 句子解析：语法点必须是原句里的英文片段（小模型有时把中文译文当成片段）。
pub fn parse_sentence(sentence: &str, raw: &str) -> Option<SentenceAnalysis> {
    let r: RawSentence = serde_json::from_str(json_slice(raw)?).ok()?;
    let translation = r.translation.trim().to_string();
    if translation.is_empty() {
        return None;
    }
    Some(SentenceAnalysis {
        translation,
        structure: r.structure.trim().to_string(),
        points: r
            .points
            .into_iter()
            .map(|p| GrammarPoint {
                text: p.text.trim().to_string(),
                note: p.note.trim().to_string(),
            })
            .filter(|p| !p.text.is_empty() && !p.note.is_empty())
            .filter(|p| sentence.to_lowercase().contains(&p.text.to_lowercase()))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_translation_noise() {
        assert_eq!(
            clean_translation("译文：“你好，世界。”\n\nHello"),
            "你好，世界。"
        );
        assert_eq!(clean_translation("  我们走吧。 "), "我们走吧。");
    }

    #[test]
    fn parses_word_json_with_noise() {
        let raw = r#"好的：{"pos":"v.","meaning":"察觉","collocations":["perceive sth as"," ","perceive","make sense"],"examples":[{"en":"I perceived a change.","zh":"我察觉到变化。"}]}"#;
        let w = parse_word("perceive", raw).unwrap();
        assert_eq!(w.meaning, "察觉");
        assert_eq!(w.collocations, ["perceive sth as"]);
        assert_eq!(w.examples.len(), 1);
        assert!(parse_word("x", r#"{"pos":"n.","meaning":" "}"#).is_none());
        assert!(parse_word("x", "not json").is_none());
    }

    #[test]
    fn parses_sentence_json() {
        let raw = r#"{"translation":"意译","structure":"主谓宾","points":[{"text":"pick up","note":"拾起"},{"text":"","note":"x"},{"text":"拾起","note":"x"}]}"#;
        let a = parse_sentence("Pick up the box.", raw).unwrap();
        assert_eq!(a.points.len(), 1);
    }

    #[test]
    fn translate_prompt_labels_terms_and_reference() {
        let p = translate(" Hi. ", Some("你好。"), "app = 应用\ntoken = 令牌");
        assert_eq!(
            p.user,
            "术语：app = 应用；token = 令牌\n原文：Hi.\n机器译文：你好。"
        );
        assert!(p.grammar.is_none());
        assert!(word("x", "y", None).grammar.unwrap().contains("root ::="));
    }
}
