import XCTest

/// 分类端到端：真实核心 + 真实点按。
/// 新建分类 → 重名被拦下 → 用文章菜单加入分类 → 菜单里新建第二个分类并加入（同一篇在两个分类里）
/// → 打开分类页 → 从此分类移除 → 返回。
final class CollectionsTests: XCTestCase {
    func shot(_ name: String) {
        let a = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        a.name = name
        a.lifetime = .keepAlways
        add(a)
    }

    /// 在系统命名对话框里输入名称并确认。
    func name(_ app: XCUIApplication, _ text: String, ok: String = "创建") {
        let field = app.alerts.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.typeText(text)
        app.alerts.buttons[ok].tap()
    }

    /// 长按第 i 行打开菜单（与 ⋯ 相同的内容）；prefix 为 "collection-row" 时是分类页里的行。
    func rowMenu(_ app: XCUIApplication, _ i: Int, prefix: String = "row") {
        // 四个 Tab 常驻（首页也有 row-0）：取当前可点的那一个
        let rows = app.descendants(matching: .any).matching(identifier: "\(prefix)-\(i)")
        XCTAssertTrue(rows.firstMatch.waitForExistence(timeout: 5))
        guard let row = rows.allElementsBoundByIndex.first(where: \.isHittable) else {
            return XCTFail("row-\(i) not hittable")
        }
        row.press(forDuration: 1.0)
    }

    /// 菜单里点“添加到分类”，等子菜单展开（以“新建分类…”出现为准）。
    func submenu(_ app: XCUIApplication) {
        let item = app.collectionViews.buttons["添加到分类"]
        XCTAssertTrue(item.waitForExistence(timeout: 5))
        item.tap()
        XCTAssertTrue(app.collectionViews.buttons["新建分类…"].waitForExistence(timeout: 5))
    }

    func waitLabel(_ el: XCUIElement, _ text: String, file: StaticString = #filePath, line: UInt = #line) {
        let ok = NSPredicate(format: "label CONTAINS %@", text)
        let r = XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: ok, object: el)], timeout: 5)
        XCTAssertEqual(r, .completed, "\(el) label should contain \(text), got \(el.label)", file: file, line: line)
    }

    func testCreateAddOpenRemove() throws {
        let app = XCUIApplication()
        app.launchArguments = ["-resetData", "YES", "-theme", "dark", "-uiTestScreen", "library",
                               "-AppleLanguages", "(zh-Hans)", "-AppleKeyboards", "(en_US@sw=QWERTY;hw=Automatic)"]
        app.launch()

        // 1. 新建分类
        let new = app.buttons["collection-new"]
        XCTAssertTrue(new.waitForExistence(timeout: 15))
        new.tap()
        name(app, "Morning")
        XCTAssertTrue(app.buttons["collection-Morning"].waitForExistence(timeout: 5))
        shot("collections-01-created")

        // 2. 重名（不区分大小写）：对话框带着错误提示重新弹出
        new.tap()
        name(app, "morning")
        XCTAssertTrue(app.alerts.staticTexts["已有同名分类"].waitForExistence(timeout: 5))
        shot("collections-02-duplicate")
        app.alerts.buttons["取消"].tap()

        // 3. 文章菜单 → 添加到分类 → Morning
        rowMenu(app, 0)
        submenu(app)
        shot("collections-03a-submenu")
        app.collectionViews.buttons["Morning"].tap()
        waitLabel(app.buttons["collection-Morning"], "1 篇")

        // 4. 同一篇：菜单里新建第二个分类，建好后自动加入
        rowMenu(app, 0)
        submenu(app)
        app.collectionViews.buttons["新建分类…"].tap()
        name(app, "Bedtime")
        let bedtime = app.buttons["collection-Bedtime"]
        XCTAssertTrue(bedtime.waitForExistence(timeout: 5))
        waitLabel(bedtime, "1 篇")
        waitLabel(app.buttons["collection-Morning"], "1 篇")  // 仍在第一个分类里
        shot("collections-03-two-collections")

        // 5. 打开分类页
        app.buttons["collection-Morning"].tap()
        XCTAssertTrue(app.buttons["播放全部"].waitForExistence(timeout: 5))
        shot("collections-04-page")

        // 6. 从此分类移除 → 空分类
        rowMenu(app, 0, prefix: "collection-row")
        let remove = app.collectionViews.buttons["从此分类移除"]
        XCTAssertTrue(remove.waitForExistence(timeout: 5))
        remove.tap()
        XCTAssertTrue(app.staticTexts["空分类"].waitForExistence(timeout: 5))
        shot("collections-05-removed")

        // 7. 返回资料库：Bedtime 里仍有这一篇
        app.buttons["返回资料库"].tap()
        XCTAssertTrue(app.buttons["collection-Bedtime"].waitForExistence(timeout: 5))
        waitLabel(app.buttons["collection-Bedtime"], "1 篇")
        waitLabel(app.buttons["collection-Morning"], "0 篇")
    }
}
