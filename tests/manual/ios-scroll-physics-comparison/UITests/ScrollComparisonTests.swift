// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import XCTest

final class ScrollComparisonTests: XCTestCase {
    private func launch(
        scenario: String,
        startOffset: Double? = nil,
        fromBottomDistance: Double? = nil
    ) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["SCROLL_SCENARIO"] = scenario
        if let startOffset {
            app.launchEnvironment["START_OFFSET"] = String(startOffset)
        }
        if let fromBottomDistance {
            app.launchEnvironment["START_FROM_BOTTOM"] = "1"
            app.launchEnvironment["START_FROM_BOTTOM_DISTANCE"] = String(fromBottomDistance)
        }
        app.launch()
        XCTAssertTrue(app.staticTexts["UIKit"].waitForExistence(timeout: 10))
        return app
    }

    private func drag(
        _ app: XCUIApplication,
        from startY: CGFloat,
        to endY: CGFloat,
        velocity: CGFloat
    ) {
        let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.25, dy: startY))
        let end = app.coordinate(withNormalizedOffset: CGVector(dx: 0.25, dy: endY))
        start.press(
            forDuration: 0.02,
            thenDragTo: end,
            withVelocity: XCUIGestureVelocity(rawValue: velocity),
            thenHoldForDuration: 0
        )
    }

    private func waitForTrace(_ app: XCUIApplication, name: String) {
        let settled = expectation(description: "Capture \(name)")
        DispatchQueue.main.asyncAfter(deadline: .now() + 6) { settled.fulfill() }
        wait(for: [settled], timeout: 8)
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    func testVelocitySweep() {
        for velocity in [200.0, 400.0, 800.0, 1_600.0, 3_200.0, 6_400.0, 12_800.0, 25_600.0] {
            let name = "velocity-\(Int(velocity))"
            let app = launch(scenario: name)
            drag(app, from: 0.75, to: 0.30, velocity: velocity)
            waitForTrace(app, name: name)
            app.terminate()
        }
    }

    func testCurveTransitionSweep() {
        for velocity in stride(from: 450.0, through: 1_200.0, by: 50.0) {
            let name = "curve-velocity-\(Int(velocity))"
            let app = launch(scenario: name)
            drag(app, from: 0.75, to: 0.30, velocity: velocity)
            waitForTrace(app, name: name)
            app.terminate()
        }
    }

    func testLargeFastCurveSweep() {
        for velocity in [3_200.0, 4_800.0, 6_400.0, 8_000.0, 9_600.0, 11_200.0] {
            let name = "large-fast-velocity-\(Int(velocity))"
            let app = launch(scenario: name)
            drag(app, from: 0.90, to: 0.10, velocity: velocity)
            waitForTrace(app, name: name)
            app.terminate()
        }
    }

    func testMomentumCarry() {
        for flickCount in [2, 3, 4] {
            let name = "momentum-carry-\(flickCount)"
            let app = launch(scenario: name)
            for flick in 0..<flickCount {
                drag(app, from: 0.75, to: 0.30, velocity: 1_600)
                if flick + 1 < flickCount { usleep(120_000) }
            }
            waitForTrace(app, name: name)
            app.terminate()
        }
    }

    func testRepeatedHardFlickAcceleration() {
        for flickCount in 1...4 {
            let name = "repeated-hard-flicks-\(flickCount)"
            let app = launch(scenario: name)
            let processID = (app.value(forKey: "processID") as! NSNumber).int32Value
            XCTAssertTrue(synthesizeRapidFlicks(
                processID, app.frame.width, app.frame.height, Int32(flickCount)))
            waitForTrace(app, name: name)
            app.terminate()
        }
    }

    func testTopPullBounce() {
        for velocity in [400.0, 1_600.0, 3_200.0] {
            let name = "top-pull-\(Int(velocity))"
            let app = launch(scenario: name)
            drag(app, from: 0.30, to: 0.75, velocity: velocity)
            waitForTrace(app, name: name)
            app.terminate()
        }
    }

    func testBottomPullBounce() {
        for velocity in [400.0, 1_600.0, 3_200.0] {
            let name = "bottom-pull-\(Int(velocity))"
            let app = launch(scenario: name, fromBottomDistance: 0)
            drag(app, from: 0.75, to: 0.30, velocity: velocity)
            waitForTrace(app, name: name)
            app.terminate()
        }
    }

    func testFlingExhaustsBottom() {
        for (distance, velocity) in [(150.0, 400.0), (300.0, 800.0), (600.0, 1_600.0)] {
            let name = "exhaust-bottom-d\(Int(distance))-v\(Int(velocity))"
            let app = launch(scenario: name, fromBottomDistance: distance)
            drag(app, from: 0.75, to: 0.30, velocity: velocity)
            waitForTrace(app, name: name)
            app.terminate()
        }
    }

    func testFlingExhaustsTop() {
        for (offset, velocity) in [(150.0, 400.0), (300.0, 800.0), (600.0, 1_600.0)] {
            let name = "exhaust-top-d\(Int(offset))-v\(Int(velocity))"
            let app = launch(scenario: name, startOffset: offset)
            drag(app, from: 0.30, to: 0.75, velocity: velocity)
            waitForTrace(app, name: name)
            app.terminate()
        }
    }

    func testReverseDuringDeceleration() {
        let app = launch(scenario: "reverse-during-deceleration", startOffset: 1_500)
        drag(app, from: 0.75, to: 0.30, velocity: 1_600)
        usleep(120_000)
        drag(app, from: 0.30, to: 0.75, velocity: 1_600)
        waitForTrace(app, name: "reverse-during-deceleration")
    }

    func testTouchStopsDeceleration() {
        let app = launch(scenario: "touch-stops-deceleration", startOffset: 1_500)
        drag(app, from: 0.75, to: 0.30, velocity: 1_600)
        usleep(300_000)
        let stop = app.coordinate(withNormalizedOffset: CGVector(dx: 0.25, dy: 0.50))
        stop.press(forDuration: 0.5)
        waitForTrace(app, name: "touch-stops-deceleration")
    }
}
