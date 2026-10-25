//! 桌面端 `AudioSink`：cpal（Windows WASAPI / macOS CoreAudio / Linux ALSA·PulseAudio）。
//!
//! cpal 的 `Stream` 在部分平台上不是 `Send`，因此由专用线程持有，
//! 通过通道控制暂停/恢复/关闭；PCM 经共享队列传给声卡回调。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use es_runtime::es_core::model::PcmFormat;
use es_runtime::es_core::ports::AudioSink;
use es_runtime::es_core::{CoreError, Result};

/// `write` 在队列超过该时长时阻塞。
const MAX_BUFFER_MS: u64 = 200;

enum Ctrl {
    Pause,
    Resume,
    Close,
}

#[derive(Default)]
struct Buf {
    queue: VecDeque<f32>,
    /// 自上次 open/flush 以来声卡消费的设备帧数。
    consumed: u64,
    src: Option<PcmFormat>,
    dev_rate: u32,
    dev_channels: u16,
    resampler: Resampler,
    /// 保持音调的变速器（源采样率域，位于重采样之前）。
    stretcher: Stretcher,
    open: bool,
}

/// 输出音量（0.0–1.0），声卡回调里按样本相乘。由 UI 的音量滑块控制。
pub struct Volume(AtomicU32);

impl Default for Volume {
    fn default() -> Self {
        Self(AtomicU32::new(1.0f32.to_bits()))
    }
}

impl Volume {
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    pub fn set(&self, v: f32) {
        self.0.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
}

#[derive(Default)]
struct Shared {
    buf: Mutex<Buf>,
    cv: Condvar,
    volume: Arc<Volume>,
    /// 播放倍速（0.5–2.0），跨 open/flush 保持。f32 以位存储。
    rate: AtomicU32,
}

impl Shared {
    fn rate(&self) -> f32 {
        f32::from_bits(self.rate.load(Ordering::Relaxed))
    }

    fn set_rate(&self, rate: f32) {
        self.rate.store(rate.to_bits(), Ordering::Relaxed);
    }
}

#[derive(Default)]
pub struct CpalSink {
    shared: Arc<Shared>,
    ctrl: Mutex<Option<mpsc::Sender<Ctrl>>>,
}

impl CpalSink {
    pub fn with_volume(volume: Arc<Volume>) -> Self {
        Self {
            shared: Arc::new(Shared {
                volume,
                ..Default::default()
            }),
            ctrl: Mutex::default(),
        }
    }

    fn send(&self, c: Ctrl) {
        if let Some(tx) = self.ctrl.lock().unwrap().as_ref() {
            let _ = tx.send(c);
        }
    }
}

impl AudioSink for CpalSink {
    fn open(&self, format: PcmFormat) -> Result<()> {
        self.close();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (ctrl_tx, ctrl_rx) = mpsc::channel();
        let shared = self.shared.clone();
        thread::Builder::new()
            .name("es-audio-out".into())
            .spawn(move || stream_thread(shared, ready_tx, ctrl_rx))
            .map_err(CoreError::Io)?;
        let (dev_rate, dev_channels) = ready_rx
            .recv()
            .map_err(|_| CoreError::Audio("audio thread exited".into()))??;

        let mut b = self.shared.buf.lock().unwrap();
        *b = Buf {
            src: Some(format),
            dev_rate,
            dev_channels,
            resampler: Resampler::new(format.sample_rate, dev_rate, format.channels),
            stretcher: Stretcher::new(
                format.channels as usize,
                format.sample_rate,
                self.shared.rate() as f64,
            ),
            open: true,
            ..Default::default()
        };
        *self.ctrl.lock().unwrap() = Some(ctrl_tx);
        Ok(())
    }

    fn write(&self, samples: &[f32]) -> Result<()> {
        let mut b = self.shared.buf.lock().unwrap();
        let limit = (b.dev_rate as u64 * b.dev_channels as u64 * MAX_BUFFER_MS / 1000) as usize;
        while b.open && b.queue.len() > limit {
            b = self
                .shared
                .cv
                .wait_timeout(b, Duration::from_millis(50))
                .unwrap()
                .0;
        }
        if !b.open {
            return Ok(());
        }
        let Buf {
            stretcher,
            resampler,
            queue,
            dev_channels,
            ..
        } = &mut *b;
        // 先做保持音调的时间伸缩（源采样率域），再重采样到设备采样率
        let mut stretched = Vec::new();
        stretcher.process(samples, &mut stretched);
        resampler.process(&stretched, *dev_channels, queue);
        Ok(())
    }

    fn pause(&self) {
        self.send(Ctrl::Pause);
    }

    fn resume(&self) {
        self.send(Ctrl::Resume);
    }

    fn flush(&self) {
        let mut b = self.shared.buf.lock().unwrap();
        b.queue.clear();
        b.consumed = 0;
        if let Some(src) = b.src {
            b.resampler = Resampler::new(src.sample_rate, b.dev_rate, src.channels);
            b.stretcher = Stretcher::new(
                src.channels as usize,
                src.sample_rate,
                self.shared.rate() as f64,
            );
        }
        self.shared.cv.notify_all();
    }

    fn played_frames(&self) -> u64 {
        let b = self.shared.buf.lock().unwrap();
        match b.src {
            Some(src) if b.dev_rate > 0 => {
                // 声卡消费的是变速后的设备帧：换回源帧数需乘倍速
                (b.consumed as f64 * src.sample_rate as f64 / b.dev_rate as f64
                    * self.shared.rate() as f64) as u64
            }
            _ => 0,
        }
    }

    fn set_rate(&self, rate: f32) -> bool {
        // WSOLA 时间伸缩在 sink 内实现，任意倍速都支持
        let rate = rate.clamp(0.5, 2.0);
        self.shared.set_rate(rate);
        self.shared
            .buf
            .lock()
            .unwrap()
            .stretcher
            .set_rate(rate as f64);
        true
    }

    fn end_stream(&self) {
        let mut stretched = Vec::new();
        {
            let mut b = self.shared.buf.lock().unwrap();
            if !b.open {
                return;
            }
            b.stretcher.flush_end(&mut stretched);
            let Buf {
                resampler,
                queue,
                dev_channels,
                ..
            } = &mut *b;
            resampler.process(&stretched, *dev_channels, queue);
        }
        self.shared.cv.notify_all();
    }

    fn close(&self) {
        if let Some(tx) = self.ctrl.lock().unwrap().take() {
            let _ = tx.send(Ctrl::Close);
        }
        let mut b = self.shared.buf.lock().unwrap();
        b.open = false;
        b.queue.clear();
        self.shared.cv.notify_all();
    }
}

fn stream_thread(
    shared: Arc<Shared>,
    ready: mpsc::Sender<Result<(u32, u16)>>,
    ctrl: mpsc::Receiver<Ctrl>,
) {
    let stream = match build_stream(&shared) {
        Ok((stream, rate, ch)) => {
            let _ = ready.send(Ok((rate, ch)));
            stream
        }
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    for c in ctrl {
        let r = match c {
            Ctrl::Pause => stream.pause(),
            Ctrl::Resume => stream.play(),
            Ctrl::Close => break,
        };
        if let Err(e) = r {
            log::warn!("audio stream control failed: {e}");
        }
    }
}

fn build_stream(shared: &Arc<Shared>) -> Result<(cpal::Stream, u32, u16)> {
    let audio_err = |e: cpal::Error| CoreError::Audio(e.to_string());
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| CoreError::Audio("no output device".into()))?;
    let supported = device.default_output_config().map_err(audio_err)?;
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let (rate, ch) = (config.sample_rate, config.channels);
    let stream = match format {
        SampleFormat::F32 => build_typed::<f32>(&device, config, shared.clone()),
        SampleFormat::I16 => build_typed::<i16>(&device, config, shared.clone()),
        SampleFormat::U16 => build_typed::<u16>(&device, config, shared.clone()),
        SampleFormat::I32 => build_typed::<i32>(&device, config, shared.clone()),
        other => {
            return Err(CoreError::Audio(format!(
                "unsupported sample format {other}"
            )))
        }
    }
    .map_err(audio_err)?;
    stream.play().map_err(audio_err)?;
    Ok((stream, rate, ch))
}

fn build_typed<T>(
    device: &cpal::Device,
    config: StreamConfig,
    shared: Arc<Shared>,
) -> std::result::Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    device.build_output_stream(
        config,
        move |out: &mut [T], _| {
            let gain = shared.volume.get();
            let mut b = shared.buf.lock().unwrap();
            let available = b.queue.len().min(out.len());
            for (slot, s) in out.iter_mut().zip(b.queue.drain(..available)) {
                *slot = T::from_sample(s * gain);
            }
            for slot in &mut out[available..] {
                *slot = T::from_sample(0.0);
            }
            b.consumed += (available / channels) as u64;
            drop(b);
            shared.cv.notify_all();
        },
        |e| log::warn!("audio stream error: {e}"),
        None,
    )
}

/// 线性插值重采样 + 声道映射（单声道复制到所有声道）。语音场景足够。
#[derive(Default)]
struct Resampler {
    step: f64,
    pos: f64,
    channels: usize,
    prev: Vec<f32>,
}

impl Resampler {
    fn new(src_rate: u32, dev_rate: u32, channels: u16) -> Self {
        Self {
            step: src_rate as f64 / dev_rate.max(1) as f64,
            pos: 0.0,
            channels: channels as usize,
            prev: vec![0.0; channels as usize],
        }
    }

    fn process(&mut self, input: &[f32], dev_channels: u16, out: &mut VecDeque<f32>) {
        let ch = self.channels.max(1);
        let frames = input.len() / ch;
        if frames == 0 {
            return;
        }
        let at = |i: isize, c: usize| {
            if i < 0 {
                self.prev[c]
            } else {
                input[i as usize * ch + c]
            }
        };
        while self.pos < (frames - 1) as f64 {
            let i = self.pos.floor() as isize;
            let t = (self.pos - i as f64) as f32;
            for d in 0..dev_channels as usize {
                let c = if ch == 1 { 0 } else { d.min(ch - 1) };
                out.push_back(at(i, c) + (at(i + 1, c) - at(i, c)) * t);
            }
            self.pos += self.step;
        }
        self.pos -= frames as f64;
        self.prev
            .copy_from_slice(&input[(frames - 1) * ch..frames * ch]);
    }
}

/// 保持音调的变速（WSOLA：波形相似叠加）。
///
/// 以固定的输出跳距 `h` 为节拍：每输出 `h` 帧（源采样率域）消耗 `h·rate` 帧输入。
/// 在理想位置 ±`delta` 范围内搜索与上一段“自然延续”最相似的输入起点再拼接，
/// 避免直接抽取/丢弃样本造成的相位断裂（变调）。输出经 Hann 窗交叠相加，能量恒定。
///
/// 延迟约一个窗长（40ms）：输入尾部滞留在缓冲里，流结束时由 [`Stretcher::flush_end`] 冲出。
struct Stretcher {
    channels: usize,
    /// 播放倍速（0.5–2.0），>1 加快。
    rate: f64,
    /// 窗长（帧，约 40ms）。
    n: usize,
    /// 输出跳距 = n/2（约 20ms）。
    h: usize,
    /// 相似搜索半径（帧，约 10ms）。
    delta: usize,
    /// 下一个输出窗的理想输入位置（相对 `buf` 开头，帧，可为小数）。
    pos: f64,
    /// 尚未消费的输入 PCM（交织）。
    buf: Vec<f32>,
    /// 上一窗的自然延续（n-h 帧），相似搜索的目标。
    nat: Vec<f32>,
    /// 上一输出窗的后半（h 帧），等待与下一窗交叠相加。
    tail: Vec<f32>,
    /// 周期 Hann 窗（n 点，跳距 h 下能量和恒为 1）。
    win: Vec<f32>,
    /// 累计输入/已输出帧数（源采样率域），flush 时据此补齐尾部。
    fed: u64,
    emitted: u64,
}

impl Default for Stretcher {
    fn default() -> Self {
        Self::new(1, 24_000, 1.0)
    }
}

impl Stretcher {
    fn new(channels: usize, src_rate: u32, rate: f64) -> Self {
        let n = (((src_rate as f64) * 0.040).round() as usize)
            .max(2)
            .next_power_of_two();
        let win: Vec<f32> = (0..n)
            .map(|i| 0.5 * (1.0 - (std::f64::consts::TAU * i as f64 / n as f64).cos()) as f32)
            .collect();
        Self {
            channels: channels.max(1),
            rate: rate.clamp(0.5, 2.0),
            n,
            h: n / 2,
            delta: ((src_rate as f64) * 0.010).round() as usize,
            pos: 0.0,
            buf: Vec::new(),
            nat: Vec::new(),
            tail: vec![0.0; n / 2 * channels.max(1)],
            win,
            fed: 0,
            emitted: 0,
        }
    }

    fn set_rate(&mut self, rate: f64) {
        self.rate = rate.clamp(0.5, 2.0);
    }

    /// 伸缩 `input`（交织 PCM），把能产出的部分追加到 `out`。
    fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        let ch = self.channels;
        if self.n == 0 {
            out.extend_from_slice(input);
            return;
        }
        self.buf.extend_from_slice(input);
        self.fed += (input.len() / ch) as u64;
        let l = self.n - self.h;
        loop {
            let base = self.pos.floor() as isize;
            let need = base + self.delta as isize + (self.n + l) as isize + 1;
            if need < 0 || (need as usize) * ch > self.buf.len() {
                break;
            }
            let d = self.best_offset(base);
            let s = (base + d).max(0) as usize;
            // 输出窗：前 h 帧与上一窗尾部交叠相加后即为最终输出；后半存为新尾部
            for i in 0..self.n {
                let w = self.win[i];
                for c in 0..ch {
                    let v = self.buf[(s + i) * ch + c] * w;
                    if i < self.h {
                        out.push(v + self.tail[i * ch + c]);
                    } else {
                        self.tail[(i - self.h) * ch + c] = v;
                    }
                }
            }
            self.emitted += self.h as u64;
            // 记录自然延续：锚定在“理想位置”（而非被选中的 s）+ 名义输入跳距处。
            // 若锚在被选位置，每帧的搜索偏移 d 会逐帧正反馈累积，输入指针原地打转，
            // 产生周期性回声（电音）。
            let off = ((self.h as f64) * self.rate).round() as usize;
            let ns = (base.max(0) as usize + off) * ch;
            self.nat.clear();
            self.nat.extend_from_slice(&self.buf[ns..ns + l * ch]);
            self.pos += self.h as f64 * self.rate;
            let drop = base.max(0) as usize;
            if drop > 0 {
                self.buf.drain(..drop * ch);
                self.pos -= drop as f64;
            }
        }
    }

    /// 流结束：出完残留的整窗与尾部，并按 `fed/rate` 补齐少量静音，
    /// 保证总输出帧数与输入/倍速严格一致（否则慢速下结尾会差一点追不上）。
    fn flush_end(&mut self, out: &mut Vec<f32>) {
        let ch = self.channels;
        if self.n == 0 {
            return;
        }
        // 还能出的整窗：不再搜索（d=0）
        loop {
            let base = self.pos.floor() as isize;
            if base < 0 || ((base + self.n as isize + 1).max(0) as usize) * ch > self.buf.len() {
                break;
            }
            let s = base.max(0) as usize;
            for i in 0..self.n {
                let w = self.win[i];
                for c in 0..ch {
                    let v = self.buf[(s + i) * ch + c] * w;
                    if i < self.h {
                        out.push(v + self.tail[i * ch + c]);
                    } else {
                        self.tail[(i - self.h) * ch + c] = v;
                    }
                }
            }
            self.emitted += self.h as u64;
            self.pos += self.h as f64 * self.rate;
            let drop = base.max(0) as usize;
            self.buf.drain(..drop * ch);
            self.pos -= drop as f64;
        }
        // 不足一窗的尾巴：补零出一窗，把 tail 交叠出去（已排空且无尾部时直接结束）
        let base = self.pos.floor().max(0.0) as usize;
        let take = (self.buf.len() / ch).saturating_sub(base).min(self.n);
        if take == 0 && self.tail.iter().all(|&v| v == 0.0) {
            self.reset();
            return;
        }
        for i in 0..self.n {
            let w = self.win[i];
            for c in 0..ch {
                let v = if i < take {
                    self.buf[(base + i) * ch + c] * w
                } else {
                    0.0
                };
                if i < self.h {
                    out.push(v + self.tail[i * ch + c]);
                } else {
                    out.push(v);
                }
            }
        }
        self.emitted += self.n as u64;
        // 按理论总输出补齐静音（慢速时结尾差量通常 < 一个窗长）
        let target = (self.fed as f64 / self.rate).ceil() as u64;
        if self.emitted < target {
            out.extend(std::iter::repeat_n(
                0.0,
                ((target - self.emitted) as usize) * ch,
            ));
            self.emitted = target;
        }
        self.reset();
    }

    fn reset(&mut self) {
        self.pos = 0.0;
        self.buf.clear();
        self.nat.clear();
        self.tail.fill(0.0);
        self.fed = 0;
        self.emitted = 0;
    }

    /// 在理想位置附近找与自然延续最相似的偏移（归一化互相关，抗响度偏置）；
    /// 倍速 1.0 时必为 0，直接跳过。
    fn best_offset(&self, base: isize) -> isize {
        if self.nat.is_empty() || (self.rate - 1.0).abs() < f64::EPSILON {
            return 0;
        }
        let ch = self.channels;
        let len = (self.n - self.h) * ch;
        let dmin = (-base).max(-(self.delta as isize));
        let mut best = 0isize;
        let mut best_score = f32::NEG_INFINITY;
        for d in dmin..=(self.delta as isize) {
            let s = ((base + d).max(0) as usize) * ch;
            let (mut dot, mut energy) = (0f32, 1e-9f32);
            let mut i = 0;
            while i < len {
                let v = self.buf[s + i];
                dot += v * self.nat[i];
                energy += v * v;
                i += ch;
            }
            let score = dot / energy.sqrt();
            if score > best_score {
                best_score = score;
                best = d;
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 端到端验收（模拟 pump 的结束判定）：核心按 CHUNK_FRAMES 逐块 write（stretcher+resampler
    /// 进 queue），声卡回调逐次消费；write 完毕后调用 end_stream 冲尾，再排空 queue。
    /// 模拟 `played_frames() = consumed * src/dev * rate`，断言最终能追上 `written`
    ///（否则核心的 pump 永远停在 `played < written`，状态卡在 Playing 且无声）。
    #[test]
    fn played_frames_reaches_written_at_end_of_stream() {
        let (src_rate, dev_rate, ch) = (24_000u32, 48_000u32, 2usize);
        for rate in [0.75f64, 1.0, 1.25, 1.5] {
            let mut resampler = Resampler::new(src_rate, dev_rate, 1);
            let mut stretcher = Stretcher::new(1, src_rate, rate);
            let mut queue: VecDeque<f32> = VecDeque::new();
            let total_src: usize = src_rate as usize; // 1 秒（收敛性与长度无关，短些跑得快）
            let mut written = 0u64;
            let mut consumed = 0u64;
            let mut i = 0;
            while written < total_src as u64 {
                let n = 2048.min(total_src - i);
                let input = vec![0.5f32; n];
                let mut stretched = Vec::new();
                stretcher.process(&input, &mut stretched);
                resampler.process(&stretched, dev_rate as u16, &mut queue);
                written += n as u64;
                i += n;
                // 声卡回调消费
                while queue.len() > dev_rate as usize * ch / 10 {
                    let take = queue.len().min(512);
                    queue.drain(..take);
                    consumed += (take / ch) as u64;
                }
            }
            let mut tail = Vec::new();
            stretcher.flush_end(&mut tail);
            resampler.process(&tail, dev_rate as u16, &mut queue);
            while !queue.is_empty() {
                let take = queue.len().min(512);
                queue.drain(..take);
                consumed += (take / ch) as u64;
            }
            let played = (consumed as f64 * src_rate as f64 / dev_rate as f64 * rate) as u64;
            assert!(
                played >= written,
                "rate {rate}: played {played} < written {written}，结尾会卡在播放状态"
            );
        }
    }

    #[test]
    fn resampler_preserves_duration() {
        let mut r = Resampler::new(24_000, 48_000, 1);
        let mut out = VecDeque::new();
        for _ in 0..10 {
            r.process(&vec![0.5; 2400], 2, &mut out);
        }
        let frames = out.len() / 2;
        assert!((frames as i64 - 48_000).abs() <= 2, "{frames}");
    }

    #[test]
    fn stretcher_changes_duration() {
        // 48k 帧（2 秒）输入：1.0 原长，2.0 减半，0.5 翻倍（容差 60ms）
        for (rate, expect) in [(1.0, 48_000i64), (2.0, 24_000), (0.5, 96_000)] {
            let mut s = Stretcher::new(1, 24_000, rate);
            let mut out = Vec::new();
            for _ in 0..10 {
                s.process(&vec![0.5; 4_800], &mut out);
            }
            s.flush_end(&mut out);
            assert!(
                (out.len() as i64 - expect).abs() <= 1_500,
                "rate {rate}: {} vs {expect}",
                out.len()
            );
        }
    }

    #[test]
    fn stretcher_preserves_level() {
        // 恒定信号经交叠相加后幅度应保持（COLA），无 perforated/削波
        let mut s = Stretcher::new(1, 24_000, 1.5);
        let mut out = Vec::new();
        for _ in 0..10 {
            s.process(&vec![0.5; 4_800], &mut out);
        }
        s.flush_end(&mut out);
        // 开头 h 帧是第一个窗的淡入（无前窗交叠），结尾是收尾窗，均不在检查范围
        let body = &out[s.h..out.len() * 3 / 4];
        let (lo, hi) = body
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(lo > 0.42 && hi < 0.58, "level [{lo}, {hi}]");
    }

    #[test]
    fn stretcher_flush_is_idempotent() {
        let mut s = Stretcher::new(1, 24_000, 1.5);
        let mut out = Vec::new();
        s.process(&vec![0.5; 4_800], &mut out);
        s.flush_end(&mut out);
        let len = out.len();
        s.flush_end(&mut out);
        assert_eq!(out.len(), len);
    }
}
