//! 真实词库测试：需要 `models/dict/ecdict.db`（scripts/fetch-models.sh 生成），没有时跳过。

use std::path::PathBuf;

use es_core::ports::Dictionary;
use es_translate::EcDict;

fn dict() -> Option<EcDict> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models");
    EcDict::is_available(&dir).then(|| EcDict::open(&dir).unwrap())
}

#[test]
fn looks_up_words_and_inflections() {
    let Some(d) = dict() else {
        eprintln!("skip: no dictionary");
        return;
    };
    let e = d.lookup("perceive").unwrap().unwrap();
    assert_eq!(e.phonetic, "pəˈsiːv");
    assert!(e.tags.contains(&"四级".to_string()));
    assert!(e.senses[0].meaning.contains("感觉"));

    let e = d.lookup("Perceived").unwrap().unwrap();
    assert_eq!(
        (e.word.as_str(), e.query.as_str()),
        ("perceive", "Perceived")
    );
    assert_eq!(e.inflection.as_deref(), Some("过去分词 / 过去式"));

    // 自身是常用词的变形：给自己的释义
    let e = d.lookup("left").unwrap().unwrap();
    assert_eq!(e.word, "left");
    assert!(e.senses[0].meaning.contains("左"));

    let e = d.lookup("apples").unwrap().unwrap();
    assert_eq!(
        (e.word.as_str(), e.inflection.as_deref()),
        ("apple", Some("复数"))
    );
    assert!(d.lookup("ran").unwrap().unwrap().word == "run");
    assert!(d.lookup("qwxzv").unwrap().is_none());
}
