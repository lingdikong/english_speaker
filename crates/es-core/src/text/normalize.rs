//! 文本规范化：把原文词元转换为可朗读形式（数字、货币、缩写、URL…）。
//!
//! 按词元处理，保留每个词元在原文中的位置，这样高亮显示的是原文
//! （`$5`），朗读的是规范化文本（`five dollars`）。

/// 原文中的一个词元。
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// 在句子中的字节区间（含前后标点，用于高亮）。
    pub start: usize,
    pub end: usize,
    /// 朗读形式，保留句末标点供 TTS 处理韵律。可能为空（纯符号）。
    pub spoken: String,
}

impl Token {
    /// 估算朗读时长的权重。
    pub fn weight(&self) -> usize {
        self.spoken
            .chars()
            .filter(|c| c.is_alphanumeric())
            .count()
            .max(1)
    }
}

const LEADING: &[char] = &['"', '\'', '(', '[', '“', '‘', '*', '_'];
const TRAILING: &[char] = &[
    '"', '\'', ')', ']', '”', '’', '.', ',', ';', ':', '!', '?', '…', '*', '_',
];
const PROSODY: &[char] = &['.', ',', ';', ':', '!', '?'];

pub fn tokenize(sentence: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut offset = 0;
    for chunk in sentence.split_whitespace() {
        let start = sentence[offset..]
            .find(chunk)
            .map(|i| i + offset)
            .unwrap_or(offset);
        let end = start + chunk.len();
        offset = end;

        let core = chunk.trim_start_matches(LEADING);
        let core_trimmed = core.trim_end_matches(TRAILING);
        let trailing = &core[core_trimmed.len()..];
        // "Dr." "e.g." 这类缩写的句点属于词本身
        let (word, trailing) = match expand_abbreviation(core_trimmed, trailing) {
            Some(expanded) => (expanded, trailing.trim_start_matches('.')),
            None => (speak_word(core_trimmed), trailing),
        };
        let punct: String = trailing
            .chars()
            .filter(|c| PROSODY.contains(c))
            .take(1)
            .collect();
        let spoken = if word.is_empty() {
            punct
        } else {
            format!("{word}{punct}")
        };
        out.push(Token { start, end, spoken });
    }
    out
}

fn expand_abbreviation(word: &str, trailing: &str) -> Option<String> {
    if !trailing.starts_with('.') && !word.contains('.') {
        return None;
    }
    let lower = word.to_lowercase();
    let s = match lower.trim_end_matches('.') {
        "mr" => "mister",
        "mrs" => "missus",
        "ms" => "miz",
        "dr" => "doctor",
        "prof" => "professor",
        "jr" => "junior",
        "sr" => "senior",
        "vs" => "versus",
        "etc" => "et cetera",
        "e.g" => "for example",
        "i.e" => "that is",
        "approx" => "approximately",
        _ => return None,
    };
    Some(s.to_string())
}

fn speak_word(w: &str) -> String {
    if w.is_empty() {
        return String::new();
    }
    if w == "&" {
        return "and".into();
    }
    if w.chars().all(|c| !c.is_alphanumeric()) {
        return String::new();
    }
    if let Some(url) = speak_url(w) {
        return url;
    }
    if let Some(rest) = w.strip_prefix('$') {
        if let Some(amount) = speak_amount(rest) {
            return format!("{amount} dollars");
        }
    }
    if let Some(num) = w.strip_suffix('%') {
        if let Some(n) = speak_number(num) {
            return format!("{n} percent");
        }
    }
    if let Some((a, b)) = w.split_once(['-', '–']) {
        if let (Some(a), Some(b)) = (speak_number(a), speak_number(b)) {
            return format!("{a} to {b}");
        }
    }
    if let Some(o) = speak_ordinal(w) {
        return o;
    }
    if let Some(n) = speak_number(w) {
        return n;
    }
    w.to_string()
}

fn speak_url(w: &str) -> Option<String> {
    let rest = w
        .strip_prefix("https://")
        .or_else(|| w.strip_prefix("http://"))
        .or_else(|| w.strip_prefix("www.").map(|_| w))?;
    let host = rest
        .split('/')
        .next()
        .unwrap_or(rest)
        .trim_start_matches("www.");
    Some(host.split('.').collect::<Vec<_>>().join(" dot "))
}

/// `3.5M` → `three point five million`
fn speak_amount(s: &str) -> Option<String> {
    let (num, scale) = match s.chars().last()? {
        'k' | 'K' => (&s[..s.len() - 1], Some("thousand")),
        'm' | 'M' => (&s[..s.len() - 1], Some("million")),
        'b' | 'B' => (&s[..s.len() - 1], Some("billion")),
        _ => (s, None),
    };
    let n = speak_number(num)?;
    Some(match scale {
        Some(sc) => format!("{n} {sc}"),
        None => n,
    })
}

/// 整数、带千分位的整数、小数、年份。
fn speak_number(s: &str) -> Option<String> {
    if s.is_empty() || !s.chars().next()?.is_ascii_digit() {
        return None;
    }
    if s.len() == 4 && s.chars().all(|c| c.is_ascii_digit()) {
        let n: u64 = s.parse().ok()?;
        if (1100..=2099).contains(&n) {
            return Some(year(n));
        }
    }
    let (int_part, frac) = match s.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (s, None),
    };
    let digits: String = int_part.chars().filter(|&c| c != ',').collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) || digits.len() > 15 {
        return None;
    }
    let mut out = cardinal(digits.parse().ok()?);
    if let Some(f) = frac {
        if f.is_empty() || !f.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        out.push_str(" point");
        for d in f.chars() {
            out.push(' ');
            out.push_str(ONES[d.to_digit(10)? as usize]);
        }
    }
    Some(out)
}

fn speak_ordinal(s: &str) -> Option<String> {
    let lower = s.to_lowercase();
    let num = ["st", "nd", "rd", "th"]
        .iter()
        .find_map(|suf| lower.strip_suffix(suf))?;
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(ordinal(num.parse().ok()?))
}

const ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

fn two_digits(n: u64) -> String {
    if n < 20 {
        ONES[n as usize].to_string()
    } else if n.is_multiple_of(10) {
        TENS[(n / 10) as usize].to_string()
    } else {
        format!("{}-{}", TENS[(n / 10) as usize], ONES[(n % 10) as usize])
    }
}

fn under_thousand(n: u64) -> String {
    match (n / 100, n % 100) {
        (0, r) => two_digits(r),
        (h, 0) => format!("{} hundred", ONES[h as usize]),
        (h, r) => format!("{} hundred {}", ONES[h as usize], two_digits(r)),
    }
}

pub fn cardinal(n: u64) -> String {
    if n == 0 {
        return "zero".into();
    }
    const SCALES: [(u64, &str); 4] = [
        (1_000_000_000_000, "trillion"),
        (1_000_000_000, "billion"),
        (1_000_000, "million"),
        (1_000, "thousand"),
    ];
    let mut parts = Vec::new();
    let mut rest = n;
    for (value, name) in SCALES {
        if rest >= value {
            parts.push(format!("{} {name}", cardinal(rest / value)));
            rest %= value;
        }
    }
    if rest > 0 {
        parts.push(under_thousand(rest));
    }
    parts.join(" ")
}

fn year(n: u64) -> String {
    match n {
        2000..=2009 => cardinal(n),
        _ => {
            let (hi, lo) = (n / 100, n % 100);
            match lo {
                0 => format!("{} hundred", two_digits(hi)),
                1..=9 => format!("{} oh {}", two_digits(hi), ONES[lo as usize]),
                _ => format!("{} {}", two_digits(hi), two_digits(lo)),
            }
        }
    }
}

pub fn ordinal(n: u64) -> String {
    let c = cardinal(n);
    let split = c.rfind([' ', '-']).map(|i| i + 1).unwrap_or(0);
    let (head, last) = c.split_at(split);
    let last = match last {
        "one" => "first".to_string(),
        "two" => "second".to_string(),
        "three" => "third".to_string(),
        "five" => "fifth".to_string(),
        "eight" => "eighth".to_string(),
        "nine" => "ninth".to_string(),
        "twelve" => "twelfth".to_string(),
        w if w.ends_with('y') => format!("{}ieth", &w[..w.len() - 1]),
        w => format!("{w}th"),
    };
    format!("{head}{last}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spoken(s: &str) -> Vec<String> {
        tokenize(s).into_iter().map(|t| t.spoken).collect()
    }

    #[test]
    fn numbers() {
        assert_eq!(
            cardinal(1_234_567),
            "one million two hundred thirty-four thousand five hundred sixty-seven"
        );
        assert_eq!(year(1905), "nineteen oh five");
        assert_eq!(year(2024), "twenty twenty-four");
        assert_eq!(year(2005), "two thousand five");
        assert_eq!(ordinal(21), "twenty-first");
        assert_eq!(ordinal(40), "fortieth");
        assert_eq!(ordinal(12), "twelfth");
    }

    #[test]
    fn tokens() {
        assert_eq!(
            spoken("In 1999, Dr. Lee raised $3.5M (about 20%) on the 3rd try, e.g. 10-20 deals & more."),
            vec![
                "In", "nineteen ninety-nine,", "doctor", "Lee", "raised",
                "three point five million dollars", "about", "twenty percent", "on", "the",
                "third", "try,", "for example", "ten to twenty", "deals", "and", "more.",
            ]
        );
    }

    #[test]
    fn urls_and_offsets() {
        let t = tokenize("Visit https://www.example.com/a now");
        assert_eq!(t[1].spoken, "example dot com");
        assert_eq!((t[2].start, t[2].end), (32, 35));
    }
}
