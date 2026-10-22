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
            resampler,
            queue,
            dev_channels,
            ..
        } = &mut *b;
        resampler.process(samples, *dev_channels, queue);
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
        }
        self.shared.cv.notify_all();
    }

    fn played_frames(&self) -> u64 {
        let b = self.shared.buf.lock().unwrap();
        match b.src {
            Some(src) if b.dev_rate > 0 => b.consumed * src.sample_rate as u64 / b.dev_rate as u64,
            _ => 0,
        }
    }

    fn set_rate(&self, _rate: f32) -> bool {
        // TODO: 桌面端变速需在 Rust 侧做保持音调的时间伸缩（WSOLA）。
        false
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
