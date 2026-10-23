//! 真实模型测试：需要先运行 `scripts/fetch-models.sh`；没有模型文件时跳过。

use std::path::PathBuf;

use es_core::ports::{SynthRequest, TtsEngine};
use es_core::text;
use es_tts_kokoro::{KokoroEngine, SAMPLE_RATE};

fn engine() -> Option<KokoroEngine> {
    let dir = std::env::var_os("ES_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models"));
    if !KokoroEngine::is_available(&dir) {
        eprintln!("skipping: no Kokoro model under {}", dir.display());
        return None;
    }
    Some(KokoroEngine::load(&dir).expect("load"))
}

#[test]
fn synthesizes_sentence_with_word_timings() {
    let Some(engine) = engine() else { return };
    let plan = text::plan("Hello world, this costs $5 today.");
    let s = &plan[0];
    let words = s.spoken_words();
    let out = engine
        .synthesize(&SynthRequest {
            text: &s.spoken(),
            words: &words,
            voice_id: "af_heart",
            speed: 1.0,
        })
        .unwrap();

    let secs = out.samples.len() as f64 / SAMPLE_RATE as f64;
    assert!((1.0..6.0).contains(&secs), "plausible duration: {secs}s");
    assert!(out.samples.iter().all(|x| x.is_finite()));
    let peak = out.samples.iter().fold(0f32, |m, x| m.max(x.abs()));
    assert!(peak > 0.05 && peak <= 1.5, "peak amplitude {peak}");

    let frames = out.word_frames.unwrap();
    assert_eq!(frames.len(), words.len());
    assert!(frames
        .windows(2)
        .all(|w| w[0].1 <= w[1].0 + 1 && w[0].0 <= w[0].1));
    assert_eq!(frames.last().unwrap().1, out.samples.len() as u64);
}

#[test]
fn accents_and_vocab_coverage() {
    let Some(engine) = engine() else { return };
    let words: Vec<String> = ["tomato", "water", "schedule", "Zxqv", "GPU"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let us = engine.phonemize(&words, false).unwrap();
    let gb = engine.phonemize(&words, true).unwrap();
    assert_ne!(us, gb, "美式与英式词典给出不同发音");
    assert!(
        us.iter().all(|p| !p.is_empty()),
        "未登录词也有兜底发音: {us:?}"
    );
    assert!(us.iter().all(|p| !p.contains('g')), "词表只有 ɡ: {us:?}");

    // 英式音色可以合成
    let out = engine
        .synthesize(&SynthRequest {
            text: "",
            words: &words,
            voice_id: "bm_george",
            speed: 1.0,
        })
        .unwrap();
    assert!(!out.samples.is_empty());
    assert!(engine
        .synthesize(&SynthRequest {
            text: "",
            words: &words,
            voice_id: "nope",
            speed: 1.0
        })
        .is_err());
}

/// 超过单次上下文的长输入会被分批推理并拼接。
#[test]
fn long_input_is_batched() {
    let Some(engine) = engine() else { return };
    let words: Vec<String> = (0..120).map(|i| format!("word{}", i % 7)).collect();
    let words: Vec<String> = words
        .iter()
        .map(|w| w.replace(char::is_numeric, ""))
        .chain(std::iter::once("end.".to_string()))
        .collect();
    let out = engine
        .synthesize(&SynthRequest {
            text: "",
            words: &words,
            voice_id: "af_heart",
            speed: 1.0,
        })
        .unwrap();
    assert!(out.samples.len() as f64 / SAMPLE_RATE as f64 > 10.0);
    assert_eq!(out.word_frames.unwrap().len(), words.len());
}

/// 连读相关的音素输入与 Kokoro 训练时（misaki）一致。
#[test]
fn connected_speech_phonemes() {
    let Some(engine) = engine() else { return };
    let ph = |s: &str| {
        let words: Vec<String> = s.split(' ').map(String::from).collect();
        engine.phonemize(&words, false).unwrap()
    };
    // 冠词按后一个词的发音选择：hour 元音开头，university 辅音 j 开头
    assert_eq!(ph("the hour")[0], "ði");
    assert_eq!(ph("the university")[0], "ðə");
    assert_eq!(ph("an hour")[0], "ɐn");
    // to 的弱读形式，句末保留原形
    assert_eq!(ph("to eat")[0], "tʊ");
    assert_eq!(ph("to go")[0], "tə");
    assert_eq!(ph("want to.")[1], "tu.");
    // 美式闪音写作 T（Kokoro v1.0 训练约定），不出现 ɾ
    let water = ph("water better city");
    assert!(
        water.iter().all(|p| p.contains('T') && !p.contains('ɾ')),
        "{water:?}"
    );
    // 虚词保持无重音形式，由模型弱读
    assert_eq!(ph("and of for")[..], ["ænd", "ʌv", "fɔɹ"]);
}

/// 真实音素上的连读标注。
#[test]
fn connected_speech_annotation_on_real_phonemes() {
    use es_core::text::connected::{annotate, Link};
    let Some(engine) = engine() else { return };
    let check = |sentence: &str, british: bool| {
        let words: Vec<String> = sentence.split(' ').map(String::from).collect();
        let display: Vec<&str> = sentence.split(' ').collect();
        let ph = engine.phonemize(&words, british).unwrap();
        annotate(&display, &ph, british)
    };
    let n = check("Pick it up and put it on the table.", false);
    let links: Vec<_> = n.iter().map(|x| x.link).collect();
    use Link::*;
    assert_eq!(
        links,
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
    assert_eq!(n[8].ipa, "tˈeɪbəl");

    let n = check("Did you see it? Turn it off and go on.", false);
    assert_eq!(n[0].link, Some(Assimilation));
    assert_eq!(n[2].link, Some(GlideJ));
    assert_eq!(n[3].link, None, "问号后不连读");
    assert_eq!(n[8].link, Some(GlideW));
    assert!(check("water", false)[0].flap);
    assert!(!check("water", true)[0].flap, "英式没有闪音");
    assert_eq!(check("far away", true)[0].link, Some(LinkingR));
}

#[test]
fn engines_can_be_reloaded_and_loaded_concurrently() {
    // 模型在后台线程加载，错误要到第一次合成才会出现，所以每个引擎都实际合成一句。
    fn speak(engine: &KokoroEngine) {
        let plan = text::plan("Hi there.");
        let words = plan[0].spoken_words();
        engine
            .synthesize(&SynthRequest {
                text: &plan[0].spoken(),
                words: &words,
                voice_id: "af_heart",
                speed: 1.0,
            })
            .expect("synthesize");
    }
    let Some(first) = engine() else { return };
    speak(&first);
    drop(first);
    let again: Vec<_> = (0..2)
        .map(|_| std::thread::spawn(|| engine().map(|e| speak(&e))))
        .collect();
    for h in again {
        h.join().expect("reload after drop / concurrent load");
    }
}
