import AVFoundation
import SpeakerCore

/// 核心层的音频输出：AVAudioEngine + AVAudioPlayerNode（scheduleBuffer）+ AVAudioUnitTimePitch。
///
/// 语义见 docs/ARCHITECTURE.md §2：
/// - `write` 在已排队超过约 200ms 时阻塞（背压）；
/// - `playedFrames` 返回自 open/flush 以来实际播放的帧数（进度的唯一来源）；
/// - `setRate` 用 TimePitch 实现变速不变调。
///
/// 所有方法都由核心的 es-player 线程调用（音量除外），内部状态用锁保护。
final class EngineAudioSink: PlatformAudioSink, @unchecked Sendable {
    private let engine = AVAudioEngine()
    private let player = AVAudioPlayerNode()
    private let timePitch = AVAudioUnitTimePitch()
    private let lock = NSLock()

    private var format: AVAudioFormat?
    /// 已排队、尚未确认播完的缓冲：(在播放器时间轴上的起始帧, 帧数)。
    private var pending: [(start: Int64, frames: Int64)] = []
    /// 已确认播完的帧数（自 open/flush 起）。
    private var completed: Int64 = 0
    /// 下一个缓冲在播放器时间轴上的预计起点。
    private var nextStart: Int64 = 0
    /// 播放器最近一次可读的采样时间（暂停/停止时 playerTime 为 nil，用它兜底）。
    private var lastSampleTime: Int64 = 0
    private var paused = false
    private var attached = false

    /// 背压阈值：排队超过约 200ms 时阻塞。
    private let maxQueuedSeconds = 0.2

    init() {}

    // MARK: PlatformAudioSink

    func openOutput(sampleRate: UInt32, channels: UInt16) -> Bool {
        lock.lock(); defer { lock.unlock() }
        guard let fmt = AVAudioFormat(standardFormatWithSampleRate: Double(sampleRate),
                                      channels: AVAudioChannelCount(max(channels, 1))) else { return false }
        AudioSessionController.shared.activate()
        if !attached {
            engine.attach(player)
            engine.attach(timePitch)
            attached = true
        }
        if format != fmt {
            engine.disconnectNodeOutput(player)
            engine.disconnectNodeOutput(timePitch)
            engine.connect(player, to: timePitch, format: fmt)
            engine.connect(timePitch, to: engine.mainMixerNode, format: fmt)
            format = fmt
        }
        do {
            if !engine.isRunning { try engine.start() }
        } catch {
            NSLog("[sink] engine start failed: \(error)")
            return false
        }
        resetLocked()
        player.play()
        paused = false
        return true
    }

    func write(samples: [Float]) {
        guard !samples.isEmpty else { return }
        // 背压：排队超过阈值时等待；引擎被系统打断时最多等 2 秒，避免卡死播放线程
        let deadline = Date().addingTimeInterval(2)
        while queuedSeconds() > maxQueuedSeconds && Date() < deadline {
            Thread.sleep(forTimeInterval: 0.01)
        }

        lock.lock()
        guard let fmt = format,
              let buffer = AVAudioPCMBuffer(pcmFormat: fmt, frameCapacity: AVAudioFrameCount(samples.count))
        else { lock.unlock(); return }
        buffer.frameLength = AVAudioFrameCount(samples.count)
        samples.withUnsafeBufferPointer { src in
            buffer.floatChannelData![0].update(from: src.baseAddress!, count: samples.count)
        }
        let now = currentSampleTimeLocked()
        let start = max(nextStart, now)
        let frames = Int64(samples.count)
        pending.append((start, frames))
        nextStart = start + frames
        if !engine.isRunning { try? engine.start() }
        player.scheduleBuffer(buffer, completionHandler: nil)
        if !paused && !player.isPlaying { player.play() }
        lock.unlock()
    }

    func pause() {
        lock.lock(); defer { lock.unlock() }
        _ = currentSampleTimeLocked()
        paused = true
        player.pause()
    }

    func resume() {
        lock.lock(); defer { lock.unlock() }
        paused = false
        if !engine.isRunning { try? engine.start() }
        player.play()
    }

    func flush() {
        lock.lock(); defer { lock.unlock() }
        // stop 会丢弃已排队的缓冲，并把播放器时间轴归零
        player.stop()
        resetLocked()
        if !paused { player.play() }
    }

    func playedFrames() -> UInt64 {
        lock.lock(); defer { lock.unlock() }
        return UInt64(max(0, playedLocked()))
    }

    func setRate(rate: Float) -> Bool {
        timePitch.rate = rate
        return true
    }

    func closeOutput() {
        lock.lock(); defer { lock.unlock() }
        player.stop()
        resetLocked()
    }

    // MARK: 音量（播放页的音量滑块）

    var volume: Float {
        get { engine.mainMixerNode.outputVolume }
        set { engine.mainMixerNode.outputVolume = newValue }
    }

    // MARK: 内部

    private func resetLocked() {
        pending.removeAll()
        completed = 0
        nextStart = 0
        lastSampleTime = 0
    }

    /// 播放器节点时间轴上的当前采样位置。TimePitch 在其下游，所以这里计的是“源帧”，与倍速无关。
    private func currentSampleTimeLocked() -> Int64 {
        if let nodeTime = player.lastRenderTime, nodeTime.isSampleTimeValid,
           let t = player.playerTime(forNodeTime: nodeTime), t.isSampleTimeValid {
            lastSampleTime = max(lastSampleTime, t.sampleTime)
        }
        return lastSampleTime
    }

    /// 已播放帧数 = 已播完的缓冲 + 正在播放的缓冲中已经过的部分。
    private func playedLocked() -> Int64 {
        let now = currentSampleTimeLocked()
        while let first = pending.first, now >= first.start + first.frames {
            completed += first.frames
            pending.removeFirst()
        }
        var played = completed
        if let first = pending.first, now > first.start {
            played += min(now - first.start, first.frames)
        }
        return played
    }

    private func queuedSeconds() -> Double {
        lock.lock(); defer { lock.unlock() }
        guard let fmt = format else { return 0 }
        let played = playedLocked()
        let scheduled = completed + pending.reduce(0) { $0 + $1.frames }
        return Double(scheduled - played) / fmt.sampleRate
    }
}

/// AVAudioSession 配置：.playback 支持后台播放；处理打断。
final class AudioSessionController: @unchecked Sendable {
    static let shared = AudioSessionController()
    private var configured = false
    /// 打断开始/结束时回调（主线程）。
    var onInterruption: ((_ began: Bool) -> Void)?

    func activate() {
        let session = AVAudioSession.sharedInstance()
        if !configured {
            configured = true
            try? session.setCategory(.playback, mode: .spokenAudio)
            NotificationCenter.default.addObserver(
                forName: AVAudioSession.interruptionNotification, object: session, queue: .main
            ) { [weak self] note in
                guard let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
                      let type = AVAudioSession.InterruptionType(rawValue: raw) else { return }
                self?.onInterruption?(type == .began)
            }
        }
        try? session.setActive(true)
    }
}
