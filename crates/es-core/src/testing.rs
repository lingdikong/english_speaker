//! 开发与测试用的替身实现：在真实模型接入前即可跑通完整流程。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::error::Result;
use crate::events::{CoreEvent, EventListener};
use crate::model::*;
use crate::ports::*;

/// 用正弦音代替语音：每个词一个音，时长与字符数成正比，并返回精确的逐词帧。
pub struct MockTtsEngine {
    sample_rate: u32,
}

impl Default for MockTtsEngine {
    fn default() -> Self {
        Self {
            sample_rate: 24_000,
        }
    }
}

impl TtsEngine for MockTtsEngine {
    fn id(&self) -> &str {
        "mock"
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn voices(&self) -> Vec<Voice> {
        vec![
            Voice {
                id: "mock_f".into(),
                name: "Mock Female".into(),
                accent: Accent::American,
                gender: Gender::Female,
            },
            Voice {
                id: "mock_m".into(),
                name: "Mock Male".into(),
                accent: Accent::British,
                gender: Gender::Male,
            },
        ]
    }

    fn synthesize(&self, req: &SynthRequest<'_>) -> Result<SynthOutput> {
        let base_hz = if req.voice_id.ends_with('m') {
            140.0
        } else {
            220.0
        };
        let per_char = self.sample_rate as usize * 6 / 100; // 60ms / 字符
        let mut samples = Vec::new();
        let mut word_frames = Vec::with_capacity(req.words.len());
        for (i, w) in req.words.iter().enumerate() {
            let start = samples.len() as u64;
            let chars = w.chars().filter(|c| c.is_alphanumeric()).count();
            let len = (chars * per_char) as f32 / req.speed.max(0.1);
            let hz = base_hz * (1.0 + (i % 4) as f32 * 0.12);
            for n in 0..len as usize {
                let t = n as f32 / self.sample_rate as f32;
                let env = (n as f32 / len * std::f32::consts::PI).sin();
                samples.push((t * hz * std::f32::consts::TAU).sin() * 0.2 * env);
            }
            word_frames.push((start, samples.len() as u64));
        }
        Ok(SynthOutput {
            samples,
            word_frames: Some(word_frames),
            word_phonemes: Some(req.words.iter().map(|w| pseudo_phonemes(w)).collect()),
        })
    }
}

/// 用拼写近似音素（e→ɛ，o→ɑ，其余字母原样），让连读标注在开发期也能演示。
fn pseudo_phonemes(word: &str) -> String {
    word.chars()
        .map(|c| match c.to_ascii_lowercase() {
            'e' => 'ɛ',
            'o' => 'ɑ',
            'g' => 'ɡ',
            'r' => 'ɹ',
            c => c,
        })
        .collect()
}

/// 不出声的 sink，按实时速度“播放”，用于无声卡环境（CI、单测）。
#[derive(Default)]
pub struct NullSink {
    state: Mutex<NullSinkState>,
    played: AtomicU64,
}

#[derive(Default)]
struct NullSinkState {
    sample_rate: u32,
    channels: u16,
    paused: bool,
}

impl AudioSink for NullSink {
    fn open(&self, format: PcmFormat) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        s.sample_rate = format.sample_rate;
        s.channels = format.channels;
        s.paused = false;
        self.played.store(0, Ordering::SeqCst);
        Ok(())
    }

    fn write(&self, samples: &[f32]) -> Result<()> {
        let (sr, ch) = {
            let s = self.state.lock().unwrap();
            (s.sample_rate.max(1), s.channels.max(1))
        };
        let frames = samples.len() as u64 / ch as u64;
        std::thread::sleep(std::time::Duration::from_micros(
            frames * 1_000_000 / sr as u64,
        ));
        self.played.fetch_add(frames, Ordering::SeqCst);
        Ok(())
    }

    fn pause(&self) {
        self.state.lock().unwrap().paused = true;
    }

    fn resume(&self) {
        self.state.lock().unwrap().paused = false;
    }

    fn flush(&self) {
        self.played.store(0, Ordering::SeqCst);
    }

    fn played_frames(&self) -> u64 {
        self.played.load(Ordering::SeqCst)
    }

    fn set_rate(&self, _rate: f32) -> bool {
        false
    }

    fn close(&self) {}
}

/// 收集事件，便于断言。
#[derive(Default)]
pub struct CollectingListener {
    pub events: Mutex<Vec<CoreEvent>>,
}

impl EventListener for CollectingListener {
    fn on_event(&self, event: CoreEvent) {
        self.events.lock().unwrap().push(event);
    }
}

// ---------- 翻译与查词替身 ----------

/// 把每个词换成“〔词〕”：可以断言逐句都翻了，且占位符原样保留（模拟 Opus-MT 照抄专有名词）。
#[derive(Default)]
pub struct MockTranslator {
    pub calls: AtomicU64,
}

impl Translator for MockTranslator {
    fn id(&self) -> &str {
        "mock-mt"
    }

    fn translate(&self, text: &str) -> Result<String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(text
            .split_whitespace()
            .map(|w| {
                let w = w.trim_matches(|c: char| !c.is_alphanumeric());
                if w.starts_with("XQ") {
                    w.to_string()
                } else {
                    format!("〔{w}〕")
                }
            })
            .collect::<Vec<_>>()
            .join(""))
    }
}

/// 每个词都查得到：原形 = 去掉结尾的 s，释义为“释义<原形>”。
pub struct MockDictionary;

impl Dictionary for MockDictionary {
    fn lookup(&self, word: &str) -> Result<Option<DictEntry>> {
        let lemma = word.to_lowercase();
        let lemma = lemma.strip_suffix('s').unwrap_or(&lemma).to_string();
        Ok(Some(DictEntry {
            inflection: (lemma != word.to_lowercase()).then(|| "复数".into()),
            query: word.into(),
            phonetic: format!("ˈ{lemma}"),
            senses: vec![DictSense {
                pos: "n.".into(),
                meaning: format!("释义{lemma}"),
            }],
            tags: vec!["四级".into()],
            collins: 3,
            oxford: true,
            frequency: Some(100),
            forms: Vec::new(),
            word: lemma,
        }))
    }
}

/// 模拟本地大模型：下载瞬间完成（可失败一次），按请求类型返回固定格式的结果。
#[derive(Default)]
pub struct MockLlm {
    downloaded: std::sync::atomic::AtomicBool,
    pub generations: AtomicU64,
}

impl LlmProvider for MockLlm {
    fn model_name(&self) -> &str {
        "Mock LLM"
    }

    fn size_bytes(&self) -> u64 {
        1000
    }

    fn is_downloaded(&self) -> bool {
        self.downloaded.load(Ordering::SeqCst)
    }

    fn partial_bytes(&self) -> u64 {
        0
    }

    fn download(
        &self,
        progress: &mut dyn FnMut(u64, u64),
        _cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<()> {
        progress(500, 1000);
        progress(1000, 1000);
        self.downloaded.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn delete(&self) -> Result<()> {
        self.downloaded.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn load(&self) -> Result<std::sync::Arc<dyn LanguageModel>> {
        Ok(std::sync::Arc::new(MockModel))
    }
}

struct MockModel;

impl LanguageModel for MockModel {
    fn id(&self) -> &str {
        "mock-llm"
    }

    fn generate(
        &self,
        req: &LlmRequest<'_>,
        _cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<String> {
        let line = |label: &str| {
            req.user
                .lines()
                .find_map(|l| l.strip_prefix(label))
                .unwrap_or("")
                .to_string()
        };
        Ok(match req.grammar {
            Some(g) if g.contains("collocations") => format!(
                r#"{{"pos":"n.","meaning":"语境义{}","collocations":["{w} up","{w} out"],"examples":[{{"en":"A {w}.","zh":"一个。"}}]}}"#,
                line("单词："),
                w = line("单词：")
            ),
            Some(_) => format!(
                r#"{{"translation":"精译：{s}","structure":"主谓","points":[{{"text":"{s}","note":"y"}}]}}"#,
                s = line("句子：")
            ),
            None => format!("精译：{}", line("原文：")),
        })
    }
}
