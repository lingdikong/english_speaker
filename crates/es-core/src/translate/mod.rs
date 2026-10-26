//! 翻译与查词：双模架构，对外只有一套接口（经 `AppCore` 暴露给五个平台）。
//!
//! | 能力       | Fast（默认，随包内置）          | Smart（设置里打开，按需下载）               |
//! |------------|----------------------------------|---------------------------------------------|
//! | 查词       | ECDICT 离线词典，毫秒级          | 同左，另可“举一反三”：语境义 / 搭配 / 例句 |
//! | 整句翻译   | Opus-MT（ONNX），术语占位保护    | Qwen（llama.cpp）以机器译文为参考精修 + 术语表 |
//! | 句子解析   | —                                | 意译 / 句子结构 / 语法点                    |
//!
//! **调度**：一个后台线程 `es-translate`。导入即把文章放进队列，用 Fast 逐句预翻并写入 SQLite；
//! 打开智能翻译后，对正在看的文章（`focus`）再用 Smart 逐句精翻。点词 / 句子解析是“插队”任务，
//! 在两句之间优先处理。译文按 (句, 引擎) 缓存，展示时 Smart 优先、没有时用 Fast。
//!
//! **模型生命周期**：Qwen 由用户在设置中下载（断点续传 + SHA-256 校验）；第一次需要时加载进内存，
//! 关闭智能翻译即释放（约 500MB）。未下载或加载失败时自动退回 Fast，界面不需要分支。

pub mod glossary;
pub mod prompt;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};

use crate::app::now_ms;
use crate::error::{CoreError, Result};
use crate::events::{CoreEvent, EventListener};
use crate::model::*;
use crate::ports::{Dictionary, LanguageModel, LlmProvider, LlmRequest, Storage, Translator};

pub(crate) const SMART_SETTING: &str = "translate.smart";

/// 翻译相关的可选依赖；都为空时翻译功能整体不可用（界面隐藏入口）。
#[derive(Default, Clone)]
pub struct TranslationDeps {
    pub translator: Option<Arc<dyn Translator>>,
    pub dictionary: Option<Arc<dyn Dictionary>>,
    pub llm: Option<Arc<dyn LlmProvider>>,
}

enum Job {
    /// 后台预翻一篇（排队尾）。
    Article(ArticleId),
    /// 用户正在看这篇：插到队首，智能模式下精翻。
    Focus(ArticleId),
    Word {
        article_id: ArticleId,
        sentence: u32,
        word: u32,
    },
    Sentence {
        article_id: ArticleId,
        sentence: u32,
    },
    /// 文章被删除：移出队列。
    Forget(ArticleId),
    /// 术语表或模式变化：重读术语表与当前文章的译文。
    Reload,
}

#[derive(Default)]
struct Download {
    running: bool,
    received: u64,
    total: u64,
    error: Option<String>,
    cancel: Arc<AtomicBool>,
}

struct Inner {
    storage: Arc<dyn Storage>,
    listener: Arc<dyn EventListener>,
    deps: TranslationDeps,
    smart: AtomicBool,
    model: Mutex<Option<Arc<dyn LanguageModel>>>,
    /// 关闭智能翻译时打断正在进行的生成。
    cancel_generation: AtomicBool,
    download: Mutex<Download>,
    focus: Mutex<Option<ArticleId>>,
}

pub(crate) struct Translation {
    inner: Arc<Inner>,
    tx: Sender<Job>,
}

impl Translation {
    pub fn spawn(
        storage: Arc<dyn Storage>,
        listener: Arc<dyn EventListener>,
        deps: TranslationDeps,
    ) -> Result<Self> {
        let smart = storage.get_setting(SMART_SETTING)?.as_deref() == Some("1");
        let inner = Arc::new(Inner {
            storage,
            listener,
            deps,
            smart: AtomicBool::new(smart),
            model: Mutex::new(None),
            cancel_generation: AtomicBool::new(false),
            download: Mutex::new(Download::default()),
            focus: Mutex::new(None),
        });
        let (tx, rx) = crossbeam_channel::unbounded();
        let worker_inner = inner.clone();
        thread::Builder::new()
            .name("es-translate".into())
            .spawn(move || Worker::new(worker_inner, rx).run())
            .map_err(|e| CoreError::Internal(e.to_string()))?;
        let t = Self { inner, tx };
        if t.inner.deps.translator.is_some() || t.inner.smart_ready() {
            // 上次退出时没翻完的文章继续（已翻完的只做一次查询）
            for a in t.inner.storage.list_articles()? {
                t.send(Job::Article(a.id));
            }
        }
        Ok(t)
    }

    fn send(&self, job: Job) {
        let _ = self.tx.send(job);
    }

    pub fn enabled(&self) -> bool {
        self.inner.deps.translator.is_some() || self.inner.deps.llm.is_some()
    }

    pub fn enqueue(&self, id: &str) {
        if self.enabled() {
            self.send(Job::Article(id.to_string()));
        }
    }

    pub fn focus(&self, id: &str) {
        *self.inner.focus.lock().unwrap() = Some(id.to_string());
        if self.enabled() {
            self.send(Job::Focus(id.to_string()));
        }
    }

    pub fn forget(&self, id: &str) {
        let mut focus = self.inner.focus.lock().unwrap();
        if focus.as_deref() == Some(id) {
            *focus = None;
        }
        self.send(Job::Forget(id.to_string()));
    }

    /// 每句一条：智能模式（或已有精翻）优先，否则用 Fast。
    pub fn translations(&self, id: &str) -> Result<Vec<SentenceTranslation>> {
        let mut best: Vec<SentenceTranslation> = Vec::new();
        for t in self.inner.storage.list_translations(id)? {
            match best.last_mut() {
                Some(prev) if prev.index == t.index => {
                    if t.engine == TranslationEngine::Smart {
                        *prev = t;
                    }
                }
                _ => best.push(t),
            }
        }
        Ok(best)
    }

    // ---------- 查词与智能解析 ----------

    pub fn lookup(&self, word: &str) -> Result<Option<DictEntry>> {
        let word = clean_word(word);
        match (&self.inner.deps.dictionary, word.is_empty()) {
            (Some(d), false) => d.lookup(word),
            _ => Ok(None),
        }
    }

    pub fn word_insight(
        &self,
        article_id: &str,
        sentence: u32,
        word: u32,
    ) -> Result<Option<WordInsight>> {
        if let Some(json) = self
            .inner
            .storage
            .get_ai_note(article_id, sentence, Some(word))?
        {
            if let Ok(w) = serde_json::from_str(&json) {
                return Ok(Some(w));
            }
        }
        self.require_smart()?;
        self.send(Job::Word {
            article_id: article_id.to_string(),
            sentence,
            word,
        });
        Ok(None)
    }

    pub fn sentence_analysis(
        &self,
        article_id: &str,
        sentence: u32,
    ) -> Result<Option<SentenceAnalysis>> {
        if let Some(json) = self.inner.storage.get_ai_note(article_id, sentence, None)? {
            if let Ok(a) = serde_json::from_str(&json) {
                return Ok(Some(a));
            }
        }
        self.require_smart()?;
        self.send(Job::Sentence {
            article_id: article_id.to_string(),
            sentence,
        });
        Ok(None)
    }

    fn require_smart(&self) -> Result<()> {
        if self.inner.smart_ready() {
            Ok(())
        } else {
            Err(CoreError::InvalidInput(
                "smart translation is off or its model is not downloaded".into(),
            ))
        }
    }

    // ---------- 模式与模型 ----------

    pub fn status(&self) -> TranslatorStatus {
        self.inner.status()
    }

    pub fn set_smart(&self, enabled: bool) -> Result<()> {
        self.inner
            .storage
            .set_setting(SMART_SETTING, if enabled { "1" } else { "0" })?;
        self.inner.smart.store(enabled, Ordering::SeqCst);
        if enabled {
            if let Some(id) = self.inner.focus.lock().unwrap().clone() {
                self.send(Job::Focus(id));
            }
        } else {
            self.inner.cancel_generation.store(true, Ordering::SeqCst);
            // 释放模型内存；正在生成的任务持有的引用会在打断后释放
            *self.inner.model.lock().unwrap() = None;
        }
        self.inner.emit_status();
        Ok(())
    }

    /// 开始下载智能翻译模型（后台线程，断点续传）。已下载或正在下载时什么也不做。
    pub fn download_llm(&self) -> Result<()> {
        let Some(llm) = self.inner.deps.llm.clone() else {
            return Err(CoreError::InvalidInput(
                "smart translation is not available in this build".into(),
            ));
        };
        let cancel = {
            let mut d = self.inner.download.lock().unwrap();
            if d.running || llm.is_downloaded() {
                return Ok(());
            }
            d.running = true;
            d.error = None;
            d.received = llm.partial_bytes();
            d.total = llm.size_bytes();
            d.cancel = Arc::new(AtomicBool::new(false));
            d.cancel.clone()
        };
        self.inner.emit_status();
        let inner = self.inner.clone();
        let tx = self.tx.clone();
        thread::Builder::new()
            .name("es-llm-download".into())
            .spawn(move || {
                let mut last = Instant::now();
                let result = llm.download(
                    &mut |received, total| {
                        {
                            let mut d = inner.download.lock().unwrap();
                            d.received = received;
                            d.total = total;
                        }
                        if last.elapsed() >= Duration::from_millis(250) {
                            last = Instant::now();
                            inner.emit_status();
                        }
                    },
                    &cancel,
                );
                {
                    let mut d = inner.download.lock().unwrap();
                    d.running = false;
                    d.error = match &result {
                        Ok(()) | Err(CoreError::Cancelled) => None,
                        Err(e) => Some(e.to_string()),
                    };
                }
                if let Err(e) = &result {
                    log::warn!("llm download: {e}");
                }
                if result.is_ok() && inner.smart.load(Ordering::SeqCst) {
                    if let Some(id) = inner.focus.lock().unwrap().clone() {
                        let _ = tx.send(Job::Focus(id));
                    }
                }
                inner.emit_status();
            })
            .map_err(|e| CoreError::Internal(e.to_string()))?;
        Ok(())
    }

    pub fn cancel_llm_download(&self) {
        self.inner
            .download
            .lock()
            .unwrap()
            .cancel
            .store(true, Ordering::SeqCst);
    }

    /// 删除已下载的模型（含未完成的下载），智能翻译退回 Fast。
    pub fn delete_llm(&self) -> Result<()> {
        self.cancel_llm_download();
        self.inner.cancel_generation.store(true, Ordering::SeqCst);
        *self.inner.model.lock().unwrap() = None;
        if let Some(llm) = &self.inner.deps.llm {
            llm.delete()?;
        }
        self.inner.download.lock().unwrap().error = None;
        self.inner.emit_status();
        Ok(())
    }

    // ---------- 术语表 ----------

    pub fn list_glossary(&self) -> Result<Vec<GlossaryTerm>> {
        self.inner.storage.list_glossary()
    }

    pub fn add_glossary(&self, term: &str, translation: &str) -> Result<GlossaryTerm> {
        let (term, translation) = (term.trim(), translation.trim());
        if term.is_empty() || translation.is_empty() {
            return Err(CoreError::InvalidInput(
                "glossary term and translation must not be empty".into(),
            ));
        }
        let entry = GlossaryTerm {
            term: term.to_string(),
            translation: translation.to_string(),
            created_at_ms: now_ms(),
        };
        self.inner.storage.upsert_glossary(&entry)?;
        self.retranslate(term)?;
        Ok(entry)
    }

    pub fn remove_glossary(&self, term: &str) -> Result<()> {
        self.inner.storage.delete_glossary(term)?;
        self.retranslate(term)
    }

    /// 术语表变化后，含该术语的句子重新翻译。
    fn retranslate(&self, term: &str) -> Result<()> {
        let affected = self.inner.storage.invalidate_translations(term)?;
        self.send(Job::Reload);
        for id in affected {
            self.inner.emit(CoreEvent::TranslationsChanged {
                article_id: id.clone(),
            });
            self.enqueue(&id);
        }
        Ok(())
    }
}

/// 去掉词两端的标点（保留词内的 ' 与 -）。
pub(crate) fn clean_word(w: &str) -> &str {
    w.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '’' && c != '-')
        .trim_matches(|c| c == '\'' || c == '’' || c == '-')
}

impl Inner {
    fn emit(&self, event: CoreEvent) {
        self.listener.on_event(event);
    }

    fn emit_status(&self) {
        self.emit(CoreEvent::TranslatorChanged {
            status: self.status(),
        });
    }

    fn smart_ready(&self) -> bool {
        self.smart.load(Ordering::SeqCst)
            && self.deps.llm.as_ref().is_some_and(|l| l.is_downloaded())
    }

    fn status(&self) -> TranslatorStatus {
        let d = self.download.lock().unwrap();
        let llm = match &self.deps.llm {
            Some(p) => {
                let downloaded = p.is_downloaded();
                LlmStatus {
                    state: if d.running {
                        LlmState::Downloading
                    } else if downloaded {
                        LlmState::Downloaded
                    } else if d.error.is_some() {
                        LlmState::Failed
                    } else {
                        LlmState::NotDownloaded
                    },
                    model_name: p.model_name().to_string(),
                    downloaded_bytes: if d.running {
                        d.received
                    } else if downloaded {
                        p.size_bytes()
                    } else {
                        p.partial_bytes()
                    },
                    total_bytes: p.size_bytes(),
                    loaded: self.model.lock().unwrap().is_some(),
                    error: d.error.clone(),
                }
            }
            None => LlmStatus {
                state: LlmState::NotDownloaded,
                model_name: String::new(),
                downloaded_bytes: 0,
                total_bytes: 0,
                loaded: false,
                error: None,
            },
        };
        drop(d);
        let fast = self.deps.translator.is_some();
        let active = if self.smart_ready() {
            Some(TranslationEngine::Smart)
        } else if fast {
            Some(TranslationEngine::Fast)
        } else {
            None
        };
        TranslatorStatus {
            dictionary: self.deps.dictionary.is_some(),
            fast,
            smart_enabled: self.smart.load(Ordering::SeqCst),
            active,
            llm,
        }
    }

    /// 已加载的模型；未加载时在当前（后台）线程加载。
    fn model(&self) -> Result<Arc<dyn LanguageModel>> {
        if let Some(m) = self.model.lock().unwrap().clone() {
            return Ok(m);
        }
        let provider = self
            .deps
            .llm
            .as_ref()
            .ok_or_else(|| CoreError::Translation("no language model".into()))?;
        let t = Instant::now();
        let model = provider.load()?;
        log::info!("llm {} loaded in {:?}", model.id(), t.elapsed());
        if !self.smart.load(Ordering::SeqCst) {
            // 加载期间用户关掉了智能翻译：不留在内存里
            return Err(CoreError::Cancelled);
        }
        *self.model.lock().unwrap() = Some(model.clone());
        self.emit_status();
        Ok(model)
    }

    fn generate(&self, req: &LlmRequest<'_>) -> Result<String> {
        let model = self.model()?;
        self.cancel_generation.store(false, Ordering::SeqCst);
        model.generate(req, &self.cancel_generation)
    }
}

/// 当前正在翻译的文章（工作线程内缓存，避免每句都查库）。
struct Doc {
    id: ArticleId,
    sentences: Vec<String>,
    /// 机器译文（精翻时作参考）。
    fast: HashMap<u32, String>,
    smart: HashSet<u32>,
}

impl Doc {
    fn translated(&self) -> u32 {
        (0..self.sentences.len() as u32)
            .filter(|i| self.fast.contains_key(i) || self.smart.contains(i))
            .count() as u32
    }
}

struct Worker {
    inner: Arc<Inner>,
    rx: Receiver<Job>,
    articles: VecDeque<ArticleId>,
    urgent: VecDeque<Job>,
    doc: Option<Doc>,
    glossary: Option<Vec<GlossaryTerm>>,
}

impl Worker {
    fn new(inner: Arc<Inner>, rx: Receiver<Job>) -> Self {
        Self {
            inner,
            rx,
            articles: VecDeque::new(),
            urgent: VecDeque::new(),
            doc: None,
            glossary: None,
        }
    }

    fn run(mut self) {
        loop {
            if self.urgent.is_empty() && self.articles.is_empty() {
                match self.rx.recv() {
                    Ok(job) => self.accept(job),
                    Err(_) => return,
                }
            }
            while let Ok(job) = self.rx.try_recv() {
                self.accept(job);
            }
            if let Some(job) = self.urgent.pop_front() {
                self.run_urgent(job);
                continue;
            }
            let Some(id) = self.articles.front().cloned() else {
                continue;
            };
            match self.step(&id) {
                Ok(true) => {}
                Ok(false) => {
                    self.articles.pop_front();
                }
                Err(e) => {
                    log::warn!("translate {id}: {e}");
                    self.articles.pop_front();
                }
            }
        }
    }

    fn accept(&mut self, job: Job) {
        match job {
            Job::Article(id) => {
                if !self.articles.contains(&id) {
                    self.articles.push_back(id);
                }
            }
            Job::Focus(id) => {
                self.articles.retain(|x| *x != id);
                self.articles.push_front(id);
            }
            Job::Forget(id) => {
                self.articles.retain(|x| *x != id);
                self.urgent.retain(|j| match j {
                    Job::Word { article_id, .. } | Job::Sentence { article_id, .. } => {
                        *article_id != id
                    }
                    _ => true,
                });
                if self.doc.as_ref().is_some_and(|d| d.id == id) {
                    self.doc = None;
                }
            }
            Job::Reload => {
                self.glossary = None;
                self.doc = None;
            }
            job @ (Job::Word { .. } | Job::Sentence { .. }) => {
                let dup = self.urgent.iter().any(|j| same_urgent(j, &job));
                if !dup {
                    self.urgent.push_back(job);
                }
            }
        }
    }

    fn glossary(&mut self) -> Vec<GlossaryTerm> {
        if self.glossary.is_none() {
            self.glossary = Some(self.inner.storage.list_glossary().unwrap_or_default());
        }
        self.glossary.clone().unwrap_or_default()
    }

    fn load_doc(&mut self, id: &str) -> Result<()> {
        if self.doc.as_ref().is_some_and(|d| d.id == id) {
            return Ok(());
        }
        let sentences = self
            .inner
            .storage
            .list_sentences(id)?
            .into_iter()
            .map(|s| s.span.text)
            .collect();
        let mut doc = Doc {
            id: id.to_string(),
            sentences,
            fast: HashMap::new(),
            smart: HashSet::new(),
        };
        for t in self.inner.storage.list_translations(id)? {
            match t.engine {
                TranslationEngine::Fast => {
                    doc.fast.insert(t.index, t.text);
                }
                TranslationEngine::Smart => {
                    doc.smart.insert(t.index);
                }
            }
        }
        self.doc = Some(doc);
        Ok(())
    }

    /// 翻译一句；返回这篇是否还有要做的。
    fn step(&mut self, id: &str) -> Result<bool> {
        self.load_doc(id)?;
        let glossary = self.glossary();
        let smart =
            self.inner.smart_ready() && self.inner.focus.lock().unwrap().as_deref() == Some(id);
        let doc = self.doc.as_ref().expect("doc loaded");
        let n = doc.sentences.len() as u32;
        // 第一遍：每句至少有一条译文（Fast 优先，没有 MT 时用 Smart）
        let missing = (0..n).find(|i| !doc.fast.contains_key(i) && !doc.smart.contains(i));
        // 第二遍：正在看的文章用 Smart 精翻
        let refine = smart
            .then(|| (0..n).find(|i| !doc.smart.contains(i)))
            .flatten();
        let (index, engine) = match (missing, refine, &self.inner.deps.translator) {
            (Some(i), _, Some(_)) => (i, TranslationEngine::Fast),
            (Some(i), _, None) if smart => (i, TranslationEngine::Smart),
            (None, Some(i), _) => (i, TranslationEngine::Smart),
            _ => return Ok(false),
        };
        let text = doc.sentences[index as usize].clone();
        let reference = doc.fast.get(&index).cloned();
        let out = match engine {
            TranslationEngine::Fast => self.translate_fast(&text, &glossary),
            TranslationEngine::Smart => {
                self.translate_smart(&text, reference.as_deref(), &glossary)
            }
        };
        let out = match out {
            Ok(t) => t,
            Err(CoreError::Cancelled) => return Ok(true),
            Err(e) if engine == TranslationEngine::Smart => {
                // 智能模式出错（如内存不足加载失败）：本篇不再精翻，Fast 译文仍在
                log::warn!("smart translation failed, keeping fast: {e}");
                return Ok(missing.is_some());
            }
            Err(e) => return Err(e),
        };
        self.inner
            .storage
            .put_translation(id, index, engine, &out)?;
        let doc = self.doc.as_mut().expect("doc loaded");
        match engine {
            TranslationEngine::Fast => {
                doc.fast.insert(index, out.clone());
            }
            TranslationEngine::Smart => {
                doc.smart.insert(index);
            }
        }
        let translated = doc.translated();
        self.inner.emit(CoreEvent::TranslationProgress {
            article_id: id.to_string(),
            translation: SentenceTranslation {
                index,
                text: out,
                engine,
            },
            translated,
            total: n,
        });
        Ok(true)
    }

    fn translate_fast(&self, text: &str, glossary: &[GlossaryTerm]) -> Result<String> {
        let mt = self
            .inner
            .deps
            .translator
            .as_ref()
            .ok_or_else(|| CoreError::Translation("no machine translator".into()))?;
        if let Some(p) = glossary::protect(text, glossary) {
            if let Some(out) = glossary::restore(&mt.translate(&p.text)?, &p.slots) {
                return Ok(out);
            }
            log::debug!("glossary placeholder lost, translating without it");
        }
        mt.translate(text)
    }

    fn translate_smart(
        &self,
        text: &str,
        reference: Option<&str>,
        glossary: &[GlossaryTerm],
    ) -> Result<String> {
        let terms = glossary::prompt_lines(text, glossary);
        let p = prompt::translate(text, reference, &terms);
        let raw = self.inner.generate(&p.request())?;
        let out = prompt::clean_translation(&raw);
        if out.is_empty() {
            return Err(CoreError::Translation("empty translation".into()));
        }
        Ok(out)
    }

    fn run_urgent(&mut self, job: Job) {
        match job {
            Job::Word {
                article_id,
                sentence,
                word,
            } => {
                let result = self.word_insight(&article_id, sentence, word);
                let (insight, error) = split(result);
                self.inner.emit(CoreEvent::WordInsightReady {
                    article_id,
                    sentence,
                    word,
                    insight,
                    error,
                });
            }
            Job::Sentence {
                article_id,
                sentence,
            } => {
                let result = self.sentence_analysis(&article_id, sentence);
                let (analysis, error) = split(result);
                self.inner.emit(CoreEvent::SentenceAnalysisReady {
                    article_id,
                    sentence,
                    analysis,
                    error,
                });
            }
            _ => {}
        }
    }

    fn sentence_text(&self, article_id: &str, index: u32) -> Result<SentenceSpan> {
        self.inner
            .storage
            .list_sentences(article_id)?
            .into_iter()
            .map(|s| s.span)
            .find(|s| s.index == index)
            .ok_or_else(|| CoreError::NotFound(format!("sentence {index}")))
    }

    fn word_insight(&self, article_id: &str, sentence: u32, word: u32) -> Result<WordInsight> {
        let span = self.sentence_text(article_id, sentence)?;
        let w = span
            .words
            .get(word as usize)
            .map(|w| clean_word(&w.text).to_string())
            .filter(|w| !w.is_empty())
            .ok_or_else(|| CoreError::NotFound(format!("word {word}")))?;
        let dict = self
            .inner
            .deps
            .dictionary
            .as_ref()
            .and_then(|d| d.lookup(&w).ok().flatten());
        let raw = self
            .inner
            .generate(&prompt::word(&w, &span.text, dict.as_ref()).request())?;
        let insight = prompt::parse_word(&w, &raw)
            .ok_or_else(|| CoreError::Translation("model returned malformed JSON".into()))?;
        let json =
            serde_json::to_string(&insight).map_err(|e| CoreError::Internal(e.to_string()))?;
        self.inner
            .storage
            .put_ai_note(article_id, sentence, Some(word), &json)?;
        Ok(insight)
    }

    fn sentence_analysis(&mut self, article_id: &str, sentence: u32) -> Result<SentenceAnalysis> {
        let span = self.sentence_text(article_id, sentence)?;
        let terms = glossary::prompt_lines(&span.text, &self.glossary());
        let reference = self
            .inner
            .storage
            .list_translations(article_id)?
            .into_iter()
            .find(|t| t.index == sentence && t.engine == TranslationEngine::Fast)
            .map(|t| t.text);
        let raw = self
            .inner
            .generate(&prompt::sentence(&span.text, reference.as_deref(), &terms).request())?;
        let analysis = prompt::parse_sentence(&span.text, &raw)
            .ok_or_else(|| CoreError::Translation("model returned malformed JSON".into()))?;
        let json =
            serde_json::to_string(&analysis).map_err(|e| CoreError::Internal(e.to_string()))?;
        self.inner
            .storage
            .put_ai_note(article_id, sentence, None, &json)?;
        Ok(analysis)
    }
}

fn same_urgent(a: &Job, b: &Job) -> bool {
    match (a, b) {
        (
            Job::Word {
                article_id: a1,
                sentence: s1,
                word: w1,
            },
            Job::Word {
                article_id: a2,
                sentence: s2,
                word: w2,
            },
        ) => a1 == a2 && s1 == s2 && w1 == w2,
        (
            Job::Sentence {
                article_id: a1,
                sentence: s1,
            },
            Job::Sentence {
                article_id: a2,
                sentence: s2,
            },
        ) => a1 == a2 && s1 == s2,
        _ => false,
    }
}

fn split<T>(r: Result<T>) -> (Option<T>, Option<String>) {
    match r {
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::clean_word;

    #[test]
    fn cleans_word_punctuation() {
        assert_eq!(clean_word("“Hello,"), "Hello");
        assert_eq!(clean_word("don't."), "don't");
        assert_eq!(clean_word("well-known—"), "well-known");
        assert_eq!(clean_word("'tis'"), "tis");
        assert_eq!(clean_word("..."), "");
    }
}
