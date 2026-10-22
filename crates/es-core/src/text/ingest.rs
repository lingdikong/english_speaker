//! 原文清洗：Markdown 去标记、分段、提取标题。
//!
//! HTML/PDF/EPUB 的正文抽取放在独立模块（后续以 feature 形式加入），
//! 输出统一为纯文本后再进入这里。

/// 推断标题：首个非空段落，截断到 80 个字符。
pub fn derive_title(raw: &str) -> String {
    let first = paragraphs(raw).into_iter().next().unwrap_or_default();
    let mut title: String = first.chars().take(80).collect();
    if first.chars().count() > 80 {
        title.push('…');
    }
    if title.is_empty() {
        "Untitled".into()
    } else {
        title
    }
}

/// 按空行分段；若全文没有空行，则每个非空行视为一段。段内换行合并为空格。
pub fn paragraphs(raw: &str) -> Vec<String> {
    let text = raw.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<String> = text.lines().map(clean_markdown_line).collect();
    let has_blank = lines.iter().any(|l| l.trim().is_empty());

    let mut out = Vec::new();
    let mut cur = String::new();
    for line in &lines {
        let line = line.trim();
        if line.is_empty() || !has_blank {
            flush(&mut cur, &mut out);
            if !line.is_empty() {
                out.push(line.to_string());
            }
            continue;
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(line);
    }
    flush(&mut cur, &mut out);
    out
}

fn flush(cur: &mut String, out: &mut Vec<String>) {
    if !cur.trim().is_empty() {
        out.push(std::mem::take(cur));
    }
    cur.clear();
}

/// 去掉一行中常见的 Markdown 标记。
fn clean_markdown_line(line: &str) -> String {
    let mut s = line.trim_start();
    // 标题、引用、列表符号
    s = s.trim_start_matches('#').trim_start_matches('>');
    for bullet in ["- ", "* ", "+ "] {
        if let Some(rest) = s.strip_prefix(bullet) {
            s = rest;
        }
    }
    let s = s.trim_start();
    if s.chars().all(|c| matches!(c, '-' | '*' | '_' | '=' | ' ')) {
        return String::new(); // 分隔线
    }
    let s = strip_links(s);
    s.replace("**", "").replace("__", "").replace('`', "")
}

/// `![alt](url)` → 删除；`[text](url)` → `text`。
fn strip_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        let is_image = open > 0 && rest[..open].ends_with('!');
        let Some(close) = rest[open..].find("](").map(|i| open + i) else {
            break;
        };
        let Some(end) = rest[close..].find(')').map(|i| close + i) else {
            break;
        };
        out.push_str(&rest[..if is_image { open - 1 } else { open }]);
        if !is_image {
            out.push_str(&rest[open + 1..close]);
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_is_cleaned() {
        let p = paragraphs("## Hello **world**\n\nSee [docs](http://x.y) ![img](a.png)now.\n---\n");
        assert_eq!(p, vec!["Hello world", "See docs now."]);
    }

    #[test]
    fn lines_without_blank_become_paragraphs() {
        assert_eq!(paragraphs("a.\nb."), vec!["a.", "b."]);
    }

    #[test]
    fn wrapped_lines_are_joined() {
        assert_eq!(paragraphs("one\ntwo\n\nthree"), vec!["one two", "three"]);
    }
}
