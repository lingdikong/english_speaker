import Foundation
import Network
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
    /// 分类（同歌单）：一篇文章可以同时在多个分类里；各自的 articleIds 按加入先后。
    var collections: [ArticleCollection] = []
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

    // —— 翻译与查词 ——
    var translator: TranslatorStatus?
    /// 当前文章的译文：句下标 → 译文（智能精翻优先）。
    var translations: [Int: SentenceTranslation] = [:]
    var glossary: [GlossaryTerm] = []
    var vocab: [VocabEntry] = []
    /// 点词“举一反三” / 句子解析，键为 "句:词" / "句"（当前文章）。
    var insights: [String: AiResult<WordInsight>] = [:]
    var analyses: [Int: AiResult<SentenceAnalysis>] = [:]
    /// 当前网络是否为蜂窝（下载大模型前提示）。
    var onCellular = false
    @ObservationIgnored private let pathMonitor = NWPathMonitor()
    /// 双语：每句下方显示中文译文；打开时让核心优先翻译当前文章。
    var bilingual = UserDefaults.standard.bool(forKey: "pref.bilingual") {
        didSet {
            UserDefaults.standard.set(bilingual, forKey: "pref.bilingual")
            if bilingual { prepareTranslations() }
        }
    }

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
        translator = core?.translatorStatus()
        vocab = (try? core?.listVocab()) ?? []
        pathMonitor.pathUpdateHandler = { [weak self] path in
            let cellular = path.isExpensive
            DispatchQueue.main.async { MainActor.assumeIsolated { self?.onCellular = cellular } }
        }
        pathMonitor.start(queue: .global(qos: .utility))
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
            syncAudioSession(for: state.status)
            if ended {
                if let l = loopSentence { seekSentence(l); core?.play() } else { advanceQueue() }
                // 播完且没有自动接下一篇：延迟一点确认仍是 ended 再释放硬件
                // （立刻释放会打断 advanceQueue 的连播）
                Task { [weak self] in
                    try? await Task.sleep(for: .milliseconds(600))
                    guard let self, self.player.status == .ended else { return }
                    let sink = self.sink
                    DispatchQueue.global(qos: .utility).async { sink.suspend() }
                    self.nowPlaying?.update()
                }
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
        case let .translationProgress(articleId, translation, _, _):
            if articleId == player.articleId { translations[Int(translation.index)] = translation }
        case let .translationsChanged(articleId):
            if articleId == player.articleId { analyses.removeAll(); reloadTranslations() }
        case let .wordInsightReady(articleId, s, w, insight, error):
            if articleId == player.articleId { insights["\(s):\(w)"] = AiResult(value: insight, error: error) }
        case let .sentenceAnalysisReady(articleId, s, analysis, error):
            if articleId == player.articleId { analyses[Int(s)] = AiResult(value: analysis, error: error) }
        case let .translatorChanged(status):
            translator = status
        case .vocabularyChanged:
            vocab = (try? core?.listVocab()) ?? vocab
        }
    }

    func reloadLibrary() {
        guard let core else { return }
        articles = (try? core.listArticles()) ?? articles
        collections = (try? core.listCollections()) ?? collections
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
        insights.removeAll()
        analyses.removeAll()
        reloadTranslations()
    }

    // MARK: 翻译与查词

    /// 有整句翻译能力（Fast 或 Smart）。
    var canTranslate: Bool { translator?.active != nil }
    /// 智能模式实际可用（已打开且模型已下载）。
    var smartReady: Bool { translator?.active == .smart }

    func reloadTranslations() {
        guard bilingual, let core, let id = player.articleId else { return }
        translations = Dictionary(((try? core.translations(id: id)) ?? []).map { (Int($0.index), $0) },
                                  uniquingKeysWith: { _, b in b })
    }

    /// 打开双语 / 智能翻译时：让核心优先翻译正在播放的文章。
    func prepareTranslations() {
        guard let core, let id = player.articleId else { return }
        try? core.prepareTranslations(id: id)
        reloadTranslations()
    }

    func lookup(_ word: String) -> DictEntry? {
        (try? core?.lookupWord(word: word)) ?? nil
    }

    func requestInsight(sentence: Int, word: Int) {
        guard let core, let id = player.articleId else { return }
        let key = "\(sentence):\(word)"
        if insights[key]?.value != nil || insights[key]?.loading == true { return }
        do {
            if let v = try core.wordInsight(id: id, sentence: UInt32(sentence), word: UInt32(word)) {
                insights[key] = AiResult(value: v)
            } else {
                insights[key] = AiResult(loading: true)
            }
        } catch { insights[key] = AiResult(error: "\(error)") }
    }

    func requestAnalysis(sentence: Int) {
        guard let core, let id = player.articleId else { return }
        if analyses[sentence]?.value != nil || analyses[sentence]?.loading == true { return }
        do {
            if let v = try core.sentenceAnalysis(id: id, sentence: UInt32(sentence)) {
                analyses[sentence] = AiResult(value: v)
            } else {
                analyses[sentence] = AiResult(loading: true)
            }
        } catch { analyses[sentence] = AiResult(error: "\(error)") }
    }

    func setSmart(_ on: Bool) {
        try? core?.setSmartTranslation(enabled: on)
        translator = core?.translatorStatus()
        if on { prepareTranslations() }
    }

    func downloadLlm() {
        try? core?.downloadLlm()
        translator = core?.translatorStatus()
    }

    func cancelLlmDownload() { core?.cancelLlmDownload() }

    func deleteLlm() {
        try? core?.setSmartTranslation(enabled: false)
        try? core?.deleteLlm()
        translator = core?.translatorStatus()
    }

    func reloadGlossary() { glossary = (try? core?.listGlossary()) ?? glossary }

    /// 失败（如为空）时返回 false。
    @discardableResult
    func addGlossary(term: String, translation: String) -> Bool {
        guard (try? core?.addGlossaryTerm(term: term, translation: translation)) != nil else { return false }
        reloadGlossary()
        return true
    }

    func removeGlossary(_ term: String) {
        try? core?.removeGlossaryTerm(term: term)
        reloadGlossary()
    }

    func isInVocab(_ word: String) -> Bool { vocab.contains { $0.word == word.lowercased() } }

    func addVocab(sentence: Int, word: Int) {
        guard let id = player.articleId else { return }
        _ = try? core?.addVocab(id: id, sentence: UInt32(sentence), word: UInt32(word))
    }

    func removeVocab(_ word: String) { try? core?.removeVocab(word: word) }

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
            if !isPlaying {
                AudioSessionController.shared.activate()
                player.status = .buffering
                core.play()
            }
            return
        }
        if let context, let i = context.firstIndex(where: { $0.id == id }) {
            upNext = context[(i + 1)...].map(\.id)
        } else {
            upNext.removeAll { $0 == id }
        }
        AudioSessionController.shared.activate()
        player.status = .buffering   // 图标立刻切成暂停态，不等核心带回真实状态
        do { try core.playArticle(id: id) } catch { NSLog("[store] play failed: \(error)") }
    }

    func togglePlay() {
        guard let core else { return }
        if player.articleId == nil, let first = articles.first {
            play(first.id, context: articles)
        } else if isPlaying {
            pause()
        } else {
            AudioSessionController.shared.activate()
            // 同上：先切成缓冲中，图标立刻响应，不等核心的事件往返
            if player.articleId != nil { player.status = .buffering }
            core.play()
        }
    }

    func pause() {
        guard let core else { return }
        // 乐观更新：核心是异步的（命令投递 → player 线程处理 → 事件回调主线程），
        // 等状态事件回来图标才切会慢一拍，这里点了就立刻变。
        if isPlaying { player.status = .paused }
        core.pause()
    }

    /// 不播放时把音频硬件还回去。
    ///
    /// 会话一旦 setActive(true) 就再没停过，系统会一直把本 App 当作“正在播放音频的
    /// App”：灵动岛的播放指示器常驻、退到后台后 App 不挂起、锁屏媒体控件不下线。
    /// 暂停时停引擎 + 停用会话即可消除；恢复播放时（resume / play）会重新激活。
    private func syncAudioSession(for status: PlayerStatus) {
        if status == .playing || status == .buffering {
            AudioSessionController.shared.activate()
        } else if status == .paused {
            // 停用会话要等硬件释放、engine.stop 也要拆掉整条渲染链，加起来可达上百毫秒。
            // 必须在后台线程做：在主线程同步执行会卡住 UI，表现为点完暂停图标“慢一下”才变。
            let sink = self.sink
            DispatchQueue.global(qos: .userInitiated).async { sink.suspend() }
        }
    }

    /// 播放底部栏 / 继续收听卡片上的文章。
    func playOrToggle(_ id: String) {
        if id == player.articleId { togglePlay() } else { play(id, context: articles) }
    }

    /// 播放一段（点词发音）：跳到 start，播到 end 后暂停。
    func playSpan(startMs: UInt64, endMs: UInt64) {
        seek(to: startMs)
        stopAtMs = endMs
        AudioSessionController.shared.activate()
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
    /// 恢复播放。锁屏/控制中心点播放时会话可能已被停用（见 syncAudioSession），先激活。
    func resume() {
        AudioSessionController.shared.activate()
        if player.articleId != nil { player.status = .buffering }
        core?.play()
    }

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

    // MARK: 分类

    func collection(_ id: String) -> ArticleCollection? { collections.first { $0.id == id } }

    /// 分类里的文章，按加入顺序。
    func articles(in collectionId: String) -> [ArticleSummary] {
        (collection(collectionId)?.articleIds ?? []).compactMap(article)
    }

    /// 文章所在的全部分类。
    func collections(of articleId: String) -> [ArticleCollection] {
        collections.filter { $0.articleIds.contains(articleId) }
    }

    /// 名称校验（与核心一致：去空白后非空、不区分大小写不重名）；合法时返回 nil。
    func collectionNameError(_ name: String, except: String? = nil) -> String? {
        let n = name.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if n.isEmpty { return "请输入分类名称" }
        if collections.contains(where: { $0.id != except && $0.name.lowercased() == n }) { return "已有同名分类" }
        return nil
    }

    /// 新建分类；给了 articleId 时建好后把这篇加进去。
    @discardableResult
    func createCollection(_ name: String, adding articleId: String? = nil) -> ArticleCollection? {
        guard let core, let c = try? core.createCollection(name: name) else { return nil }
        if let articleId { try? core.addToCollection(articleId: articleId, collectionId: c.id) }
        reloadLibrary()
        return c
    }

    func renameCollection(_ id: String, to name: String) {
        try? core?.renameCollection(id: id, name: name)
        reloadLibrary()
    }

    func deleteCollection(_ id: String) {
        try? core?.deleteCollection(id: id)
        reloadLibrary()
    }

    /// 已在其中则移出，否则加入（菜单里打 ✓ 的项再点一次即移出）。
    func toggleCollection(_ collectionId: String, article articleId: String) {
        guard let c = collection(collectionId) else { return }
        if c.articleIds.contains(articleId) {
            try? core?.removeFromCollection(articleId: articleId, collectionId: collectionId)
        } else {
            try? core?.addToCollection(articleId: articleId, collectionId: collectionId)
        }
        reloadLibrary()
    }

    func removeFromCollection(_ collectionId: String, article articleId: String) {
        try? core?.removeFromCollection(articleId: articleId, collectionId: collectionId)
        reloadLibrary()
    }

    /// 播放分类：从第一篇开始，其余依次排进待播清单。
    func playCollection(_ id: String) {
        let items = articles(in: id)
        guard let first = items.first else { return }
        play(first.id, context: items)
    }

    func addToQueue(_ id: String, next: Bool) {
        upNext.removeAll { $0 == id }
        if next { upNext.insert(id, at: 0) } else { upNext.append(id) }
    }
}

/// 智能解析的请求状态。
struct AiResult<T> {
    var loading = false
    var value: T?
    var error: String?
}

extension Array {
    subscript(safe i: Int) -> Element? { indices.contains(i) ? self[i] : nil }
}
