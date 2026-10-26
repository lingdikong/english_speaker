//! ECDICT 离线词典（skywind3000/ECDICT，MIT）。
//!
//! 随包分发裁剪后的核心词库（约 6 万词、7MB，见 `scripts/build-dict.py`），只读打开，查一次 < 1ms。
//! 查变形词时还原到原形：perceived → perceive（过去式 / 过去分词）；但本身就是常用词的变形
//! （left 左边的、better 更好的）直接给出它自己的释义。
//!
//! ```text
//! <model_dir>/dict/ecdict.db
//! ```

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use es_core::model::{DictEntry, DictSense, WordForm};
use es_core::ports::Dictionary;
use es_core::{CoreError, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension};

const FILE: &str = "ecdict.db";
const MAX_SENSES: usize = 8;

fn dict_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Translation(format!("dictionary: {e}"))
}

pub struct EcDict {
    conn: Mutex<Connection>,
}

struct Row {
    word: String,
    phonetic: String,
    translation: String,
    tags: String,
    collins: u8,
    oxford: bool,
    frq: u32,
    exchange: String,
}

impl EcDict {
    pub fn path(model_dir: &Path) -> PathBuf {
        model_dir.join("dict").join(FILE)
    }

    pub fn is_available(model_dir: &Path) -> bool {
        Self::path(model_dir).is_file()
    }

    pub fn open(model_dir: &Path) -> Result<Self> {
        Self::open_file(&Self::path(model_dir))
    }

    /// 只读 + immutable：安装包目录（iOS Bundle）不可写，不能让 SQLite 尝试建日志文件。
    pub fn open_file(path: &Path) -> Result<Self> {
        let uri = format!("file:{}?immutable=1", uri_path(path));
        let conn = Connection::open_with_flags(
            uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(dict_err)?;
        conn.query_row("SELECT count(*) FROM words", [], |r| r.get::<_, i64>(0))
            .map_err(dict_err)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn row(&self, key: &str) -> Result<Option<Row>> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT word, phonetic, translation, tags, collins, oxford, frq, exchange
                 FROM words WHERE key = ?1",
                [key],
                |r| {
                    Ok(Row {
                        word: r.get(0)?,
                        phonetic: r.get(1)?,
                        translation: r.get(2)?,
                        tags: r.get(3)?,
                        collins: r.get(4)?,
                        oxford: r.get::<_, i64>(5)? != 0,
                        frq: r.get(6)?,
                        exchange: r.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(dict_err)
    }

    fn lemma_of(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .lock()
            .unwrap()
            .query_row("SELECT lemma FROM forms WHERE form = ?1", [key], |r| {
                r.get(0)
            })
            .optional()
            .map_err(dict_err)
    }
}

/// SQLite URI 里的路径：`%`、`?`、`#` 与空格需要转义。Windows 路径用正斜杠并加前导 `/`。
fn uri_path(path: &Path) -> String {
    let mut p = path.to_string_lossy().replace('\\', "/");
    if p.as_bytes().get(1) == Some(&b':') {
        p.insert(0, '/');
    }
    p.replace('%', "%25")
        .replace('?', "%3f")
        .replace('#', "%23")
        .replace(' ', "%20")
}

fn form_label(code: char) -> Option<&'static str> {
    Some(match code {
        'p' => "过去式",
        'd' => "过去分词",
        'i' => "现在分词",
        '3' => "第三人称单数",
        's' => "复数",
        'r' => "比较级",
        't' => "最高级",
        _ => return None,
    })
}

fn tag_label(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "zk" => "中考",
        "gk" => "高考",
        "cet4" => "四级",
        "cet6" => "六级",
        "ky" => "考研",
        "toefl" => "托福",
        "ielts" => "雅思",
        "gre" => "GRE",
        _ => return None,
    })
}

/// exchange 字段（`p:ran/i:running/...`）里的变形，同一个词合并标签（过去式 / 过去分词）。
fn forms(exchange: &str, lemma: &str) -> Vec<WordForm> {
    let mut out: Vec<WordForm> = Vec::new();
    for part in exchange.split('/') {
        let Some((code, word)) = part.split_once(':') else {
            continue;
        };
        let (Some(c), None) = (code.chars().next(), code.chars().nth(1)) else {
            continue;
        };
        let Some(label) = form_label(c) else { continue };
        let word = word.trim();
        if word.is_empty() || word.eq_ignore_ascii_case(lemma) {
            continue;
        }
        match out.iter_mut().find(|f| f.word == word) {
            Some(f) => {
                f.kind.push_str(" / ");
                f.kind.push_str(label);
            }
            None => out.push(WordForm {
                kind: label.into(),
                word: word.into(),
            }),
        }
    }
    out
}

/// 释义行：`vt. 感觉, 认知` / `[计] 记号` / `run的过去式和过去分词`。
fn senses(translation: &str) -> Vec<DictSense> {
    translation
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|line| {
            let (pos, meaning) = split_pos(line);
            DictSense {
                pos: pos.to_string(),
                meaning: meaning.replace(", ", "，").replace(',', "，"),
            }
        })
        .filter(|s| !s.meaning.is_empty())
        .take(MAX_SENSES)
        .collect()
}

fn split_pos(line: &str) -> (&str, &str) {
    if let Some(rest) = line.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            return (&line[..end + 2], line[end + 2..].trim());
        }
    }
    let head = line.split_whitespace().next().unwrap_or("");
    let is_pos = head.len() <= 6
        && head.ends_with('.')
        && head[..head.len() - 1]
            .chars()
            .all(|c| c.is_ascii_lowercase() || matches!(c, '&' | '/' | '.'));
    if is_pos && head.len() > 1 {
        (head, line[head.len()..].trim())
    } else {
        ("", line)
    }
}

fn entry(row: Row, query: &str, inflection: Option<String>) -> DictEntry {
    let tags = row
        .tags
        .split_whitespace()
        .filter_map(tag_label)
        .map(String::from)
        .collect();
    DictEntry {
        forms: forms(&row.exchange, &row.word),
        senses: senses(&row.translation),
        word: row.word,
        query: query.to_string(),
        inflection,
        phonetic: row.phonetic,
        tags,
        collins: row.collins,
        oxford: row.oxford,
        frequency: (row.frq > 0).then_some(row.frq),
    }
}

/// `query` 是原形的哪种变形（“过去式 / 过去分词”）。
fn inflection_of(lemma: &Row, query: &str) -> Option<String> {
    forms(&lemma.exchange, &lemma.word)
        .into_iter()
        .find(|f| f.word.eq_ignore_ascii_case(query))
        .map(|f| f.kind)
}

/// 词典没收的变形按规则猜原形（-s / -es / -ies / -ed / -ing / 's）。
fn guesses(key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |s: String| {
        if s.len() >= 2 && !out.contains(&s) {
            out.push(s);
        }
    };
    if let Some(s) = key.strip_suffix("'s").or_else(|| key.strip_suffix("’s")) {
        push(s.to_string());
    }
    if let Some(s) = key.strip_suffix("ies") {
        push(format!("{s}y"));
    }
    if let Some(s) = key.strip_suffix("es") {
        push(s.to_string());
    }
    if let Some(s) = key.strip_suffix('s') {
        push(s.to_string());
    }
    if let Some(s) = key.strip_suffix("ied") {
        push(format!("{s}y"));
    }
    if let Some(s) = key.strip_suffix("ed") {
        push(s.to_string());
        push(format!("{s}e"));
    }
    if let Some(s) = key.strip_suffix("ing") {
        push(s.to_string());
        push(format!("{s}e"));
        // running → run
        let b = s.as_bytes();
        if b.len() >= 2 && b[b.len() - 1] == b[b.len() - 2] {
            push(s[..s.len() - 1].to_string());
        }
    }
    out
}

impl Dictionary for EcDict {
    fn lookup(&self, word: &str) -> Result<Option<DictEntry>> {
        let key = word.trim().replace('’', "'").to_lowercase();
        if key.is_empty() {
            return Ok(None);
        }
        let direct = self.row(&key)?;
        let lemma_key = match self.lemma_of(&key)? {
            Some(l) => Some(l),
            None => direct.as_ref().and_then(|r| {
                r.exchange
                    .split('/')
                    .find_map(|p| p.strip_prefix("0:"))
                    .map(|l| l.trim().to_lowercase())
                    .filter(|l| *l != key)
            }),
        };
        // 本身就是常用词（有星级 / 牛津 / 考纲标签）的变形，给它自己的释义
        let standalone = direct
            .as_ref()
            .is_some_and(|r| r.collins > 0 || r.oxford || !r.tags.is_empty());
        if !standalone {
            if let Some(lemma) = lemma_key
                .as_deref()
                .map(|l| self.row(l))
                .transpose()?
                .flatten()
            {
                let inflection = inflection_of(&lemma, &key);
                return Ok(Some(entry(lemma, word.trim(), inflection)));
            }
        }
        if let Some(row) = direct {
            return Ok(Some(entry(row, word.trim(), None)));
        }
        for guess in guesses(&key) {
            if let Some(row) = self.row(&guess)? {
                return Ok(Some(entry(row, word.trim(), None)));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_senses_and_pos() {
        let s = senses("vt. 感觉, 认知\n[计] 记号\nrun的过去式和过去分词\nn.&v. 混合");
        assert_eq!(
            s[0],
            DictSense {
                pos: "vt.".into(),
                meaning: "感觉，认知".into()
            }
        );
        assert_eq!(s[1].pos, "[计]");
        assert_eq!(s[2].pos, "");
        assert_eq!(s[3].pos, "n.&v.");
    }

    #[test]
    fn merges_form_labels() {
        let f = forms("0:leave/1:dp/d:left/p:left/i:leaving", "leave");
        assert_eq!(
            f[0],
            WordForm {
                kind: "过去分词 / 过去式".into(),
                word: "left".into()
            }
        );
        assert_eq!(f[1].word, "leaving");
    }

    #[test]
    fn guesses_lemmas() {
        assert!(guesses("running").contains(&"run".to_string()));
        assert!(guesses("studies").contains(&"study".to_string()));
        assert!(guesses("hoped").contains(&"hope".to_string()));
    }

    #[test]
    fn escapes_uri_paths() {
        assert_eq!(uri_path(Path::new("/a b/c?#%.db")), "/a%20b/c%3f%23%25.db");
        assert_eq!(uri_path(Path::new("C:\\x\\d.db")), "/C:/x/d.db");
    }
}
