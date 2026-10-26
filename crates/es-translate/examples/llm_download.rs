//! 下载智能翻译模型（断点续传 + SHA-256 校验），开发期预置模型用。
//!
//! ```bash
//! cargo run --release -p es-translate --example llm_download -- <dir>
//! ```

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use es_core::ports::LlmProvider;

fn main() {
    let dir = std::env::args().nth(1).expect("usage: llm_download <dir>");
    let provider = es_translate::QwenProvider::new(dir.into());
    let t = Instant::now();
    let mut last = 0;
    let result = provider.download(
        &mut |received, total| {
            let pct = received * 100 / total.max(1);
            if pct != last {
                last = pct;
                eprintln!("{pct:>3}%  {:.1} MB", received as f64 / 1e6);
            }
        },
        &AtomicBool::new(false),
    );
    println!(
        "{result:?} in {:?} -> {}",
        t.elapsed(),
        provider.path().display()
    );
}
