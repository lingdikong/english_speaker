//! 连读标注：根据每个词的音素，标出词与词之间的连读、虚词弱读、闪音、省音，供学习者跟读。
//!
//! 输入音素使用 misaki / Kokoro 记号（A=eɪ, I=aɪ, O=oʊ, Q=əʊ, W=aʊ, Y=ɔɪ, T=闪音 …）。
//! 这是**教学用的规则分析**：标出的是标准口语中“通常会”发生的现象，帮助学习者听和模仿；
//! 实际音频中的连读由 TTS 模型整句合成时自然产生，两者大体一致，但不保证逐处吻合。

use serde::{Deserialize, Serialize};

use crate::model::SentenceSpan;

/// 一个词与**下一个词**之间的连读方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Link {
    /// 辅音 + 元音：pick‿it‿up
    ConsonantVowel,
    /// 元音 + 元音，中间带出 /j/：see‿it（/siːjɪt/）
    GlideJ,
    /// 元音 + 元音，中间带出 /w/：go‿on（/ɡoʊwɑn/）
    GlideW,
    /// 英式连接 r：far‿away（/fɑːrəweɪ/）
    LinkingR,
    /// 相同辅音只发一次、略延长：big‿game
    SameSound,
    /// 美式跨词闪音：get‿it（t 读得像轻快的 d，/ɡɛɾɪt/）
    Flap,
    /// 同化：t/d + you → /ʧ/ /ʤ/：did‿you（/dɪʤu/）
    Assimilation,
}

/// 一个词的标注。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WordNote {
    /// 词典读音（标准 IPA，用于显示）。
    pub ipa: String,
    /// 与下一个词的连读。
    pub link: Option<Link>,
    /// 虚词弱读形式（IPA），如 and → ən。
    pub weak_form: Option<String>,
    /// 词内有美式闪音（water、better）。
    pub flap: bool,
    /// 词尾 t/d 在辅音前通常省略（next‿day → nex day）。
    pub elided: bool,
}

/// misaki 记号中的元音 / 辅音。
pub const VOWELS: &str = "AIOQWYaiuæɑɒɔəɛɜɪʊʌᵻᵊɐ";
pub const CONSONANTS: &str = "bdfhjklmnpstvwzðŋɡɹɾʃʒʤʧθT";
/// 打断连读的标点（引号不打断）。
const BREAKS: &[char] = &[',', '.', ';', ':', '!', '?', '—', '…', '(', ')'];

/// 常见虚词的弱读形式（美式；英式去掉卷舌 r）。
const WEAK_FORMS: &[(&str, &str)] = &[
    ("a", "ə"),
    ("an", "ən"),
    ("and", "ən"),
    ("are", "ər"),
    ("as", "əz"),
    ("at", "ət"),
    ("but", "bət"),
    ("can", "kən"),
    ("could", "kəd"),
    ("do", "də"),
    ("does", "dəz"),
    ("for", "fər"),
    ("from", "frəm"),
    ("had", "əd"),
    ("has", "əz"),
    ("have", "əv"),
    ("her", "ər"),
    ("him", "ɪm"),
    ("of", "əv"),
    ("or", "ər"),
    ("should", "ʃəd"),
    ("some", "səm"),
    ("than", "ðən"),
    ("them", "ðəm"),
    ("the", "ðə"),
    ("to", "tə"),
    ("was", "wəz"),
    ("were", "wər"),
    ("would", "wəd"),
    ("you", "jə"),
    ("your", "jər"),
];

/// 为一句话的每个词元做标注。
///
/// `display[i]` 为原文词元（含标点，用于判断停顿），`phonemes[i]` 为该词元的音素（可能含多个词）。
pub fn annotate(display: &[&str], phonemes: &[String], british: bool) -> Vec<WordNote> {
    let n = display.len().min(phonemes.len());
    let sounds: Vec<Vec<char>> = phonemes[..n].iter().map(|p| sounds(p)).collect();
    (0..n)
        .map(|i| {
            let word = core_word(display[i]).to_lowercase();
            // 能与下一个词连读：下一个词存在、有音，且中间没有停顿
            let next = (i + 1 < n
                && !sounds[i + 1].is_empty()
                && !ends_with_break(display[i])
                && !display[i + 1].starts_with(['(', '—']))
            .then(|| &sounds[i + 1]);

            let weak_form = next
                .filter(|_| !phonemes[i].contains('ˈ'))
                .and_then(|_| WEAK_FORMS.iter().find(|(w, _)| *w == word))
                .map(|(_, f)| {
                    if british {
                        f.replace('r', "")
                    } else {
                        f.to_string()
                    }
                });
            let link = next.and_then(|nx| link_between(&sounds[i], nx, &word, british));
            let elided = weak_form.is_none()
                && link.is_none()
                && next.is_some_and(|nx| elides(&sounds[i], nx));
            WordNote {
                ipa: to_ipa(&phonemes[i], british),
                link,
                weak_form,
                flap: !british && (phonemes[i].contains('T') || phonemes[i].contains('ɾ')),
                elided,
            }
        })
        .collect()
}

fn link_between(cur: &[char], next: &[char], spelling: &str, british: bool) -> Option<Link> {
    let (&last, &first) = (cur.last()?, next.first()?);
    let is_vowel = |c: char| VOWELS.contains(c);
    let before_last = cur.len().checked_sub(2).map(|i| cur[i]);
    let td = |c: char| matches!(c, 't' | 'd' | 'T');

    if td(last) && first == 'j' {
        return Some(Link::Assimilation);
    }
    if !is_vowel(last) && last == first {
        return Some(Link::SameSound);
    }
    if !is_vowel(first) {
        return None;
    }
    if !is_vowel(last) {
        let after_vowel = before_last.is_some_and(|c| is_vowel(c) || c == 'ɹ');
        return Some(if !british && td(last) && after_vowel {
            Link::Flap
        } else {
            Link::ConsonantVowel
        });
    }
    // 元音 + 元音
    match last {
        'i' | 'I' | 'A' | 'Y' => Some(Link::GlideJ),
        'u' | 'O' | 'W' | 'Q' => Some(Link::GlideW),
        _ if british && spelling.ends_with(['r', 'e']) && spelling.contains('r') => {
            Some(Link::LinkingR)
        }
        _ => None,
    }
}

/// 辅音丛结尾的 t/d 在辅音前省略：next day、last night、old man。
fn elides(cur: &[char], next: &[char]) -> bool {
    let n = cur.len();
    n >= 2
        && matches!(cur[n - 1], 't' | 'd')
        && CONSONANTS.contains(cur[n - 2])
        && !matches!(cur[n - 2], 'ɹ' | 'l')
        && next
            .first()
            .is_some_and(|c| CONSONANTS.contains(*c) && !matches!(c, 'h' | 'j' | 'w'))
}

/// 去掉重音、标点、空格后的音段序列。
fn sounds(p: &str) -> Vec<char> {
    p.chars()
        .filter(|c| VOWELS.contains(*c) || CONSONANTS.contains(*c))
        .collect()
}

fn core_word(display: &str) -> &str {
    display.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
}

fn ends_with_break(display: &str) -> bool {
    display
        .trim_end_matches(['"', '\'', '”', '’'])
        .ends_with(BREAKS)
}

/// 纯文本渲染：连读处用 ‿ 连接原文（`Pick‿it‿up and put‿it‿on the table.`）。
/// 平台 UI 应使用 `WordSpan::notes` 的结构化字段自行渲染（颜色、弱读淡化、点按看读音）。
pub fn render_links(span: &SentenceSpan) -> String {
    let text = &span.text;
    let mut out = String::with_capacity(text.len() + 16);
    let mut pos = 0;
    for (i, w) in span.words.iter().enumerate() {
        let end = w.char_end as usize;
        out.push_str(&text[pos..end]);
        pos = end;
        if let (Some(_), Some(next)) = (w.notes.link, span.words.get(i + 1)) {
            out.push('‿');
            pos = next.char_start as usize;
        }
    }
    out.push_str(&text[pos..]);
    out
}

/// misaki 记号 → 标准 IPA（显示用）。
pub fn to_ipa(p: &str, british: bool) -> String {
    let mut out = String::with_capacity(p.len() + 8);
    for c in p.chars() {
        match c {
            'A' => out.push_str("eɪ"),
            'I' => out.push_str("aɪ"),
            'O' if british => out.push_str("əʊ"),
            'O' => out.push_str("oʊ"),
            'Q' => out.push_str("əʊ"),
            'W' => out.push_str("aʊ"),
            'Y' => out.push_str("ɔɪ"),
            'T' => out.push('ɾ'),
            'ᵊ' => out.push('ə'),
            'ᵻ' => out.push('ɪ'),
            c if VOWELS.contains(c)
                || CONSONANTS.contains(c)
                || matches!(c, 'ˈ' | 'ˌ' | 'ː' | ' ') =>
            {
                out.push(c)
            }
            _ => {} // 标点
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notes(pairs: &[(&str, &str)], british: bool) -> Vec<WordNote> {
        let display: Vec<&str> = pairs.iter().map(|p| p.0).collect();
        let phonemes: Vec<String> = pairs.iter().map(|p| p.1.to_string()).collect();
        annotate(&display, &phonemes, british)
    }

    fn links(pairs: &[(&str, &str)], british: bool) -> Vec<Option<Link>> {
        notes(pairs, british).into_iter().map(|n| n.link).collect()
    }

    #[test]
    fn consonant_vowel_and_flap() {
        // Pick it up and put it on the table.
        let n = notes(
            &[
                ("Pick", "pˈɪk"),
                ("it", "ɪt"),
                ("up", "ˌʌp"),
                ("and", "ænd"),
                ("put", "pˌʊt"),
                ("it", "ɪt"),
                ("on", "ˌɔn"),
                ("the", "ðə"),
                ("table.", "tˈAbᵊl."),
            ],
            false,
        );
        use Link::*;
        assert_eq!(
            n.iter().map(|x| x.link).collect::<Vec<_>>(),
            [
                Some(ConsonantVowel),
                Some(Flap),
                Some(ConsonantVowel),
                None,
                Some(Flap),
                Some(Flap),
                None,
                None,
                None
            ]
        );
        assert_eq!(n[3].weak_form.as_deref(), Some("ən"));
        assert_eq!(n[7].weak_form.as_deref(), Some("ðə"));
        assert_eq!(n[8].ipa, "tˈeɪbəl");
    }

    #[test]
    fn british_has_no_flap_but_linking_r() {
        use Link::*;
        assert_eq!(
            links(&[("get", "ɡɛt"), ("it", "ɪt")], true),
            [Some(ConsonantVowel), None]
        );
        assert_eq!(
            links(&[("far", "fˈɑː"), ("away", "əwˈA")], true),
            [Some(LinkingR), None]
        );
        assert!(!notes(&[("water", "wˈɔːtə")], true)[0].flap);
        assert!(notes(&[("water", "wˈɔTəɹ")], false)[0].flap);
        assert_eq!(
            notes(&[("for", "fɔɹ"), ("me", "mi")], true)[0]
                .weak_form
                .as_deref(),
            Some("fə")
        );
    }

    #[test]
    fn glides_same_sound_assimilation_elision() {
        use Link::*;
        assert_eq!(
            links(&[("see", "sˈi"), ("it", "ɪt")], false)[0],
            Some(GlideJ)
        );
        assert_eq!(
            links(&[("go", "ɡˈO"), ("on", "ˈɔn")], false)[0],
            Some(GlideW)
        );
        assert_eq!(
            links(&[("big", "bˈɪɡ"), ("game", "ɡˈAm")], false)[0],
            Some(SameSound)
        );
        assert_eq!(
            links(&[("did", "dˈɪd"), ("you", "ju")], false)[0],
            Some(Assimilation)
        );
        let n = notes(&[("next", "nˈɛkst"), ("day", "dˈA")], false);
        assert!(n[0].elided && n[0].link.is_none());
        assert!(
            !notes(&[("start", "stˈɑɹt"), ("now", "nˈW")], false)[0].elided,
            "ɹt 不省音"
        );
    }

    #[test]
    fn pauses_break_links_and_strong_forms() {
        // 逗号后不连读；句末虚词用强读
        assert_eq!(links(&[("Stop,", "stˈɑp,"), ("it", "ɪt")], false)[0], None);
        let n = notes(&[("What", "wˌʌt"), ("for?", "fɔɹ?")], false);
        assert_eq!(n[1].weak_form, None);
        // 有主重音的虚词不弱读
        assert_eq!(
            notes(&[("can", "kˈæn"), ("go", "ɡˈO")], false)[0].weak_form,
            None
        );
    }
}
