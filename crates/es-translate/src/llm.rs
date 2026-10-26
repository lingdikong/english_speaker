//! 智能翻译：Qwen2.5-0.5B-Instruct（Apache-2.0）本地推理，llama.cpp（GGUF Q4_0）。
//!
//! - **下载**：首选 ModelScope（国内快），失败回退 HuggingFace（版本锁定）；HTTP Range 断点续传，
//!   完成后校验 SHA-256，校验失败删掉重下。文件放在可写的数据目录 `<data_dir>/models/`。
//! - **推理**：Apple 用 Metal，其余平台用 CPU（ARM NEON / x86 AVX；Q4_0 在 ARM 上有重排加速）。
//!   每次生成新建上下文（2048 token，KV 缓存约 25MB），用完即释放；模型权重 mmap，常驻约 450MB。
//! - **输出约束**：请求带 GBNF 语法时，用语法采样器保证输出结构合法（点词 / 句子解析的 JSON）。

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use es_core::ports::{LanguageModel, LlmProvider, LlmRequest};
use es_core::{CoreError, Result};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;
use sha2::{Digest, Sha256};

/// 随版本锁定的模型文件。换模型时同时更新文件名、大小与校验值。
pub struct ModelSpec {
    pub name: &'static str,
    pub file: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    pub urls: &'static [&'static str],
}

pub const QWEN2_5_0_5B_Q4_0: ModelSpec = ModelSpec {
    name: "Qwen2.5-0.5B-Instruct Q4_0",
    file: "qwen2.5-0.5b-instruct-q4_0.gguf",
    size: 428_730_208,
    sha256: "7671c0c304e6ce5a7fc577bcb12aba01e2c155cc2efd29b2213c95b18edaf6ed",
    urls: &[
        "https://modelscope.cn/models/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/master/qwen2.5-0.5b-instruct-q4_0.gguf",
        "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/9217f5db79a29953eb74d5343926648285ec7e67/qwen2.5-0.5b-instruct-q4_0.gguf",
    ],
};

const N_CTX: u32 = 2048;
const N_BATCH: usize = 512;

fn llm_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Translation(format!("llm: {e}"))
}

fn net_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Network(e.to_string())
}

/// llama.cpp 后端全进程只能初始化一次。
fn backend() -> Result<&'static LlamaBackend> {
    static BACKEND: OnceLock<std::result::Result<LlamaBackend, String>> = OnceLock::new();
    BACKEND
        .get_or_init(|| {
            // llama.cpp 默认把加载日志打到 stderr；交给 log 太吵，直接关掉
            llama_cpp_2::send_logs_to_tracing(
                llama_cpp_2::LogOptions::default().with_logs_enabled(false),
            );
            LlamaBackend::init().map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(llm_err)
}

pub struct QwenProvider {
    dir: PathBuf,
    spec: &'static ModelSpec,
    urls: Vec<String>,
    threads: i32,
}

impl QwenProvider {
    /// `dir`：可写目录（`<data_dir>/models`）。
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            spec: &QWEN2_5_0_5B_Q4_0,
            urls: QWEN2_5_0_5B_Q4_0
                .urls
                .iter()
                .map(|s| s.to_string())
                .collect(),
            threads: default_threads(),
        }
    }

    /// 自定义下载地址（自建镜像），按顺序尝试。
    pub fn with_urls(mut self, urls: Vec<String>) -> Self {
        if !urls.is_empty() {
            self.urls = urls;
        }
        self
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join(self.spec.file)
    }

    fn part_path(&self) -> PathBuf {
        self.dir.join(format!("{}.part", self.spec.file))
    }

    /// 从 `url` 续传到 `.part`；返回时 `.part` 已完整（大小等于预期）。
    fn fetch(
        &self,
        url: &str,
        progress: &mut dyn FnMut(u64, u64),
        cancel: &AtomicBool,
    ) -> Result<()> {
        let part = self.part_path();
        let have = std::fs::metadata(&part).map_or(0, |m| m.len());
        let total = self.spec.size;
        if have >= total {
            return Ok(());
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .http_status_as_error(false)
            .build()
            .into();
        let mut resp = agent
            .get(url)
            .header("Range", &format!("bytes={have}-"))
            .header("User-Agent", "EnglishSpeaker")
            .call()
            .map_err(net_err)?;
        let status = resp.status().as_u16();
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&part)?;
        let mut written = match status {
            206 => {
                file.seek(SeekFrom::Start(have))?;
                have
            }
            // 服务器不支持续传：从头写
            200 => {
                file.set_len(0)?;
                0
            }
            416 => return Ok(()), // 已经下完
            s => return Err(net_err(format!("HTTP {s} from {url}"))),
        };
        progress(written, total);
        let mut reader = resp.body_mut().as_reader();
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            if cancel.load(Ordering::SeqCst) {
                file.flush()?;
                return Err(CoreError::Cancelled);
            }
            let n = reader.read(&mut buf).map_err(net_err)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            written += n as u64;
            progress(written, total);
        }
        file.sync_all()?;
        if written != total {
            return Err(net_err(format!(
                "incomplete download: {written} of {total} bytes"
            )));
        }
        Ok(())
    }

    fn verify(&self, path: &Path) -> Result<bool> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 1024 * 1024];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let hex: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(hex == self.spec.sha256)
    }
}

impl LlmProvider for QwenProvider {
    fn model_name(&self) -> &str {
        self.spec.name
    }

    fn size_bytes(&self) -> u64 {
        self.spec.size
    }

    fn is_downloaded(&self) -> bool {
        std::fs::metadata(self.path()).is_ok_and(|m| m.len() == self.spec.size)
    }

    fn partial_bytes(&self) -> u64 {
        std::fs::metadata(self.part_path()).map_or(0, |m| m.len())
    }

    fn download(&self, progress: &mut dyn FnMut(u64, u64), cancel: &AtomicBool) -> Result<()> {
        if self.is_downloaded() {
            return Ok(());
        }
        std::fs::create_dir_all(&self.dir)?;
        let mut last_err = CoreError::Network("no download source".into());
        for url in &self.urls {
            match self.fetch(url, progress, cancel) {
                Ok(()) => {}
                Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
                Err(e) => {
                    log::warn!("download from {url} failed: {e}");
                    last_err = e;
                    continue;
                }
            }
            if self.verify(&self.part_path())? {
                std::fs::rename(self.part_path(), self.path())?;
                return Ok(());
            }
            // 内容不对（镜像文件被替换 / 传输损坏）：丢掉重下，换下一个源
            log::warn!("checksum mismatch from {url}");
            std::fs::remove_file(self.part_path())?;
            last_err = CoreError::Network("checksum mismatch".into());
        }
        Err(last_err)
    }

    fn delete(&self) -> Result<()> {
        for p in [self.path(), self.part_path()] {
            match std::fs::remove_file(&p) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    fn load(&self) -> Result<Arc<dyn LanguageModel>> {
        if !self.is_downloaded() {
            return Err(llm_err("model is not downloaded"));
        }
        Ok(Arc::new(QwenModel::load(&self.path(), self.threads)?))
    }
}

/// 推理线程数：核数的一半（2–4）。ggml 的线程在同步点上自旋等待，线程数等于核数时，
/// 只要界面渲染或 TTS 占住一个核，生成就会被拖慢上百倍（4 核 Android 模拟器上实测：
/// 单独运行 3 秒的句子解析，在 App 里 3 分钟仍未完成）。
fn default_threads() -> i32 {
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    (cores / 2).clamp(2, 4) as i32
}

pub struct QwenModel {
    model: LlamaModel,
    threads: i32,
}

/// Metal 层数：Apple 真机 / Mac 全部放 GPU；iOS 模拟器的 Metal 不完整，用 CPU。
fn gpu_layers() -> u32 {
    if cfg!(all(target_os = "ios", target_abi = "sim")) {
        0
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        99
    } else {
        0
    }
}

impl QwenModel {
    pub fn load(path: &Path, threads: i32) -> Result<Self> {
        let params = LlamaModelParams::default().with_n_gpu_layers(gpu_layers());
        let model = LlamaModel::load_from_file(backend()?, path, &params).map_err(llm_err)?;
        Ok(Self { model, threads })
    }
}

impl LanguageModel for QwenModel {
    fn id(&self) -> &str {
        "qwen2.5-0.5b-instruct"
    }

    fn generate(&self, req: &LlmRequest<'_>, cancel: &AtomicBool) -> Result<String> {
        // Qwen 的 ChatML 对话模板；少样本示例作为之前的对话轮次
        let mut prompt = format!("<|im_start|>system\n{}<|im_end|>\n", req.system);
        for (user, assistant) in req.shots {
            prompt.push_str(&format!(
                "<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n{assistant}<|im_end|>\n"
            ));
        }
        prompt.push_str(&format!(
            "<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
            req.user
        ));
        let vocab = self.model.vocab();
        let tokens = vocab.tokenize(prompt.as_bytes(), false, true);
        let budget = N_CTX as usize - req.max_tokens as usize;
        if tokens.len() > budget {
            return Err(llm_err(format!("prompt too long: {} tokens", tokens.len())));
        }
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(N_CTX))
            .with_n_batch(N_BATCH as u32)
            .with_n_threads(self.threads)
            .with_n_threads_batch(self.threads);
        let mut ctx = self
            .model
            .new_context(backend()?, ctx_params)
            .map_err(llm_err)?;

        let mut batch = LlamaBatch::new(N_BATCH, 1);
        for (c, chunk) in tokens.chunks(N_BATCH).enumerate() {
            batch.clear();
            let last_chunk = (c + 1) * N_BATCH >= tokens.len();
            for (i, &t) in chunk.iter().enumerate() {
                let pos = (c * N_BATCH + i) as i32;
                let logits = last_chunk && i == chunk.len() - 1;
                batch.add(t, pos, &[0], logits).map_err(llm_err)?;
            }
            ctx.decode(&mut batch).map_err(llm_err)?;
        }

        let mut samplers = Vec::new();
        if let Some(g) = req.grammar {
            samplers.push(LlamaSampler::grammar(&self.model, g, "root").map_err(llm_err)?);
        }
        // 轻微的重复惩罚 + 贪心：翻译与解析都要确定、稳定的输出
        samplers.push(LlamaSampler::penalties(
            self.model.n_vocab(),
            64,
            1.1,
            0.0,
            0.0,
        ));
        samplers.push(LlamaSampler::greedy());
        let mut sampler = LlamaSampler::chain_simple(samplers);

        let mut out = Vec::new();
        for pos in (tokens.len() as i32..).take(req.max_tokens as usize) {
            if cancel.load(Ordering::SeqCst) {
                return Err(CoreError::Cancelled);
            }
            // sample 内部已 accept（语法采样器据此推进状态）
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            if vocab.is_eog(token) {
                break;
            }
            out.extend(vocab.token_to_piece(token, false, None));
            batch.clear();
            batch.add(token, pos, &[0], true).map_err(llm_err)?;
            ctx.decode(&mut batch).map_err(llm_err)?;
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }
}
