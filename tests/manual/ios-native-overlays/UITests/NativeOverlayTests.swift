// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:ignore Defocuses texta XCUI

import XCTest

final class NativeOverlayTests: XCTestCase {
    private func nativeEditor(_ app: XCUIApplication) -> XCUIElement {
        let editor =
            app.descendants(matching: .any).matching(identifier: "Native text editor").firstMatch
        XCTAssertTrue(editor.waitForExistence(timeout: 10))
        return editor
    }

    private func multilineEditor(_ app: XCUIApplication) -> XCUIElement {
        let editor = app.descendants(matching: .any)
            .matching(identifier: "Native multiline text editor").firstMatch
        XCTAssertTrue(editor.waitForExistence(timeout: 10))
        return editor
    }

    private func nativeMultilineEditor(_ app: XCUIApplication) -> XCUIElement {
        let editor = app.textViews["Pure UIKit multiline editor"]
        XCTAssertTrue(editor.waitForExistence(timeout: 10))
        return editor
    }

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        app.launch()
        return app
    }

    private func keepScreenshot(_ app: XCUIApplication, name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func trackpadEndpointDisplacement(
        _ app: XCUIApplication,
        editor: XCUIElement,
        dragOffset: CGVector
    ) -> Int {
        editor.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.75)).tap()
        XCTAssertTrue(app.keyboards.element.waitForExistence(timeout: 5))

        let original = editor.value as? String ?? ""
        let space = app.keys["space"]
        XCTAssertTrue(space.waitForExistence(timeout: 5))
        let start = space.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        start.press(
            forDuration: 1,
            thenDragTo: start.withOffset(dragOffset),
            withVelocity: .slow,
            thenHoldForDuration: 0.5
        )
        app.typeText("Z")

        let updated = editor.value as? String ?? ""
        guard let marker = updated.firstIndex(of: "Z") else {
            XCTFail("Trackpad gesture did not leave the editor ready for text input")
            return original.utf16.count
        }
        return original.utf16.count - updated[..<marker].utf16.count
    }

    func testHorizontalKeyboardTrackpadEndpointMatchesUIKit() {
        let app = launch()
        let dragOffset = CGVector(dx: -60, dy: 0)
        let slintDisplacement = trackpadEndpointDisplacement(
            app, editor: multilineEditor(app), dragOffset: dragOffset)
        let nativeDisplacement = trackpadEndpointDisplacement(
            app, editor: nativeMultilineEditor(app), dragOffset: dragOffset)

        let evidence = XCTAttachment(
            string: "Slint displacement: \(slintDisplacement) UTF-16 units\n"
                + "UIKit displacement: \(nativeDisplacement) UTF-16 units"
        )
        evidence.name = "keyboard-trackpad-endpoint-displacement"
        evidence.lifetime = .keepAlways
        add(evidence)

        XCTAssertLessThanOrEqual(abs(slintDisplacement - nativeDisplacement), 2)
    }

    func testVerticalKeyboardTrackpadEndpointMatchesUIKit() {
        let app = launch()
        let dragOffset = CGVector(dx: 0, dy: -20)
        let slintDisplacement = trackpadEndpointDisplacement(
            app, editor: multilineEditor(app), dragOffset: dragOffset)
        let nativeDisplacement = trackpadEndpointDisplacement(
            app, editor: nativeMultilineEditor(app), dragOffset: dragOffset)

        let evidence = XCTAttachment(
            string: "Slint displacement: \(slintDisplacement) UTF-16 units\n"
                + "UIKit displacement: \(nativeDisplacement) UTF-16 units"
        )
        evidence.name = "vertical-keyboard-trackpad-endpoint-displacement"
        evidence.lifetime = .keepAlways
        add(evidence)

        XCTAssertLessThanOrEqual(abs(slintDisplacement - nativeDisplacement), 2)
    }

    func testNativeTextEditorAndSelectionMenu() {
        let app = launch()
        let editor = nativeEditor(app)
        editor.tap()
        XCTAssertTrue(app.keyboards.element.waitForExistence(timeout: 5))
        editor.typeText(" – UIKit")
        XCTAssertEqual(editor.value as? String, "Edit this text – UIKit")
        editor.coordinate(withNormalizedOffset: CGVector(dx: 0.25, dy: 0.5)).doubleTap()
        XCTAssertTrue(app.keyboards.element.exists)
        XCTAssertTrue(app.menuItems["Copy"].waitForExistence(timeout: 5))
        keepScreenshot(app, name: "native-text-selection")
    }

    func testKeyboardKeyReachesNativeEditor() {
        let app = launch()
        let editor = nativeEditor(app)
        editor.tap()
        XCTAssertTrue(app.keyboards.element.waitForExistence(timeout: 5))

        let originalValue = editor.value as? String
        let key = app.keys["a"]
        XCTAssertTrue(key.waitForExistence(timeout: 5))
        key.tap()

        let valueChanged = NSPredicate(format: "value != %@", originalValue ?? "")
        expectation(for: valueChanged, evaluatedWith: editor)
        waitForExpectations(timeout: 5)
        XCTAssertEqual(editor.value as? String, "Edit this texta")
    }

    func testNativeStartHandleCanExpandSelection() {
        let app = launch()
        let editor = nativeEditor(app)
        editor.tap()
        XCTAssertTrue(app.keyboards.element.waitForExistence(timeout: 5))

        editor.coordinate(withNormalizedOffset: CGVector(dx: 0.25, dy: 0.5)).doubleTap()
        XCTAssertTrue(app.menuItems["Copy"].waitForExistence(timeout: 5))

        let startHandle = editor.coordinate(withNormalizedOffset: CGVector(dx: 0.22, dy: 0.25))
        let expandedStart = editor.coordinate(withNormalizedOffset: CGVector(dx: 0.05, dy: 0.25))
        startHandle.press(forDuration: 0.1, thenDragTo: expandedStart)
        keepScreenshot(app, name: "native-start-handle-expanded")

        let original = editor.value as? String ?? ""
        let delete = app.keys["delete"]
        XCTAssertTrue(delete.waitForExistence(timeout: 5))
        delete.tap()
        let updated = editor.value as? String ?? ""
        XCTAssertLessThan(updated.count, original.count - 4)
    }

    func testSecondSlintFieldDefocusesNativeEditor() {
        let app = launch()
        let editor = nativeEditor(app)
        let secondEditor = multilineEditor(app)
        editor.tap()
        XCTAssertTrue(app.keyboards.element.waitForExistence(timeout: 5))
        let nativeValue = editor.value as? String
        editor.coordinate(withNormalizedOffset: CGVector(dx: 0.75, dy: 0.5)).doubleTap()
        XCTAssertTrue(app.menuItems["Copy"].waitForExistence(timeout: 5))

        secondEditor.tap()

        XCTAssertTrue(app.keyboards.element.exists)
        XCTAssertTrue(app.menuItems["Copy"].waitForNonExistence(timeout: 5))
        let key = app.keys["a"]
        XCTAssertTrue(key.waitForExistence(timeout: 5))
        key.tap()
        XCTAssertEqual(editor.value as? String, nativeValue)
        keepScreenshot(app, name: "second-slint-field-focused")

        editor.tap()
        XCTAssertTrue(app.keyboards.element.exists)
        let refocusedValue = editor.value as? String
        key.tap()
        let valueChanged = NSPredicate(format: "value != %@", refocusedValue ?? "")
        expectation(for: valueChanged, evaluatedWith: editor)
        waitForExpectations(timeout: 5)
        keepScreenshot(app, name: "native-field-refocused")
    }

    func testMultilineEditorUsesNativeTextControls() {
        let app = launch()
        let editor = multilineEditor(app)
        editor.tap()
        XCTAssertTrue(app.keyboards.element.waitForExistence(timeout: 5))

        editor.typeText("\nA third editable line")
        let value = editor.value as? String ?? ""
        XCTAssertTrue(value.contains("\nA third editable line"))

        editor.coordinate(withNormalizedOffset: CGVector(dx: 0.25, dy: 0.25)).doubleTap()
        XCTAssertTrue(app.menuItems["Copy"].waitForExistence(timeout: 5))
        keepScreenshot(app, name: "native-multiline-selection")
    }

    func testMultilineSelectionFollowsHandleDrag() {
        let app = launch()
        let editor = multilineEditor(app)
        editor.tap()
        XCTAssertTrue(app.keyboards.element.waitForExistence(timeout: 5))

        editor.coordinate(withNormalizedOffset: CGVector(dx: 0.25, dy: 0.25)).doubleTap()
        XCTAssertTrue(app.menuItems["Copy"].waitForExistence(timeout: 5))

        let endHandle = editor.coordinate(withNormalizedOffset: CGVector(dx: 0.29, dy: 0.32))
        let thirdLine = editor.coordinate(withNormalizedOffset: CGVector(dx: 0.82, dy: 0.74))
        endHandle.press(
            forDuration: 0.1,
            thenDragTo: thirdLine,
            withVelocity: .slow,
            thenHoldForDuration: 1
        )
        keepScreenshot(app, name: "native-multiline-handle-drag")

        let original = editor.value as? String ?? ""
        let delete = app.keys["delete"]
        XCTAssertTrue(delete.waitForExistence(timeout: 5))
        delete.tap()
        let updated = editor.value as? String ?? ""
        XCTAssertLessThan(updated.count, original.count - 6)
    }

    func testMultilineSelectionFollowsLeadingHandleUpward() {
        let app = launch()
        let editor = multilineEditor(app)
        editor.tap()
        XCTAssertTrue(app.keyboards.element.waitForExistence(timeout: 5))

        editor.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.36)).doubleTap()
        XCTAssertTrue(app.menuItems["Copy"].waitForExistence(timeout: 5))

        let startHandle = editor.coordinate(withNormalizedOffset: CGVector(dx: 0.48, dy: 0.20))
        let previousLine = editor.coordinate(withNormalizedOffset: CGVector(dx: 0.1, dy: 0.12))
        startHandle.press(
            forDuration: 0.1,
            thenDragTo: previousLine,
            withVelocity: .slow,
            thenHoldForDuration: 1
        )
        keepScreenshot(app, name: "native-multiline-leading-handle-upward")

        let original = editor.value as? String ?? ""
        let delete = app.keys["delete"]
        XCTAssertTrue(delete.waitForExistence(timeout: 5))
        delete.tap()
        let updated = editor.value as? String ?? ""
        XCTAssertLessThan(updated.count, original.count - 4)
    }

    func testMultilineSelectionFollowsSlintScroll() {
        let app = launch()
        let editor = multilineEditor(app)
        editor.tap()
        XCTAssertTrue(app.keyboards.element.waitForExistence(timeout: 5))

        editor.coordinate(withNormalizedOffset: CGVector(dx: 0.35, dy: 0.3)).doubleTap()
        XCTAssertTrue(app.menuItems["Copy"].waitForExistence(timeout: 5))
        let originalValue = editor.value as? String ?? ""
        let originalFrame = editor.frame

        let dragStart = editor.coordinate(
            withNormalizedOffset: CGVector(dx: 0.5, dy: 1.08)
        )
        dragStart.press(
            forDuration: 0.1,
            thenDragTo: dragStart.withOffset(CGVector(dx: 0, dy: -60)),
            withVelocity: .slow,
            thenHoldForDuration: 0.1
        )

        let editorMoved = NSPredicate { _, _ in
            editor.frame.minY < originalFrame.minY - 20
        }
        expectation(for: editorMoved, evaluatedWith: editor)
        waitForExpectations(timeout: 5)
        XCTAssertTrue(app.keyboards.element.exists)
        keepScreenshot(app, name: "selection-after-slint-scroll")

        let evidence = XCTAttachment(
            string: "Before: \(originalFrame)\nAfter: \(editor.frame)"
        )
        evidence.name = "slint-scroll-editor-geometry"
        evidence.lifetime = .keepAlways
        add(evidence)

        app.typeText("Z")
        let updatedValue = editor.value as? String ?? ""
        XCTAssertLessThan(updatedValue.count, originalValue.count - 1)
    }

    func testNativeContextMenuCallsSlint() {
        let app = launch()
        let menuTarget = app.buttons["Show native menu"]
        XCTAssertTrue(menuTarget.waitForExistence(timeout: 10))
        menuTarget.tap()
        let duplicate = app.buttons["Duplicate"]
        XCTAssertTrue(duplicate.waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["Rename"].exists)
        XCTAssertTrue(app.buttons["Delete"].exists)
        keepScreenshot(app, name: "native-context-menu")
        duplicate.tap()
        let result = NSPredicate(format: "value == %@", "Duplicate selected")
        expectation(for: result, evaluatedWith: menuTarget)
        waitForExpectations(timeout: 5)
        keepScreenshot(app, name: "native-context-action-result")
    }

}
