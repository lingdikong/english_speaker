import XCTest

/// 端到端（UI_SPEC §7-3）：真实核心 + 真实点按。
/// 导入一篇文章 → 立即播放（看到 Buffering → Playing）→ 逐词高亮跟随（第 x 句递增）→ 点按某句跳转。
final class EndToEndTests: XCTestCase {
    let body = """
    The first train leaves the station at six in the morning. Most of the passengers are still half asleep.

    A woman by the window opens a book and starts to read. Outside, the city slowly wakes up.

    By the time the train reaches the coast, the sun is high and the sea is bright blue. Everyone looks out at the water.

    A small boy points at a boat far away and asks his father where it is going. His father smiles and says that nobody really knows.

    At the last stop, the doors open and the cold air rushes in. The passengers pick up their bags and step out onto the platform, ready for a long day by the sea.
    """

    func shot(_ name: String) {
        let a = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        a.name = name
        a.lifetime = .keepAlways
        add(a)
    }

    func testImportPlayHighlightSeek() throws {
        let app = XCUIApplication()
        app.launchArguments = ["-resetData", "YES", "-theme", "dark", "-uiTestScreen", "library",
                               "-AppleLanguages", "(en)", "-pref.focusMode", "NO", "-AppleKeyboards", "(en_US@sw=QWERTY;hw=Automatic)"]
        app.launch()

        // 1. 导入
        let importButton = app.buttons["import-button"]
        XCTAssertTrue(importButton.waitForExistence(timeout: 10))
        importButton.tap()
        let title = app.textFields["import-title"]
        XCTAssertTrue(title.waitForExistence(timeout: 5))
        title.tap()
        title.typeText("Morning Train")
        let bodyView = app.textViews["import-body"]
        // 正文框中部可能在键盘下面：点它的顶部
        bodyView.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.02)).withOffset(CGVector(dx: 0, dy: 20)).tap()
        bodyView.typeText(body)
        app.buttons["完成"].firstMatch.tap()
        sleep(1)
        shot("e2e-01-import-filled")
        app.buttons["import-commit"].tap()

        // 2. 立即播放：先 Buffering（播放键显示“缓冲中”），再 Playing（显示“暂停”）
        let buffering = app.buttons["缓冲中"]
        let sawBuffering = buffering.waitForExistence(timeout: 2)
        if sawBuffering { shot("e2e-02-buffering") }
        let pause = app.buttons.matching(identifier: "player-play").matching(NSPredicate(format: "label == '暂停'")).firstMatch
        XCTAssertTrue(pause.waitForExistence(timeout: 30), "should reach Playing")
        print("E2E sawBuffering=\(sawBuffering)")
        shot("e2e-03-playing")

        // 3. 逐词高亮跟随：读“第 x / y 句”，x 随播放递增
        func currentSentence() -> Int? {
            let el = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH '第 '")).firstMatch
            guard el.exists else { return nil }
            let parts = el.label.split(separator: " ")
            return parts.count > 1 ? Int(parts[1]) : nil
        }
        let s0 = currentSentence() ?? 0
        print("E2E sentence at start=\(s0)")
        var advanced = false
        for _ in 0..<20 {
            sleep(1)
            if let s = currentSentence(), s > s0 { advanced = true; print("E2E advanced to \(s)"); break }
        }
        XCTAssertTrue(advanced, "highlight should follow playback to later sentences")
        shot("e2e-04-following")

        // 4. 点按后面一句可见的句子跳转（2 秒内到达，自然播放做不到）
        let before = currentSentence() ?? 0
        // 选一个在当前句之后至少 2 句、位于当前句卡片与控制面板之间（可见可点）的句子
        var pick = -1
        for i in (before + 1)..<10 {
            let f = app.descendants(matching: .any)["sentence-\(i)"].frame
            if f.midY > 300 && f.maxY < 600 { pick = i; break }
        }
        XCTAssertGreaterThan(pick, before, "a later sentence should be visible")
        let target = app.descendants(matching: .any)["sentence-\(pick)"]
        print("E2E before tap sentence=\(before) target frame=\(target.frame)")
        target.tap()
        var seeked = false
        for _ in 0..<8 {
            if currentSentence() == pick + 1 { seeked = true; break }
            usleep(250_000)
        }
        print("E2E after tap sentence=\(currentSentence() ?? -1)")
        XCTAssertTrue(seeked, "tap on a later sentence should seek to it")
        let playing = app.buttons.matching(identifier: "player-play").matching(NSPredicate(format: "label == '暂停'")).firstMatch
        XCTAssertTrue(playing.exists, "still playing after seek")
        shot("e2e-05-seeked")
    }
}
