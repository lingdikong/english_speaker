//! 英文分句。

use std::ops::Range;

/// 句点后不应断句的缩写（小写，不含句点）。
const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "etc", "e.g", "i.e", "inc", "ltd",
    "co", "corp", "u.s", "u.k", "no", "fig", "approx", "jan", "feb", "mar", "apr", "jun", "jul",
    "aug", "sep", "sept", "oct", "nov", "dec",
];

const CLOSERS: &[char] = &['"', '\'', ')', ']', '”', '’'];

/// 返回段落中每个句子的字节区间（已去掉首尾空白）。
pub fn sentences(para: &str) -> Vec<Range<usize>> {
    let chars: Vec<(usize, char)> = para.char_indices().collect();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < chars.len() {
        let (pos, c) = chars[i];
        if matches!(c, '.' | '!' | '?' | '…') {
            // 吞掉连续的终止符和右引号/括号
            let mut j = i + 1;
            while j < chars.len()
                && (matches!(chars[j].1, '.' | '!' | '?') || CLOSERS.contains(&chars[j].1))
            {
                j += 1;
            }
            let at_end = j >= chars.len();
            let next_ok = at_end || (chars[j].1.is_whitespace() && starts_sentence(&chars[j..]));
            if next_ok && !(c == '.' && is_abbreviation(&para[start..pos])) {
                let end = if at_end { para.len() } else { chars[j].0 };
                push_trimmed(para, start..end, &mut out);
                start = end;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    push_trimmed(para, start..para.len(), &mut out);
    out
}

/// 下一个非空白字符是否像句子开头。
fn starts_sentence(rest: &[(usize, char)]) -> bool {
    rest.iter()
        .map(|(_, c)| *c)
        .find(|c| !c.is_whitespace())
        .is_some_and(|c| {
            c.is_uppercase() || c.is_ascii_digit() || matches!(c, '"' | '“' | '\'' | '(' | '‘')
        })
}

fn is_abbreviation(before: &str) -> bool {
    let word = before
        .rsplit(|c: char| c.is_whitespace() || c == '(' || c == '"')
        .next()
        .unwrap_or("");
    let lower = word.to_lowercase();
    // 单个大写字母：人名缩写 "J. K. Rowling"
    if word.len() == 1 && word.chars().all(|c| c.is_ascii_uppercase()) {
        return true;
    }
    ABBREVIATIONS.contains(&lower.as_str())
}

fn push_trimmed(s: &str, r: Range<usize>, out: &mut Vec<Range<usize>>) {
    let slice = &s[r.clone()];
    let lead = slice.len() - slice.trim_start().len();
    let trail = slice.len() - slice.trim_end().len();
    if lead + trail < slice.len() {
        out.push(r.start + lead..r.end - trail);
    }
}

/// 把过长的句子在 `; : ,` 或空格处切成不超过 `max` 字符的片段。
pub fn split_long(sentence: &str, max: usize) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = sentence.trim();
    while rest.chars().count() > max {
        let limit = rest
            .char_indices()
            .nth(max)
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        let window = &rest[..limit];
        let cut = [';', ':', ',']
            .iter()
            .filter_map(|p| window.rfind(*p).map(|i| i + 1))
            .filter(|&i| i > limit / 3)
            .max()
            .or_else(|| window.rfind(' '))
            .unwrap_or(limit);
        out.push(rest[..cut].trim());
        rest = rest[cut..].trim_start();
    }
    if !rest.is_empty() {
        out.push(rest);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(p: &str) -> Vec<&str> {
        sentences(p).into_iter().map(|r| &p[r]).collect()
    }

    #[test]
    fn handles_abbreviations_and_quotes() {
        assert_eq!(
            split("Mr. J. K. Rowling arrived. \"Hello!\" she said. Done"),
            vec!["Mr. J. K. Rowling arrived.", "\"Hello!\" she said.", "Done"]
        );
    }

    #[test]
    fn decimals_do_not_split() {
        assert_eq!(
            split("It costs 3.5 dollars. Yes."),
            vec!["It costs 3.5 dollars.", "Yes."]
        );
    }

    #[test]
    fn long_sentence_is_split_at_comma() {
        let s = "aaaa aaaa, bbbb bbbb cccc";
        assert_eq!(split_long(s, 15), vec!["aaaa aaaa,", "bbbb bbbb cccc"]);
    }
}
