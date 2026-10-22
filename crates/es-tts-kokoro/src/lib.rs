//! Kokoro-82M 端侧 TTS 引擎（ONNX Runtime）。
//!
//! - 推理：`ort`，默认 CPU；可选平台加速（CoreML / NNAPI / DirectML）。
//! - G2P：见 [`g2p`]，misaki 词典 + 规则兜底，**不使用 espeak-ng（GPL-3）**。
//! - 逐词时间：该 ONNX 导出只输出波形，按每个词的音素数在句内分配时长。
//!
//! 模型目录结构（`scripts/fetch-models.sh` 生成，随安装包分发）：
//! ```text
//! <model_dir>/kokoro/
//!   model.onnx            # Kokoro-82M v1.0（默认 int8 量化，约 88MB）
//!   tokenizer.json        # 音素 → token id
//!   voices/<voice_id>.bin # [510, 1, 256] f32 风格向量，按 token 数取行
//!   g2p/{us,gb}_{gold,silver}.json
//! ```

pub mod g2p;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use es_core::model::{Accent, Gender, Voice};
use es_core::ports::{SynthOutput, SynthRequest, TtsEngine};
use es_core::{CoreError, Result};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;

use g2p::{to_model_phonemes, Lexicon, NextSound, PUNCT};

pub const SAMPLE_RATE: u32 = 24_000;
const MODEL_FILE: &str = "model.onnx";
const STYLE_DIM: usize = 256;
const STYLE_ROWS: usize = 510;
/// 模型上下文 512，去掉首尾填充；留余量以免超长。
const MAX_TOKENS: usize = 500;

/// 随包分发的精选英文音色（Kokoro v1.0 音色 id；首字母 a=美式 b=英式，次字母 f/m=性别）。
const VOICES: &[(&str, &str)] = &[
    ("af_heart", "Heart"),
    ("af_bella", "Bella"),
    ("am_michael", "Michael"),
    ("am_fenrir", "Fenrir"),
    ("bf_emma", "Emma"),
    ("bm_george", "George"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accelerator {
    Cpu,
    /// CoreML（Apple）/ NNAPI（Android）/ DirectML（Windows）；不可用时 ORT 自动回落 CPU。
    Platform,
}

#[derive(Debug, Clone)]
pub struct KokoroOptions {
    pub threads: usize,
    pub accelerator: Accelerator,
}

impl Default for KokoroOptions {
    fn default() -> Self {
        Self {
            threads: std::thread::available_parallelism().map_or(2, |n| n.get().min(4)),
            accelerator: Accelerator::Cpu,
        }
    }
}

/// 耗时的部分（模型会话约 2-3 秒、词表、G2P 词典），在后台线程初始化。
struct Runtime {
    session: Mutex<Session>,
    _env: ort::environment::Environment,
    /// 模型输入名（不同导出命名不同）：ids、style、speed；以及输出名。
    io: [String; 4],
    vocab: HashMap<char, i64>,
    us: Lexicon,
    gb: Lexicon,
}

/// 构造立即返回（音色列表可用），推理会话在后台加载；首次合成时等待加载完成。
/// 这样 App 启动不会被模型加载阻塞。
pub struct KokoroEngine {
    runtime: Arc<OnceLock<std::result::Result<Runtime, String>>>,
    voices: Vec<Voice>,
    styles: HashMap<String, Vec<f32>>,
}

fn engine_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Engine(e.to_string())
}

impl KokoroEngine {
    pub fn model_root(model_dir: &Path) -> PathBuf {
        model_dir.join("kokoro")
    }

    pub fn is_available(model_dir: &Path) -> bool {
        Self::model_root(model_dir).join(MODEL_FILE).is_file()
    }

    pub fn load(model_dir: &Path) -> Result<Self> {
        Self::load_with(model_dir, KokoroOptions::default())
    }

    pub fn load_with(model_dir: &Path, options: KokoroOptions) -> Result<Self> {
        let root = Self::model_root(model_dir);

        let mut styles = HashMap::new();
        let mut voices = Vec::new();
        for &(id, name) in VOICES {
            let path = root.join("voices").join(format!("{id}.bin"));
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            if bytes.len() != STYLE_ROWS * STYLE_DIM * 4 {
                return Err(engine_err(format!(
                    "unexpected voice file size: {}",
                    path.display()
                )));
            }
            styles.insert(
                id.to_string(),
                bytes
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect(),
            );
            voices.push(Voice {
                id: id.into(),
                name: name.into(),
                accent: if id.starts_with('b') {
                    Accent::British
                } else {
                    Accent::American
                },
                gender: if id.as_bytes()[1] == b'm' {
                    Gender::Male
                } else {
                    Gender::Female
                },
            });
        }
        if voices.is_empty() {
            return Err(engine_err(format!(
                "no Kokoro voices under {}",
                root.display()
            )));
        }

        let model = root.join(MODEL_FILE);
        if !model.is_file() {
            return Err(engine_err(format!("missing {}", model.display())));
        }
        let runtime = Arc::new(OnceLock::new());
        let slot = runtime.clone();
        std::thread::Builder::new()
            .name("kokoro-load".into())
            .spawn(move || {
                let t = std::time::Instant::now();
                let result = Runtime::load(&root, &options).map_err(|e| e.to_string());
                log::info!(
                    "kokoro runtime loaded in {:?}: ok={}",
                    t.elapsed(),
                    result.is_ok()
                );
                let _ = slot.set(result);
            })
            .map_err(engine_err)?;
        Ok(Self {
            runtime,
            voices,
            styles,
        })
    }

    /// 阻塞直到推理会话加载完成。
    pub fn wait_ready(&self) -> Result<()> {
        self.runtime().map(|_| ())
    }

    fn runtime(&self) -> Result<&Runtime> {
        self.runtime.wait().as_ref().map_err(engine_err)
    }

    /// 每个词元的音素串（含标点）。
    pub fn phonemize(&self, words: &[String], british: bool) -> Result<Vec<String>> {
        Ok(self.runtime()?.phonemize(words, british))
    }
}

impl Runtime {
    fn load(root: &Path, options: &KokoroOptions) -> Result<Self> {
        let env = ort::init()
            .with_name("english-speaker")
            .build()
            .map_err(engine_err)?;
        let session = build_session(&env, &root.join(MODEL_FILE), options)?;
        let inputs: Vec<String> = session
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .collect();
        let find = |keys: &[&str]| {
            inputs
                .iter()
                .find(|n| keys.iter().any(|k| n.contains(k)))
                .cloned()
                .ok_or_else(|| engine_err(format!("model inputs {inputs:?} missing {keys:?}")))
        };
        let output = session
            .outputs()
            .first()
            .map(|o| o.name().to_string())
            .ok_or_else(|| engine_err("model has no outputs"))?;
        let io = [
            find(&["input_ids", "tokens"])?,
            find(&["style"])?,
            find(&["speed"])?,
            output,
        ];

        let g2p_dir = root.join("g2p");
        Ok(Self {
            session: Mutex::new(session),
            _env: env,
            io,
            vocab: load_vocab(&root.join("tokenizer.json"))?,
            us: Lexicon::load(&g2p_dir, false)?,
            gb: Lexicon::load(&g2p_dir, true)?,
        })
    }

    fn phonemize(&self, words: &[String], british: bool) -> Vec<String> {
        let lex = if british { &self.gb } else { &self.us };
        // 展开为 (所属词元, 前导标点, 词, 尾随标点)
        let mut parts = Vec::new();
        for (t, token) in words.iter().enumerate() {
            for raw in token.split_whitespace() {
                let core = raw.trim_matches(|c: char| {
                    !c.is_alphanumeric() && c != '\'' && c != '’' && c != '-'
                });
                let start = raw.find(core).unwrap_or(0);
                let lead: String = raw[..start].chars().filter(|c| PUNCT.contains(c)).collect();
                let trail: String = raw[start + core.len()..]
                    .chars()
                    .filter(|c| PUNCT.contains(c))
                    .collect();
                parts.push((t, lead, core.to_string(), trail));
            }
        }
        // 从右往左：每个词都知道后一个词的开头音（按音素判断，不按拼写）
        let mut pieces = vec![String::new(); parts.len()];
        let mut next: NextSound = None;
        for (i, (_, lead, core, trail)) in parts.iter().enumerate().rev() {
            let after = g2p::next_sound(trail, next);
            let ph = lex.word(core, after);
            pieces[i] = to_model_phonemes(&format!("{lead}{ph}{trail}"));
            next = g2p::next_sound(&pieces[i], after);
        }
        let mut out = vec![String::new(); words.len()];
        for ((t, ..), piece) in parts.iter().zip(pieces) {
            if piece.is_empty() {
                continue;
            }
            if !out[*t].is_empty() {
                out[*t].push(' ');
            }
            out[*t].push_str(&piece);
        }
        out
    }

    fn encode(&self, phonemes: &str) -> Vec<i64> {
        phonemes
            .chars()
            .filter_map(|c| self.vocab.get(&c).copied())
            .collect()
    }

    fn infer(&self, ids: &[i64], style: &[f32], speed: f32) -> Result<Vec<f32>> {
        let row = ids.len().min(STYLE_ROWS - 1);
        let style = style[row * STYLE_DIM..(row + 1) * STYLE_DIM].to_vec();
        let mut padded = Vec::with_capacity(ids.len() + 2);
        padded.push(0);
        padded.extend_from_slice(ids);
        padded.push(0);

        let ids_t =
            Tensor::from_array((vec![1i64, padded.len() as i64], padded)).map_err(engine_err)?;
        let style_t =
            Tensor::from_array((vec![1i64, STYLE_DIM as i64], style)).map_err(engine_err)?;
        let speed_t = Tensor::from_array((vec![1i64], vec![speed])).map_err(engine_err)?;
        let [ids_name, style_name, speed_name, out_name] = &self.io;
        let mut session = self.session.lock().unwrap();
        let outputs = session
            .run(ort::inputs![
                ids_name.as_str() => ids_t,
                style_name.as_str() => style_t,
                speed_name.as_str() => speed_t,
            ])
            .map_err(engine_err)?;
        let (_, wave) = outputs[out_name.as_str()]
            .try_extract_tensor::<f32>()
            .map_err(engine_err)?;
        Ok(wave.to_vec())
    }
}

fn build_session(
    env: &ort::environment::Environment,
    path: &Path,
    options: &KokoroOptions,
) -> Result<Session> {
    let mut builder = Session::builder(env)
        .map_err(engine_err)?
        .with_optimization_level(GraphOptimizationLevel::Level1)
        .map_err(engine_err)?
        .with_intra_threads(options.threads.max(1))
        .map_err(engine_err)?;
    if options.accelerator == Accelerator::Platform {
        builder = builder
            .with_execution_providers(platform_providers())
            .map_err(engine_err)?;
    }
    builder.commit_from_file(path).map_err(engine_err)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn platform_providers() -> Vec<ort::ep::ExecutionProviderDispatch> {
    vec![ort::ep::CoreML::default().build()]
}

#[cfg(target_os = "android")]
fn platform_providers() -> Vec<ort::ep::ExecutionProviderDispatch> {
    vec![ort::ep::NNAPI::default().build()]
}

#[cfg(target_os = "windows")]
fn platform_providers() -> Vec<ort::ep::ExecutionProviderDispatch> {
    vec![ort::ep::DirectML::default().build()]
}

/// Linux 等：没有平台加速后端，使用 CPU。
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "android",
    target_os = "windows"
)))]
fn platform_providers() -> Vec<ort::ep::ExecutionProviderDispatch> {
    Vec::new()
}

fn load_vocab(path: &Path) -> Result<HashMap<char, i64>> {
    let text = std::fs::read_to_string(path).map_err(engine_err)?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(engine_err)?;
    let vocab = json["model"]["vocab"]
        .as_object()
        .ok_or_else(|| engine_err("tokenizer.json has no model.vocab"))?;
    Ok(vocab
        .iter()
        .filter_map(|(k, v)| {
            let mut chars = k.chars();
            match (chars.next(), chars.next(), v.as_i64()) {
                (Some(c), None, Some(id)) => Some((c, id)),
                _ => None,
            }
        })
        .collect())
}

/// 音素中“发音”的字符数，用于在句内分配逐词时长。
fn phoneme_weight(p: &str) -> u64 {
    p.chars()
        .filter(|c| !PUNCT.contains(c) && !matches!(c, ' ' | 'ˈ' | 'ˌ' | 'ː'))
        .count()
        .max(1) as u64
}

impl TtsEngine for KokoroEngine {
    fn id(&self) -> &str {
        "kokoro-82m"
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn voices(&self) -> Vec<Voice> {
        self.voices.clone()
    }

    fn synthesize(&self, req: &SynthRequest<'_>) -> Result<SynthOutput> {
        let style = self
            .styles
            .get(req.voice_id)
            .ok_or_else(|| CoreError::InvalidInput(format!("unknown voice {}", req.voice_id)))?;
        let rt = self.runtime()?;
        let british = req.voice_id.starts_with('b');
        let per_word = rt.phonemize(req.words, british);

        // 按 token 上限分批（正常句子一批即可），每批一次推理
        let mut batches: Vec<Vec<usize>> = vec![Vec::new()];
        let mut len = 0;
        for (i, p) in per_word.iter().enumerate() {
            let n = rt.encode(p).len() + 1;
            if len + n > MAX_TOKENS && !batches.last().unwrap().is_empty() {
                batches.push(Vec::new());
                len = 0;
            }
            batches.last_mut().unwrap().push(i);
            len += n;
        }

        let mut samples = Vec::new();
        let mut word_frames = vec![(0u64, 0u64); per_word.len()];
        for batch in batches {
            let text = batch
                .iter()
                .map(|&i| per_word[i].as_str())
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let ids = rt.encode(&text);
            let offset = samples.len() as u64;
            if !ids.is_empty() {
                samples.extend(rt.infer(&ids, style, req.speed)?);
            }
            let total = samples.len() as u64 - offset;
            let weights: Vec<u64> = batch
                .iter()
                .map(|&i| phoneme_weight(&per_word[i]))
                .collect();
            let sum = weights.iter().sum::<u64>().max(1);
            let mut acc = 0;
            for (&i, w) in batch.iter().zip(&weights) {
                let start = offset + acc * total / sum;
                acc += w;
                word_frames[i] = (start, offset + acc * total / sum);
            }
        }
        Ok(SynthOutput {
            samples,
            word_frames: Some(word_frames),
            word_phonemes: Some(per_word),
        })
    }
}
