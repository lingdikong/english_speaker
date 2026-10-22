//! Kokoro 基准：音素化结果、加载耗时、RTF（合成耗时 / 音频时长），并输出 WAV 供试听。
//!
//! ```text
//! cargo run --release -p es-tts-kokoro --example kokoro_bench -- [models目录] [输出目录] [cpu|platform]
//! ```

use std::path::PathBuf;
use std::time::Instant;

use es_core::ports::{SynthRequest, TtsEngine};
use es_core::text;
use es_tts_kokoro::{Accelerator, KokoroEngine, KokoroOptions, SAMPLE_RATE};

const TEXT: &str = "Kokoro is an open-weight text-to-speech model with 82 million parameters. \
Despite its lightweight architecture, it delivers quality comparable to much larger models. \
In 2024, Dr. Lee's team raised $3.5M, about 20% more than they had planned.";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let models = PathBuf::from(args.first().map(String::as_str).unwrap_or("models"));
    let out_dir = PathBuf::from(args.get(1).map(String::as_str).unwrap_or("."));
    let accelerator = match args.get(2).map(String::as_str) {
        Some("platform") => Accelerator::Platform,
        _ => Accelerator::Cpu,
    };
    std::fs::create_dir_all(&out_dir).unwrap();

    let t = Instant::now();
    let g2p_dir = KokoroEngine::model_root(&models).join("g2p");
    es_tts_kokoro::g2p::Lexicon::load(&g2p_dir, false).unwrap();
    println!("  (of which: one G2P lexicon {:?})", t.elapsed());

    let t = Instant::now();
    let options = KokoroOptions {
        accelerator,
        ..Default::default()
    };
    let threads = options.threads;
    let engine = KokoroEngine::load_with(&models, options).expect("load Kokoro");
    println!(
        "constructed: {:?} (session loads in background)",
        t.elapsed()
    );
    engine.wait_ready().expect("load session");
    println!(
        "ready: {:?} ({accelerator:?}, {threads} threads)",
        t.elapsed()
    );

    // ES_TEXT 可替换测试文本
    let custom = std::env::var("ES_TEXT").ok();
    let plan = text::plan(custom.as_deref().unwrap_or(TEXT));
    for voice in ["af_heart", "bm_george"] {
        let mut all = Vec::new();
        let mut synth_time = 0.0;
        for (i, s) in plan.iter().enumerate() {
            let words = s.spoken_words();
            if voice == "af_heart" {
                println!(
                    "  [{i}] {}",
                    engine.phonemize(&words, false).unwrap().join(" ")
                );
            }
            let t = Instant::now();
            let out = engine
                .synthesize(&SynthRequest {
                    text: &s.spoken(),
                    words: &words,
                    voice_id: voice,
                    speed: 1.0,
                })
                .expect("synthesize");
            let dt = t.elapsed().as_secs_f64();
            synth_time += dt;
            let audio = out.samples.len() as f64 / SAMPLE_RATE as f64;
            println!(
                "  {voice} sentence {i}: {audio:.2}s audio in {dt:.3}s (RTF {:.3})",
                dt / audio
            );
            all.extend(out.samples);
            all.extend(std::iter::repeat_n(0.0, SAMPLE_RATE as usize / 4));
        }
        let audio = all.len() as f64 / SAMPLE_RATE as f64;
        println!(
            "{voice}: {audio:.1}s audio, {synth_time:.2}s compute, RTF {:.3}",
            synth_time / audio
        );
        let path = out_dir.join(format!("kokoro_{voice}.wav"));
        write_wav(&path, &all);
        println!("  → {}", path.display());
    }
}

fn write_wav(path: &std::path::Path, samples: &[f32]) {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    b.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}
