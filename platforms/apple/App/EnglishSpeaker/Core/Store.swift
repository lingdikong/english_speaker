import Foundation
import Observation
import SpeakerCore
import SwiftUI

/// 事件回调：核心在后台线程调用，这里切回主线程交给 Store。
final class CoreListener: SpeakerListener, @unchecked Sendable {
    weak var store: Store?
    func onEvent(event: SpeakerEvent) {
        DispatchQueue.main.async { [weak self] in
            MainActor.assumeIsolated { self?.store?.handle(event) }
        }
    }
}

/// App 全局状态：资料库、播放状态、当前位置/句/词、缓存统计、待播清单。
/// 业务状态全部来自核心层（UI_SPEC §4），这里只做缓存与派生。
@MainActor
@Observable
final class Store {
    private(set) var core: SpeakerCore?
    @ObservationIgnored let sink = EngineAudioSink()
    @ObservationIgnored private let listener = CoreListener()
    @ObservationIgnored private var nowPlaying: NowPlayingCenter?

    // —— 资料库 ——
    var articles: [ArticleSummary] = []
    var bodies: [String: String] = [:]
    var voices: [Voice] = []
    var cache: CacheStats?
    var usingRealModel = false
    var startupError: String?

    // —— 播放 ——
    var player = PlayerState(articleId: nil, status: .idle, positionMs: 0, bufferedMs: 0,
                             durationMs: 0, durationFinal: false, rate: 1)
    var positionMs: UInt64 = 0
    var bufferedMs: UInt64 = 0
    var sentence: Int?
    var word: Int?
    /// 最近一次位置事件的时间，用于在两次事件之间插值（逐字渐变、进度条平滑）。
    @ObservationIgnored var positionStamp = Date()
    var timeline: Timeline?
    /// 待播清单（客户端维护）。
    var upNext: [String] = []

    /// 单句循环：当前循环的句子（核心没有该接口，客户端在离开该句时跳回）。
    var loopSentence: Int?
    /// 最近播放的文章（客户端记录，重启后底部播放条仍显示）。
    var lastPlayedId: String? = UserDefaults.standard.string(forKey: "lastPlayed") {
        didSet { UserDefaults.standard.set(lastPlayedId, forKey: "lastPlayed") }
    }
    /// 点词发音：播到该时间点后暂停。
    @ObservationIgnored private var stopAtMs: UInt64?
    /// 非当前文章的时间轴缓存（“继续收听”的第 x/y 句）。
    @ObservationIgnored private var timelineCache: [String: Timeline] = [:]

    // —— 界面 ——
    var isPlayerExpanded = false

    static let defaultVoice = "af_heart"

    // MARK: 启动

    func start(resetData: Bool = false) {
        listener.store = self
        let fm = FileManager.default
        let support = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("EnglishSpeaker", isDirectory: true)
        if resetData {
            try? fm.removeItem(at: support)
            UserDefaults.standard.removeObject(forKey: "seeded")
        }
        try? fm.createDirectory(at: support, withIntermediateDirectories: true)
        // 模型随 Bundle 分发（folder reference：Bundle/models/kokoro）；没有时核心退回 Mock 引擎
        let modelDir = Bundle.main.url(forResource: "models", withExtension: nil)
        usingRealModel = modelDir.map { fm.fileExists(atPath: $0.appendingPathComponent("kokoro/model.onnx").path) } ?? false
        do {
            core = try SpeakerCore(dataDir: support.path, modelDir: modelDir?.path,
                                   defaultVoice: Self.defaultVoice, sink: sink, listener: listener)
        } catch {
            startupError = "\(error)"
            return
        }
        voices = core?.voices() ?? []
        player = core?.playerState() ?? player
        reloadLibrary()
        if !UserDefaults.standard.bool(forKey: "seeded") {
            UserDefaults.standard.set(true, forKey: "seeded")
            if articles.isEmpty { seedSamples() }
        }
        nowPlaying = NowPlayingCenter(store: self)
        AudioSessionController.shared.onInterruption = { [weak self] began in
            if began { self?.pause() }
        }
    }

    private func seedSamples() {
        for sample in SampleArticles.all {
            _ = try? core?.importText(title: sample.title, body: sample.body, source: .paste, sourceRef: nil)
        }
        reloadLibrary()
    }

    // MARK: 事件

    func handle(_ event: SpeakerEvent) {
        switch event {
        case .libraryChanged:
            reloadLibrary()
        case let .synthesisProgress(articleId, ready, total, span):
            if let i = articles.firstIndex(where: { $0.id == articleId }) {
                articles[i].readySentences = ready
                articles[i].totalSentences = total
                if articles[i].status != .ready { articles[i].status = .synthesizing }
            }
            if articleId == player.articleId, var t = timeline, Int(span.index) < t.sentences.count {
                t.sentences[Int(span.index)] = span
                timeline = t
            }
        case let .synthesisFinished(articleId, _):
            reloadLibrary()
            if articleId == player.articleId { reloadTimeline() }
        case .synthesisFailed:
            reloadLibrary()
        case let .cacheChanged(stats):
            cache = stats
        case let .playerStateChanged(state):
            let changed = state.articleId != player.articleId
            let ended = state.status == .ended && player.status != .ended
            if state.status != player.status { NSLog("[player] status \(player.status) -> \(state.status)") }
            player = state
            positionMs = state.positionMs
            bufferedMs = state.bufferedMs
            positionStamp = Date()
            if changed {
                sentence = nil; word = nil; loopSentence = nil
                if let id = state.articleId { lastPlayedId = id }
                reloadTimeline()
            } else if state.durationFinal, timeline?.complete == false {
                reloadTimeline()
            }
            nowPlaying?.update()
            if ended {
                if let l = loopSentence { seekSentence(l); core?.play() } else { advanceQueue() }
            }
        case let .playerPosition(pos, buffered, s, w):
            positionMs = pos
            bufferedMs = buffered
            positionStamp = Date()
            let newSentence = s.map(Int.init)
            if let l = loopSentence, let prev = sentence, prev == l, let ns = newSentence, ns != l {
                seekSentence(l)
                return
            }
            sentence = newSentence
            word = w.map(Int.init)
            if let stop = stopAtMs, pos >= stop {
                stopAtMs = nil
                core?.pause()
            }
            nowPlaying?.updateElapsedIfNeeded()
        }
    }

    func reloadLibrary() {
        guard let core else { return }
        articles = (try? core.listArticles()) ?? articles
        cache = try? core.cacheStats()
        for a in articles where bodies[a.id] == nil {
            bodies[a.id] = try? core.getArticle(id: a.id).body
        }
        upNext.removeAll { id in !articles.contains { $0.id == id } }
        if let l = lastPlayedId, !articles.contains(where: { $0.id == l }) { lastPlayedId = nil }
        timelineCache.removeAll()
    }

    func reloadTimeline() {
        guard let core, let id = player.articleId else { timeline = nil; return }
        timeline = try? core.timeline(id: id)
    }

    // MARK: 派生

    var current: ArticleSummary? {
        guard let id = player.articleId else { return nil }
        return articles.first { $0.id == id }
    }

    var isPlaying: Bool { player.status == .playing || player.status == .buffering }

    func article(_ id: String) -> ArticleSummary? { articles.first { $0.id == id } }

    func voice(of a: ArticleSummary) -> Voice? {
        let vid = a.voiceId ?? Self.defaultVoice
        return voices.first { $0.id == vid }
    }

    /// “Heart · 美音”
    func voiceLabel(_ a: ArticleSummary, separator: String = " · ") -> String {
        voice(of: a).map { Fmt.voice($0, separator: separator) } ?? (a.voiceId ?? Self.defaultVoice)
    }

    /// 资料库行副标题：“6 分钟 · Heart 美音”；未转换时“约 5 分钟”。
    func rowMeta(_ a: ArticleSummary) -> String {
        if a.status == .notCached { return Fmt.minutes(a.durationMs, estimated: true) }
        return "\(Fmt.minutes(a.durationMs, estimated: a.status != .ready)) · \(voiceLabel(a, separator: " "))"
    }

    /// 底部播放条 / 继续收听显示的文章：正在播放的，否则最近播放的。
    var barArticle: ArticleSummary? { current ?? lastPlayedId.flatMap(article) }

    /// 第 x / y 句（x 从 1 开始）。非当前文章按已保存进度在时间轴中查找。
    func sentenceProgress(_ a: ArticleSummary) -> (index: Int, total: Int) {
        let total = Int(a.totalSentences)
        if a.id == player.articleId { return ((sentence ?? 0) + 1, max(total, timeline?.sentences.count ?? 0)) }
        guard let pos = a.progressMs, pos > 0, let core else { return (1, total) }
        if timelineCache[a.id] == nil { timelineCache[a.id] = try? core.timeline(id: a.id) }
        let idx = timelineCache[a.id]?.sentences.lastIndex { $0.ready && $0.startMs <= pos } ?? 0
        return (idx + 1, total)
    }

    /// 播放进度（0…1）。
    func progressFraction(_ a: ArticleSummary) -> Double {
        let pos = a.id == player.articleId ? Double(positionMs) : Double(a.progressMs ?? 0)
        let dur = a.id == player.articleId ? Double(durationMs) : Double(a.durationMs)
        return dur > 0 ? min(1, pos / dur) : 0
    }

    func remainingMs(_ a: ArticleSummary) -> UInt64 {
        let pos = a.id == player.articleId ? positionMs : (a.progressMs ?? 0)
        let dur = a.id == player.articleId ? durationMs : a.durationMs
        return dur > pos ? dur - pos : 0
    }

    var currentSentenceText: String? {
        guard let t = timeline else { return nil }
        return t.sentences[safe: sentence ?? 0]?.text
    }

    /// 两次位置事件之间按播放速度插值的当前位置（毫秒）。
    func livePosition(at date: Date) -> Double {
        let base = Double(positionMs)
        guard player.status == .playing else { return base }
        let elapsed = date.timeIntervalSince(positionStamp) * 1000 * Double(player.rate)
        return min(base + min(elapsed, 400), Double(max(bufferedMs, positionMs)))
    }

    var durationMs: UInt64 { max(player.durationMs, current?.durationMs ?? 0) }

    // MARK: 播放控制

    /// 从列表播放：待播清单设为列表中其后的文章（Apple Music 从专辑播放的行为）。
    func play(_ id: String, context: [ArticleSummary]? = nil) {
        guard let core else { return }
        if id == player.articleId {
            if !isPlaying { core.play() }
            return
        }
        if let context, let i = context.firstIndex(where: { $0.id == id }) {
            upNext = context[(i + 1)...].map(\.id)
        } else {
            upNext.removeAll { $0 == id }
        }
        do { try core.playArticle(id: id) } catch { NSLog("[store] play failed: \(error)") }
    }

    func togglePlay() {
        guard let core else { return }
        if player.articleId == nil, let first = articles.first {
            play(first.id, context: articles)
        } else if isPlaying {
            core.pause()
        } else {
            core.play()
        }
    }

    func pause() { core?.pause() }

    /// 播放底部栏 / 继续收听卡片上的文章。
    func playOrToggle(_ id: String) {
        if id == player.articleId { togglePlay() } else { play(id, context: articles) }
    }

    /// 播放一段（点词发音）：跳到 start，播到 end 后暂停。
    func playSpan(startMs: UInt64, endMs: UInt64) {
        seek(to: startMs)
        stopAtMs = endMs
        core?.play()
    }

    func toggleLoop() {
        loopSentence = loopSentence == nil ? (sentence ?? 0) : nil
    }

    func setRate(_ r: Float) { core?.setRate(rate: r) }

    func setCacheLimit(_ bytes: UInt64) {
        try? core?.setCacheLimit(bytes: bytes)
        cache = try? core?.cacheStats()
    }

    @discardableResult
    func clearCache() -> UInt32 {
        let n = (try? core?.clearCache()) ?? 0
        reloadLibrary()
        return n
    }
    func resume() { core?.play() }

    func seek(to ms: UInt64) {
        core?.seek(positionMs: ms)
        positionMs = ms
        positionStamp = Date()
    }

    func seekSentence(_ index: Int) {
        guard let core else { return }
        if loopSentence != nil { loopSentence = index }
        stopAtMs = nil
        if (try? core.seekSentence(index: UInt32(index))) != nil {
            sentence = index
            if let s = timeline?.sentences[safe: index] {
                positionMs = s.startMs
                positionStamp = Date()
            }
        }
    }

    /// 下一句（锁屏“下一首”同此）。
    func nextSentence() {
        guard let t = timeline else { return }
        let next = (sentence ?? -1) + 1
        if next < t.sentences.count, t.sentences[next].ready {
            seekSentence(next)
        } else if next >= t.sentences.count, t.complete {
            advanceQueue()
        }
    }

    /// 上一句：本句已读超过 1.5 秒时回到句首，否则跳到上一句（同 Apple Music 的“上一首”）。
    func previousSentence() {
        guard let t = timeline, let s = sentence ?? (t.sentences.isEmpty ? nil : 0) else { return }
        let start = t.sentences[s].startMs
        if positionMs > start + 1500 || s == 0 {
            seekSentence(s)
        } else {
            seekSentence(s - 1)
        }
    }

    static let rates: [Float] = [0.75, 1.0, 1.25, 1.5]

    func cycleRate() {
        let r = player.rate
        let i = Self.rates.firstIndex { abs($0 - r) < 0.01 } ?? 1
        core?.setRate(rate: Self.rates[(i + 1) % Self.rates.count])
        player.rate = Self.rates[(i + 1) % Self.rates.count]
    }

    private func advanceQueue() {
        guard !upNext.isEmpty else { return }
        let id = upNext.removeFirst()
        try? core?.playArticle(id: id)
    }

    // MARK: 资料库操作

    @discardableResult
    func importText(title: String?, body: String, source: SourceKind = .paste, sourceRef: String? = nil,
                    voice: String? = nil) -> ArticleSummary? {
        guard let core else { return nil }
        let summary = try? core.importText(title: title, body: body, source: source, sourceRef: sourceRef)
        // 导入即以默认音色入队；选了其它音色时换音色重新排队
        if let summary, let voice, voice != Self.defaultVoice {
            try? core.synthesize(id: summary.id, voiceId: voice)
        }
        reloadLibrary()
        return summary
    }

    func setPinned(_ id: String, _ pinned: Bool) {
        try? core?.setPinned(id: id, pinned: pinned)
        reloadLibrary()
    }

    func synthesize(_ id: String, voice: String? = nil) {
        try? core?.synthesize(id: id, voiceId: voice)
        reloadLibrary()
    }

    func removeDownload(_ id: String) {
        try? core?.removeDownload(id: id)
        reloadLibrary()
    }

    func delete(_ id: String) {
        if id == player.articleId { core?.stop() }
        try? core?.deleteArticle(id: id)
        upNext.removeAll { $0 == id }
        reloadLibrary()
    }

    func addToQueue(_ id: String, next: Bool) {
        upNext.removeAll { $0 == id }
        if next { upNext.insert(id, at: 0) } else { upNext.append(id) }
    }
}

extension Array {
    subscript(safe i: Int) -> Element? { indices.contains(i) ? self[i] : nil }
}
