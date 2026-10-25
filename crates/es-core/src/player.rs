//! 播放引擎：独立线程按块读取缓存、解码并推送给平台 `AudioSink`，
//! 根据设备实际播放帧数计算进度，映射到时间轴（句/词）。
//!
//! **边听边转**：播放器只读取 SQL 中已提交的块。追上合成进度时进入 `Buffering`，
//! 收到合成线程的 `DataAvailable` 后自动恢复，类似网络音频的缓冲。
//!
//! 平台层只负责“把 PCM 放出声音”和系统媒体控件；队列、进度、
//! 断点续播、句子跳转等逻辑全部在这里，所有平台行为一致。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender, TryRecvError};

use crate::app::Shared;
use crate::error::{CoreError, Result};
use crate::events::CoreEvent;
use crate::model::*;
use crate::ports::AudioSink;

const CHUNK_FRAMES: usize = 2048;
const STARVED_POLL: Duration = Duration::from_millis(20);
const POSITION_INTERVAL: Duration = Duration::from_millis(100);
const PROGRESS_SAVE_INTERVAL: Duration = Duration::from_secs(5);
/// 结尾冲刷后 `played_frames` 停止前进多久即视为放完。
///
/// 平台 sink 的帧数统计若因舍入 / 内部缓冲差一点永远追不上 `written`，
/// 播放器会一直卡在播放状态却无声；此后不再依赖精确对齐，强制进入结束状态。
const END_STALL: Duration = Duration::from_millis(500);

pub(crate) enum PlayerCmd {
    Load {
        article_id: ArticleId,
        start_ms: u64,
        autoplay: bool,
    },
    Play,
    Pause,
    Seek(u64),
    SetRate(f32),
    Stop,
    /// 合成线程提交了新块或任务结束。
    DataAvailable(ArticleId),
    /// 合成任务已终止且不会再有新数据（失败或被取消）。
    SynthDone(ArticleId),
}

pub(crate) struct Player {
    tx: Sender<PlayerCmd>,
    state: Arc<Mutex<PlayerState>>,
}

impl Player {
    pub fn spawn(shared: Arc<Shared>, sink: Arc<dyn AudioSink>) -> Self {
        let (tx, rx) = unbounded();
        let state: Arc<Mutex<PlayerState>> = Default::default();
        let format = PcmFormat {
            sample_rate: shared.tts.sample_rate(),
            channels: 1,
        };
        let mut worker = Worker {
            shared,
            sink,
            state: state.clone(),
            format,
            loaded: None,
            status: PlayerStatus::Idle,
            rate: 1.0,
            end_flushed: false,
            last_played: 0,
            last_advance: None,
            synth_done: false,
        };
        thread::Builder::new()
            .name("es-player".into())
            .spawn(move || worker.run(rx))
            .expect("spawn player thread");
        Self { tx, state }
    }

    pub fn sender(&self) -> Sender<PlayerCmd> {
        self.tx.clone()
    }

    pub fn send(&self, cmd: PlayerCmd) {
        let _ = self.tx.send(cmd);
    }

    pub fn state(&self) -> PlayerState {
        self.state.lock().unwrap().clone()
    }
}

/// 当前加载的文章。
struct Loaded {
    article_id: ArticleId,
    estimated_ms: u64,
    file_name: Option<String>,
    file: Option<File>,
    /// 已提交的块（从开头开始连续）。
    chunks: Vec<Chunk>,
    timeline: Timeline,
    complete: bool,
    /// 下一个要送入 sink 的帧。
    next_frame: u64,
    /// 最近一次 seek 的起点帧。
    base_frame: u64,
    /// 自 base 以来已写入 sink 的帧数。
    written: u64,
    decoded: Option<(usize, Vec<f32>)>,
}

impl Loaded {
    fn ready_frames(&self) -> u64 {
        self.chunks.last().map(Chunk::end_frame).unwrap_or(0)
    }
}

struct Worker {
    shared: Arc<Shared>,
    sink: Arc<dyn AudioSink>,
    state: Arc<Mutex<PlayerState>>,
    /// 单声道，采样率与 TTS 引擎一致。
    format: PcmFormat,
    loaded: Option<Loaded>,
    status: PlayerStatus,
    rate: f32,
    /// 本次加载是否已调用过 `end_stream`（结尾冲刷）。
    end_flushed: bool,
    /// 上次读到的 `played_frames`（用于检测结尾停滞）。
    last_played: u64,
    /// `played_frames` 最近一次前进的时刻（仅结尾冲刷后计时）。
    last_advance: Option<Instant>,
    /// 当前文章的合成任务已终止且不会再有新数据（失败 / 被取消）。
    synth_done: bool,
}

impl Worker {
    fn run(&mut self, rx: Receiver<PlayerCmd>) {
        let mut last_position = Instant::now();
        let mut last_save = Instant::now();

        loop {
            let active = matches!(self.status, PlayerStatus::Playing | PlayerStatus::Buffering);
            let cmd = if !active {
                match rx.recv() {
                    Ok(c) => Some(c),
                    Err(_) => break,
                }
            } else if self.starved() {
                match rx.recv_timeout(STARVED_POLL) {
                    Ok(c) => Some(c),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match rx.try_recv() {
                    Ok(c) => Some(c),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => break,
                }
            };
            if let Some(cmd) = cmd {
                if let Err(e) = self.handle(cmd) {
                    log::error!("player command failed: {e}");
                    self.unload();
                }
                continue;
            }

            if let Err(e) = self.pump() {
                log::error!("playback failed: {e}");
                self.unload();
                continue;
            }
            if last_position.elapsed() >= POSITION_INTERVAL {
                last_position = Instant::now();
                self.emit_position();
            }
            if last_save.elapsed() >= PROGRESS_SAVE_INTERVAL {
                last_save = Instant::now();
                self.save_progress();
            }
        }
        self.sink.close();
    }

    /// 已没有可送入 sink 的数据（追上合成进度，或已到结尾）。
    fn starved(&self) -> bool {
        self.loaded
            .as_ref()
            .is_none_or(|l| l.next_frame >= l.ready_frames())
    }

    /// 推送一块 PCM；没有数据时处理缓冲/结束的状态切换。
    fn pump(&mut self) -> Result<()> {
        let Some(l) = self.loaded.as_mut() else {
            return Ok(());
        };
        if l.next_frame < l.ready_frames() {
            let idx = l.chunks.partition_point(|c| c.end_frame() <= l.next_frame);
            let chunk = l.chunks[idx];
            if l.decoded.as_ref().is_none_or(|(i, _)| *i != idx) {
                let pcm = read_chunk(&self.shared, l, &chunk, self.format)?;
                l.decoded = Some((idx, pcm));
            }
            let pcm = &l.decoded.as_ref().unwrap().1;
            let off = ((l.next_frame - chunk.start_frame) as usize).min(pcm.len());
            let n = (pcm.len() - off).min(CHUNK_FRAMES);
            if n == 0 {
                // 块的实际样本数少于记录（不应发生）：跳到下一块
                l.next_frame = chunk.end_frame();
                return Ok(());
            }
            self.sink.write(&pcm[off..off + n])?;
            l.next_frame += n as u64;
            l.written += n as u64;
            if self.status == PlayerStatus::Buffering {
                self.set_status(PlayerStatus::Playing);
            }
            return Ok(());
        }

        if l.complete && l.next_frame >= l.ready_frames() {
            // 全部 PCM 已写入：让 sink 冲出变速器等内部缓冲的残留尾部，
            // 否则 played_frames 永远追不上 written，文章结尾会卡在缓冲状态。
            if !self.end_flushed {
                self.sink.end_stream();
                self.end_flushed = true;
                self.last_advance = Some(Instant::now());
            }
        }
        let played = self.sink.played_frames();
        if played != self.last_played {
            self.last_played = played;
            if self.end_flushed {
                self.last_advance = Some(Instant::now());
            }
        }
        if played < l.written {
            // 结尾兜底：残留尾部已冲出、位置却长时间不再前进，
            // 说明平台 sink 的帧数统计差一点永远追不上（舍入 / 内部缓冲），
            // 此时无声空转，直接判定放完，避免卡在播放状态。
            let stalled =
                self.end_flushed && self.last_advance.is_some_and(|t| t.elapsed() >= END_STALL);
            if !stalled {
                return Ok(()); // sink 里还有没放完的
            }
            log::debug!(
                "end-of-stream stall: played {played} / written {}",
                l.written
            );
        }
        if l.complete {
            let id = l.article_id.clone();
            self.set_status(PlayerStatus::Ended);
            self.emit_position();
            let _ = self.shared.storage.save_progress(&id, 0);
        } else if self.synth_done
            && matches!(self.status, PlayerStatus::Playing | PlayerStatus::Buffering)
        {
            // 合成已终止且读到了现有音频的末尾：停下来，
            // 而不是永远停留在缓冲状态（无声但界面显示播放中）。
            self.sink.pause();
            self.set_status(PlayerStatus::Paused);
            self.emit_position();
            self.save_progress();
        } else if self.status == PlayerStatus::Playing {
            self.set_status(PlayerStatus::Buffering);
        }
        Ok(())
    }

    fn handle(&mut self, cmd: PlayerCmd) -> Result<()> {
        match cmd {
            PlayerCmd::Load {
                article_id,
                start_ms,
                autoplay,
            } => {
                self.save_progress();
                self.unload();
                let article = self
                    .shared
                    .storage
                    .get_article(&article_id)?
                    .ok_or_else(|| CoreError::NotFound(article_id.clone()))?;
                self.sink.open(self.format)?;
                self.sink.set_rate(self.rate);
                *self.shared.now_playing.lock().unwrap() = Some(article_id.clone());
                self.end_flushed = false;
                self.last_played = 0;
                self.last_advance = None;
                self.synth_done = false;
                let mut l = Loaded {
                    article_id,
                    estimated_ms: article.estimated_ms,
                    file_name: None,
                    file: None,
                    chunks: Vec::new(),
                    timeline: Timeline::default(),
                    complete: false,
                    next_frame: 0,
                    base_frame: 0,
                    written: 0,
                    decoded: None,
                };
                refresh(&self.shared, &mut l)?;
                let duration = self.duration_ms(&l);
                self.loaded = Some(l);
                let start = if start_ms + 1000 >= duration {
                    0
                } else {
                    start_ms
                };
                self.seek(start);
                if autoplay {
                    self.set_status(PlayerStatus::Playing);
                } else {
                    self.sink.pause();
                    self.set_status(PlayerStatus::Paused);
                }
            }
            PlayerCmd::Play => {
                if self.loaded.is_none() {
                    return Ok(());
                }
                if self.status == PlayerStatus::Ended {
                    self.seek(0);
                }
                self.sink.resume();
                self.set_status(PlayerStatus::Playing);
            }
            PlayerCmd::Pause => {
                if matches!(self.status, PlayerStatus::Playing | PlayerStatus::Buffering) {
                    self.sink.pause();
                    self.set_status(PlayerStatus::Paused);
                    self.emit_position();
                    self.save_progress();
                }
            }
            PlayerCmd::Seek(ms) => {
                if self.loaded.is_some() {
                    if self.status == PlayerStatus::Ended {
                        self.sink.pause();
                        self.set_status(PlayerStatus::Paused);
                    }
                    self.seek(ms);
                }
            }
            PlayerCmd::SetRate(rate) => {
                let rate = rate.clamp(0.5, 2.0);
                if self.sink.set_rate(rate) {
                    self.rate = rate;
                    self.emit_state();
                }
            }
            PlayerCmd::Stop => {
                self.save_progress();
                self.unload();
            }
            PlayerCmd::DataAvailable(id) => {
                let Some(l) = self.loaded.as_mut().filter(|l| l.article_id == id) else {
                    return Ok(());
                };
                let was_complete = l.complete;
                refresh(&self.shared, l)?;
                // 合成在继续推进（任务恢复或已完成），不再是“已终止”状态
                self.synth_done = false;
                if l.complete != was_complete {
                    self.emit_state();
                }
            }
            PlayerCmd::SynthDone(id) => {
                let Some(l) = self.loaded.as_mut().filter(|l| l.article_id == id) else {
                    return Ok(());
                };
                refresh(&self.shared, l)?;
                // 任务已终止且不会再有新数据：若已读到现有音频的末尾，
                // 停下来而不是永远停留在缓冲（无声但界面显示播放中）。
                if !l.complete {
                    self.synth_done = true;
                }
            }
        }
        Ok(())
    }

    /// 允许跳到尚未合成的位置：进入缓冲，合成追上后自动开始播放。
    fn seek(&mut self, ms: u64) {
        let Some(duration) = self.loaded.as_ref().map(|l| self.duration_ms(l)) else {
            return;
        };
        let frame = self.format.ms_to_frames(ms.min(duration));
        self.sink.flush();
        self.end_flushed = false;
        self.last_played = 0;
        self.last_advance = None;
        if let Some(l) = self.loaded.as_mut() {
            l.base_frame = frame;
            l.next_frame = frame;
            l.written = 0;
        }
        self.emit_position();
    }

    fn unload(&mut self) {
        if self.loaded.take().is_some() {
            self.sink.close();
        }
        *self.shared.now_playing.lock().unwrap() = None;
        self.status = PlayerStatus::Idle;
        self.emit_state();
    }

    fn position_ms(&self) -> u64 {
        match &self.loaded {
            Some(l) => {
                let played = self.sink.played_frames().min(l.written);
                self.format.frames_to_ms(l.base_frame + played)
            }
            None => 0,
        }
    }

    fn duration_ms(&self, l: &Loaded) -> u64 {
        blended_duration_ms(
            l.estimated_ms,
            self.format.frames_to_ms(l.ready_frames()),
            l.chunks.len() as u32,
            l.timeline.sentences.len() as u32,
            l.complete,
        )
    }

    /// 更新并返回共享状态快照。
    fn snapshot(&self) -> PlayerState {
        let mut st = PlayerState {
            status: self.status,
            rate: self.rate,
            ..Default::default()
        };
        if let Some(l) = &self.loaded {
            st.article_id = Some(l.article_id.clone());
            st.position_ms = self.position_ms();
            st.buffered_ms = self.format.frames_to_ms(l.ready_frames());
            st.duration_ms = self.duration_ms(l);
            st.duration_final = l.complete;
        }
        *self.state.lock().unwrap() = st.clone();
        st
    }

    fn emit_position(&self) {
        let st = self.snapshot();
        let Some(l) = &self.loaded else { return };
        let (sentence, word) = l.timeline.locate(st.position_ms);
        self.shared.emit(CoreEvent::PlayerPosition {
            position_ms: st.position_ms,
            buffered_ms: st.buffered_ms,
            sentence,
            word,
        });
    }

    fn save_progress(&self) {
        if let Some(l) = &self.loaded {
            if let Err(e) = self
                .shared
                .storage
                .save_progress(&l.article_id, self.position_ms())
            {
                log::warn!("save progress failed: {e}");
            }
        }
    }

    fn set_status(&mut self, status: PlayerStatus) {
        self.status = status;
        self.emit_state();
    }

    fn emit_state(&self) {
        let state = self.snapshot();
        self.shared.emit(CoreEvent::PlayerStateChanged { state });
    }
}

/// 从 SQL 重新读取已提交的块与时间轴。缓存被重建（换音色）时切换到新文件。
fn refresh(sh: &Shared, l: &mut Loaded) -> Result<()> {
    let entry = sh.storage.get_cache_entry(&l.article_id)?;
    let file_name = entry.as_ref().map(|e| e.file_name.clone());
    if file_name != l.file_name {
        l.file_name = file_name;
        l.file = None;
        l.decoded = None;
    }
    let sentences = sh.storage.list_sentences(&l.article_id)?;
    l.chunks = sentences.iter().map_while(|s| s.chunk).collect();
    l.complete = entry.is_some_and(|e| e.state == AudioStatus::Ready);
    l.timeline = Timeline {
        sentences: sentences.into_iter().map(|s| s.span).collect(),
        complete: l.complete,
    };
    Ok(())
}

fn read_chunk(sh: &Shared, l: &mut Loaded, chunk: &Chunk, format: PcmFormat) -> Result<Vec<f32>> {
    if l.file.is_none() {
        let name = l
            .file_name
            .as_ref()
            .ok_or_else(|| CoreError::Audio("no cache file".into()))?;
        l.file = Some(File::open(sh.audio_dir.join(name))?);
    }
    let file = l.file.as_mut().unwrap();
    file.seek(SeekFrom::Start(chunk.byte_offset))?;
    let mut buf = vec![0u8; chunk.byte_len as usize];
    file.read_exact(&mut buf)?;
    sh.codec.decode(&buf, format)
}
