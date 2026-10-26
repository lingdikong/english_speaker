//! 真实 Opus-MT 测试：需要 `models/opus-mt-en-zh`（scripts/fetch-models.sh 下载），没有时跳过。

use std::path::PathBuf;

use es_core::ports::Translator;
use es_translate::OpusMt;

#[test]
fn translates_english_to_chinese() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models");
    if !OpusMt::is_available(&dir) {
        eprintln!("skip: no Opus-MT model");
        return;
    }
    let mt = OpusMt::load(&dir, 2).unwrap();
    let out = mt
        .translate("I'd like a cup of coffee before the meeting starts.")
        .unwrap();
    assert!(out.contains("咖啡") && out.contains("会议"), "{out}");
    assert!(out.ends_with('。'), "补上句末标点：{out}");
    // 术语占位符原样保留（核心层据此替换为固定译法）
    let out = mt.translate("XQA powers this XQB.").unwrap();
    assert!(out.contains("XQA") && out.contains("XQB"), "{out}");
    assert_eq!(mt.translate("  ").unwrap(), "");
}
