//! 翻译与查词基准：Opus-MT、ECDICT，以及（给出模型目录时）Qwen 整句翻译 / 点词 / 句子解析。
//!
//! ```bash
//! cargo run --release -p es-translate --example translate_bench -- models [<llm_dir>]
//! ```

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use es_core::ports::{Dictionary, LlmProvider, Translator};
use es_core::translate::prompt;

const SENTENCES: &[&str] = &[
    "I'd like a cup of coffee before the meeting starts.",
    "Learning by listening is one of the most natural ways to pick up a language.",
    "The access token expires after an hour, so the app refreshes it automatically.",
    "She perceived a subtle change in his tone, although he insisted nothing was wrong.",
    "If you leave the bread out overnight, it will go stale by the morning.",
];

fn main() {
    let mut args = std::env::args().skip(1);
    let models = std::path::PathBuf::from(args.next().unwrap_or_else(|| "models".into()));

    let mt = es_translate::OpusMt::load(&models, 4).expect("opus-mt");
    mt.translate("Warm up.").unwrap();
    println!("== Opus-MT");
    for s in SENTENCES {
        let t = Instant::now();
        let out = mt.translate(s).unwrap();
        println!("{:>5}ms  {s}\n         {out}", t.elapsed().as_millis());
    }

    let dict = es_translate::EcDict::open(&models).expect("ecdict");
    println!("\n== ECDICT");
    for w in ["perceived", "stale", "overnight", "tokens"] {
        let t = Instant::now();
        let e = dict.lookup(w).unwrap();
        println!(
            "{:>5}µs  {w}: {:?}",
            t.elapsed().as_micros(),
            e.map(|e| (e.brief(), e.word, e.phonetic, e.inflection))
        );
    }

    let Some(llm_dir) = args.next() else { return };
    let provider = es_translate::QwenProvider::new(llm_dir.into());
    let t = Instant::now();
    let model = provider.load().expect("load qwen");
    println!("\n== Qwen (loaded in {:?})", t.elapsed());
    let cancel = AtomicBool::new(false);
    for s in SENTENCES {
        let terms = if s.contains("token") {
            "access token = 访问令牌\napp = 应用"
        } else {
            ""
        };
        let reference = mt.translate(s).unwrap();
        let p = prompt::translate(s, Some(&reference), terms);
        let t = Instant::now();
        let raw = model.generate(&p.request(), &cancel).unwrap();
        println!(
            "{:>5}ms  {s}\n   MT    {reference}\n   Qwen  {}",
            t.elapsed().as_millis(),
            prompt::clean_translation(&raw)
        );
    }

    for (word, sentence) in [
        ("stale", SENTENCES[4]),
        ("perceived", SENTENCES[3]),
        ("pick", SENTENCES[1]),
    ] {
        let entry = dict.lookup(word).unwrap();
        let t = Instant::now();
        let raw = model
            .generate(
                &prompt::word(word, sentence, entry.as_ref()).request(),
                &cancel,
            )
            .unwrap();
        println!(
            "\n== word {word} {:?}\n{raw}\n{:?}",
            t.elapsed(),
            prompt::parse_word(word, &raw)
        );
    }

    let s = SENTENCES[3];
    let reference = mt.translate(s).unwrap();
    let t = Instant::now();
    let raw = model
        .generate(
            &prompt::sentence(s, Some(&reference), "").request(),
            &cancel,
        )
        .unwrap();
    println!(
        "\n== sentence {:?}\n{raw}\n{:?}",
        t.elapsed(),
        prompt::parse_sentence(s, &raw)
    );
}
