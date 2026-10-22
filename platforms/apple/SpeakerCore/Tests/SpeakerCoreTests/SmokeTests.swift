import Foundation
import XCTest
@testable import SpeakerCore

/// 不出声的 sink：按实时速度累加已播放帧数，验证 Rust ↔ Swift 回调链路。
final class NullSink: PlatformAudioSink, @unchecked Sendable {
    private let lock = NSLock()
    private var rate: UInt32 = 24_000
    private var played: UInt64 = 0

    func openOutput(sampleRate: UInt32, channels: UInt16) -> Bool {
        lock.withLock { rate = sampleRate; played = 0 }
        return true
    }
    func write(samples: [Float]) {
        Thread.sleep(forTimeInterval: Double(samples.count) / Double(rate) / 20) // 20 倍速
        lock.withLock { played += UInt64(samples.count) * 20 }
    }
    func pause() {}
    func resume() {}
    func flush() { lock.withLock { played = 0 } }
    func playedFrames() -> UInt64 { lock.withLock { played } }
    func setRate(rate: Float) -> Bool { false }
    func closeOutput() {}
}

final class Recorder: SpeakerListener, @unchecked Sendable {
    let finished = XCTestExpectation(description: "synthesis finished")
    let ended = XCTestExpectation(description: "playback ended")
    func onEvent(event: SpeakerEvent) {
        switch event {
        case .synthesisFinished: finished.fulfill()
        case .playerStateChanged(let state) where state.status == .ended: ended.fulfill()
        default: break
        }
    }
}

final class SmokeTests: XCTestCase {
    /// 导入后立即播放（边听边转），播完后完整缓存在本地；缓存池接口可用。
    func testImportPlayWhileConvertingAndCache() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let recorder = Recorder()
        let core = try SpeakerCore(
            dataDir: dir.path, modelDir: nil, defaultVoice: nil,
            sink: NullSink(), listener: recorder)

        let article = try core.importText(
            title: nil, body: "Hello from Swift. It costs $5.", source: .paste, sourceRef: nil)
        XCTAssertEqual(try core.timeline(id: article.id).sentences.count, 2, "原文导入即可见")
        try core.playArticle(id: article.id)
        wait(for: [recorder.finished, recorder.ended], timeout: 10)

        let timeline = try core.timeline(id: article.id)
        XCTAssertTrue(timeline.complete)
        XCTAssertEqual(timeline.sentences.map(\.text), ["Hello from Swift.", "It costs $5."])
        XCTAssertEqual(timeline.sentences[1].words.last?.text, "$5.")

        let summary = try XCTUnwrap(core.listArticles().first)
        XCTAssertEqual(summary.status, .ready)
        XCTAssertGreaterThan(try core.cacheStats().usedBytes, 0)

        try core.setPinned(id: article.id, pinned: true)
        XCTAssertEqual(try core.clearCache(), 0, "钉住的不清")
        XCTAssertThrowsError(try core.getArticle(id: "missing"))
    }

    /// 真实 Kokoro 模型（需先运行 scripts/fetch-models.sh；没有模型则跳过）。
    func testRealKokoroThroughFFI() throws {
        let models = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().appendingPathComponent("../../../../../models").standardized
        try XCTSkipUnless(
            FileManager.default.fileExists(atPath: models.appendingPathComponent("kokoro/model.onnx").path),
            "no Kokoro model")
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let recorder = Recorder()
        let core = try SpeakerCore(
            dataDir: dir.path, modelDir: models.path, defaultVoice: "bf_emma",
            sink: NullSink(), listener: recorder)
        XCTAssertTrue(core.voices().contains { $0.id == "bm_george" && $0.accent == .british })

        let article = try core.importText(
            title: nil, body: "Good morning. This is Kokoro, speaking from Swift.", source: .paste, sourceRef: nil)
        try core.playArticle(id: article.id)
        wait(for: [recorder.finished, recorder.ended], timeout: 60)

        let summary = try XCTUnwrap(core.listArticles().first)
        XCTAssertEqual(summary.status, .ready)
        XCTAssertEqual(summary.voiceId, "bf_emma")
        XCTAssertGreaterThan(summary.durationMs, 1500)
        let words = try core.timeline(id: article.id).sentences[1].words
        XCTAssertEqual(words.map(\.text).last, "Swift.")
        // 跟读标注："This is" 辅音接元音 → This‿is
        let first = try core.timeline(id: article.id).sentences[1].words
        XCTAssertEqual(first[0].link, .consonantVowel, "This‿is")
        XCTAssertFalse(first[0].ipa.isEmpty)
        XCTAssertTrue(zip(words, words.dropFirst()).allSatisfy { $0.endMs <= $1.startMs + 1 })
    }
}
