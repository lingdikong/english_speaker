//! 合成调度：后台线程逐句合成，每句提交后立即可播放（边听边转）。
//!
//! - **断点续转**：已提交的句子记录在 SQL，任务被中断后从第一个未合成的句子继续；
//!   音频文件先截断到最后一个已提交块，丢弃写了一半的数据。
//! - **抢占**：用户点播一篇未缓存的文章时插到队首，当前任务在句边界让出，稍后续转。
//! - 句子严格按顺序合成，因此已合成部分始终是从开头开始的连续区间。

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crossbeam_channel::Sender;

use crate::app::Shared;
use crate::error::{CoreError, Result};
use crate::events::CoreEvent;
use crate::model::*;
use crate::player::PlayerCmd;
use crate::ports::SynthRequest;
use crate::text::{self, connected, PlannedSentence};

const SENTENCE_GAP_MS: u64 = 250;
const PARAGRAPH_GAP_MS: u64 = 650;

#[derive(Default)]
struct Queue {
    items: VecDeque<ArticleId>,
    current: Option<ArticleId>,
    /// 当前任务需在下一个句边界停止（取消或被抢占）。
    stop_current: bool,
}

type SharedQueue = Arc<(Mutex<Queue>, Condvar)>;

pub(crate) struct Synthesizer {
    queue: SharedQueue,
}

enum Outcome {
    Completed,
    Stopped,
}

impl Synthesizer {
    pub fn spawn(shared: Arc<Shared>, player: Sender<PlayerCmd>) -> Self {
        let queue: SharedQueue = Default::default();
        let q = queue.clone();
        thread::Builder::new()
            .name("es-synth".into())
            .spawn(move || worker(shared, q, player))
            .expect("spawn synth thread");
        Self { queue }
    }

    /// 入队。`urgent` 时插到队首，并让当前任务在句边界让出（之后从断点继续）。
    pub fn enqueue(&self, id: ArticleId, urgent: bool) {
        let (lock, cv) = &*self.queue;
        let mut q = lock.lock().unwrap();
        if q.current.as_ref() == Some(&id) && !q.stop_current {
            return; // 已在合成
        }
        let queued = q.items.contains(&id);
        if urgent {
            q.items.retain(|x| *x != id);
            if let Some(cur) = q.current.clone().filter(|c| *c != id && !q.stop_current) {
                q.items.retain(|x| *x != cur);
                q.items.push_front(cur);
                q.stop_current = true;
            }
            q.items.push_front(id);
        } else if !queued {
            q.items.push_back(id);
        }
        cv.notify_all();
    }

    pub fn cancel(&self, id: &str) {
        let mut q = self.queue.0.lock().unwrap();
        q.items.retain(|x| x != id);
        if q.current.as_deref() == Some(id) {
            q.stop_current = true;
        }
    }
}

fn worker(sh: Arc<Shared>, queue: SharedQueue, player: Sender<PlayerCmd>) {
    let (lock, cv) = &*queue;
    loop {
        let id = {
            let mut q = lock.lock().unwrap();
            while q.items.is_empty() {
                q = cv.wait(q).unwrap();
            }
            let id = q.items.pop_front().unwrap();
            q.current = Some(id.clone());
            q.stop_current = false;
            id
        };
        *sh.synthesizing.lock().unwrap() = Some(id.clone());

        let should_stop = || lock.lock().unwrap().stop_current;
        let result = run(&sh, &id, &should_stop, &player);

        *sh.synthesizing.lock().unwrap() = None;
        let (requeued, stopped) = {
            let mut q = lock.lock().unwrap();
            q.current = None;
            (q.items.contains(&id), q.stop_current)
        };
        // 被取消的任务（换音色、删除）在提交时会被存储层拒绝，这类错误不算失败
        let result = match result {
            Err(_) if stopped => Ok(Outcome::Stopped),
            other => other,
        };
        let state = match result {
            Ok(Outcome::Completed) => None,
            Ok(Outcome::Stopped) if requeued => Some((AudioStatus::Queued, None)),
            Ok(Outcome::Stopped) => Some((AudioStatus::Partial, None)),
            Err(e) => {
                log::error!("synthesis of {id} failed: {e}");
                sh.emit(CoreEvent::SynthesisFailed {
                    article_id: id.clone(),
                    message: e.to_string(),
                });
                Some((AudioStatus::Failed, Some(e.to_string())))
            }
        };
        if let Some((state, err)) = state {
            let _ = sh.storage.set_cache_state(&id, state, err.as_deref());
        }
        let _ = player.send(PlayerCmd::DataAvailable(id));
        sh.emit(CoreEvent::LibraryChanged);
    }
}

fn run(
    sh: &Shared,
    id: &str,
    should_stop: &dyn Fn() -> bool,
    player: &Sender<PlayerCmd>,
) -> Result<Outcome> {
    let article = sh
        .storage
        .get_article(id)?
        .ok_or_else(|| CoreError::NotFound(id.into()))?;
    let entry = match sh.storage.get_cache_entry(id)? {
        // 旧编码格式/采样率的缓存直接重建
        Some(e) if e.codec == sh.codec.id() && e.sample_rate == sh.tts.sample_rate() => e,
        _ => {
            sh.remove_cache(id)?;
            let voice = article
                .voice_id
                .clone()
                .unwrap_or_else(|| sh.default_voice.clone());
            sh.new_cache_entry(&article, &voice, AudioStatus::Queued)?
        }
    };
    if entry.state == AudioStatus::Ready {
        return Ok(Outcome::Completed);
    }
    sh.storage
        .set_cache_state(id, AudioStatus::Synthesizing, None)?;
    sh.emit(CoreEvent::LibraryChanged);

    let sentences = sh.storage.list_sentences(id)?;
    let total = sentences.len() as u32;
    let format = entry.format();
    let committed: Vec<Chunk> = sentences.iter().filter_map(|s| s.chunk).collect();
    let mut offset = committed
        .iter()
        .map(|c| c.byte_offset + c.byte_len)
        .max()
        .unwrap_or(0);
    let mut cursor = committed.iter().map(Chunk::end_frame).max().unwrap_or(0);
    let mut ready = committed.len() as u32;

    let british = sh
        .tts
        .voices()
        .iter()
        .any(|v| v.id == entry.voice_id && v.accent == Accent::British);
    let path = sh.audio_path(&entry);
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)?;
    file.set_len(offset)?; // 丢弃上次中断时写了一半的块
    file.seek(SeekFrom::Start(offset))?;

    for (i, rec) in sentences.iter().enumerate() {
        if rec.chunk.is_some() {
            continue;
        }
        if should_stop() {
            file.sync_data()?;
            return Ok(Outcome::Stopped);
        }
        let planned = PlannedSentence {
            paragraph: rec.span.paragraph,
            text: rec.span.text.clone(),
            tokens: text::normalize::tokenize(&rec.span.text),
        };
        let gap_ms = match i {
            0 => 0,
            _ if sentences[i - 1].span.paragraph != rec.span.paragraph => PARAGRAPH_GAP_MS,
            _ => SENTENCE_GAP_MS,
        };
        let gap = format.ms_to_frames(gap_ms) as usize;

        let words = planned.spoken_words();
        let out = sh.tts.synthesize(&SynthRequest {
            text: &planned.spoken(),
            words: &words,
            voice_id: &entry.voice_id,
            speed: 1.0,
        })?;
        let speech_len = out.samples.len() as u64;
        let word_frames = out
            .word_frames
            .filter(|w| w.len() == planned.tokens.len())
            .unwrap_or_else(|| estimate_word_frames(&planned, speech_len));
        let notes = out
            .word_phonemes
            .filter(|p| p.len() == planned.tokens.len())
            .map(|p| {
                let display: Vec<&str> = planned
                    .tokens
                    .iter()
                    .map(|t| &planned.text[t.start..t.end])
                    .collect();
                connected::annotate(&display, &p, british)
            })
            .unwrap_or_default();
        let span = build_span(
            rec.span.index,
            &planned,
            cursor + gap as u64,
            &word_frames,
            &notes,
            format,
        );

        let mut pcm = vec![0f32; gap];
        pcm.extend_from_slice(&out.samples);
        let bytes = sh.codec.encode(&pcm, format)?;
        file.write_all(&bytes)?;
        let chunk = Chunk {
            byte_offset: offset,
            byte_len: bytes.len() as u64,
            start_frame: cursor,
            frames: pcm.len() as u64,
        };
        sh.storage.commit_chunk(&entry, &chunk, &span)?;
        offset += chunk.byte_len;
        cursor += chunk.frames;
        ready += 1;

        sh.emit(CoreEvent::SynthesisProgress {
            article_id: id.to_string(),
            ready,
            total,
            sentence: span,
        });
        let _ = player.send(PlayerCmd::DataAvailable(id.to_string()));
        if let Err(e) = sh.enforce_cache_limit() {
            log::warn!("cache eviction failed: {e}");
        }
    }
    file.sync_all()?;
    sh.storage.set_cache_state(id, AudioStatus::Ready, None)?;
    sh.emit(CoreEvent::SynthesisFinished {
        article_id: id.to_string(),
        duration_ms: format.frames_to_ms(cursor),
    });
    if let Ok(stats) = sh.cache_stats() {
        sh.emit(CoreEvent::CacheChanged { stats });
    }
    Ok(Outcome::Completed)
}

/// 引擎没有给出逐词时长时，按字符数比例分配。
fn estimate_word_frames(s: &PlannedSentence, total: u64) -> Vec<(u64, u64)> {
    let weights: Vec<u64> = s.tokens.iter().map(|t| t.weight() as u64).collect();
    let sum: u64 = weights.iter().sum::<u64>().max(1);
    let mut acc = 0;
    weights
        .iter()
        .map(|w| {
            let start = acc * total / sum;
            acc += w;
            (start, acc * total / sum)
        })
        .collect()
}

fn build_span(
    index: u32,
    s: &PlannedSentence,
    offset: u64,
    word_frames: &[(u64, u64)],
    notes: &[connected::WordNote],
    format: PcmFormat,
) -> SentenceSpan {
    let words: Vec<WordSpan> = s
        .tokens
        .iter()
        .zip(word_frames)
        .enumerate()
        .map(|(i, (t, &(a, b)))| WordSpan {
            text: s.text[t.start..t.end].to_string(),
            char_start: t.start as u32,
            char_end: t.end as u32,
            start_ms: format.frames_to_ms(offset + a),
            end_ms: format.frames_to_ms(offset + b),
            notes: notes.get(i).cloned().unwrap_or_default(),
        })
        .collect();
    let len = word_frames.last().map(|w| w.1).unwrap_or(0);
    SentenceSpan {
        index,
        paragraph: s.paragraph,
        text: s.text.clone(),
        ready: true,
        start_ms: format.frames_to_ms(offset),
        end_ms: format.frames_to_ms(offset + len),
        words,
    }
}
