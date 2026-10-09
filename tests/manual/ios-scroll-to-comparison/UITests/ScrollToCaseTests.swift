// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:ignore XCUI

import XCTest

/// Captures UIKit's `setContentOffset(_:animated:)` and Slint's smooth `scroll-to` side by side,
/// one scenario per app launch. `scripts/collect.py` sorts the saved traces into `cases/`.
final class ScrollToCaseTests: XCTestCase {
    private static let trials = 1...2
    private static let touchSampleRate = 120.0
    /// How long both lists are recorded after the last scroll-to.
    private static let settleSeconds = 3.0
    /// The app waits this long after its first layout before recording, so launch work is over.
    private static let planStartSeconds = 1.5
    /// Recorded at rest before the first scroll-to.
    private static let leadInSeconds = 0.3
    private static let viewportHeight = 774.0
    private static let contentHeight = 72_000.0
    private static let maximumOffset = ScrollToCaseTests.contentHeight - ScrollToCaseTests.viewportHeight
    private static let middleOffset = 36_000.0
    private static let distances: [Double] = [25, 50, 100, 200, 400, 800, 1_600, 3_200, 7_200, 20_000]

    private struct Command {
        var delay: Double
        var target: Double
    }

    private struct TouchPath {
        var points: [(time: Double, x: Double, y: Double)]
        var lift: Double
    }

    private func scenario(_ number: Int, _ parameters: KeyValuePairs<String, Double>,
                          trial: Int) -> String {
        let values = parameters.map { "\($0.key)\(String(format: "%04d", Int($0.value)))" }
        return ([String(format: "case%02d", number)] + values + ["trial\(trial)"])
            .joined(separator: "-")
    }

    private func launch(_ scenario: String, startOffset: Double, commands: [Command],
                        anchor: String) -> XCUIApplication {
        let app = XCUIApplication()
        let plan = commands.map { String(format: "%.3f:%.3f", $0.delay, $0.target) }
        let saveDelay = commands.map(\.delay).max()! + Self.settleSeconds
        app.launchEnvironment["SLINT_BACKEND"] = "winit-skia"
        app.launchEnvironment["SCROLL_SCENARIO"] = scenario
        app.launchEnvironment["START_OFFSET"] = String(startOffset)
        app.launchEnvironment["VIEWPORT_HEIGHT"] = String(Self.viewportHeight)
        app.launchEnvironment["SCROLL_TO_ANCHOR"] = anchor
        app.launchEnvironment["SCROLL_TO_COMMANDS"] = plan.joined(separator: ";")
        app.launchEnvironment["PLAN_START_DELAY_MS"] = String(Int(Self.planStartSeconds * 1_000))
        app.launchEnvironment["PLAN_LEAD_IN_MS"] = String(Int(Self.leadInSeconds * 1_000))
        app.launchEnvironment["TRACE_SAVE_DELAY_MS"] = String(Int(saveDelay * 1_000))
        app.launchEnvironment["INPUT_TRACE"] = anchor == "release" ? "1" : "0"
        app.launch()
        XCTAssertTrue(app.staticTexts["Scroll comparison metrics"].waitForExistence(timeout: 10))
        return app
    }

    /// Sleeps without touching the app, then waits for it to report the saved trace.
    private func waitForSavedTrace(_ app: XCUIApplication, _ scenario: String, after seconds: Double) {
        let idle = expectation(description: "\(scenario) recorded")
        DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { idle.fulfill() }
        wait(for: [idle], timeout: seconds + 2)
        let status = app.staticTexts["Trace status"]
        let saved = expectation(for: NSPredicate(format: "label == 'saved'"), evaluatedWith: status)
        wait(for: [saved], timeout: 15)
        app.terminate()
    }

    /// Both lists start at `startOffset` and scroll to the targets of `commands`; the delays are
    /// seconds from the end of the lead-in.
    private func capture(_ scenario: String, startOffset: Double, _ commands: [Command]) {
        let app = launch(scenario, startOffset: startOffset, commands: commands, anchor: "launch")
        let duration = Self.planStartSeconds + Self.leadInSeconds + commands.map(\.delay).max()!
            + Self.settleSeconds
        waitForSavedTrace(app, scenario, after: duration)
    }

    /// Like `capture`, but the delays are seconds from the release of a fling.
    private func captureAfterFling(_ scenario: String, startOffset: Double, dy: Double,
                                   speed: Double, _ commands: [Command]) {
        let app = launch(scenario, startOffset: startOffset, commands: commands, anchor: "release")
        let x = Double(app.frame.width) * 0.5
        let y = Double(app.frame.height) * 0.65
        var points: [(time: Double, x: Double, y: Double)] = [(0, x, y)]
        let moveStart = 0.08
        let duration = abs(dy) / speed
        let steps = max(1, Int((duration * Self.touchSampleRate).rounded(.up)))
        for step in 1...steps {
            let progress = Double(step) / Double(steps)
            points.append((moveStart + duration * progress, x, y + dy * progress))
        }
        let path = TouchPath(points: points, lift: moveStart + duration)
        let processID = (app.value(forKey: "processID") as! NSNumber).int32Value
        XCTAssertTrue(
            synthesizeTouchPaths(processID, 1, [Int32(path.points.count)],
                                 path.points.map { $0.time }, path.points.map { $0.x },
                                 path.points.map { $0.y }, [path.lift]),
            "Failed to deliver \(scenario)")
        waitForSavedTrace(app, scenario,
                          after: path.lift + commands.map(\.delay).max()! + Self.settleSeconds)
    }

    func testCase01ScrollDownFromRest() {
        for distance in Self.distances {
            for trial in Self.trials {
                capture(scenario(1, ["distance": distance], trial: trial),
                        startOffset: Self.middleOffset,
                        [Command(delay: 0, target: Self.middleOffset + distance)])
            }
        }
    }

    func testCase02ScrollUpFromRest() {
        for distance in Self.distances {
            for trial in Self.trials {
                capture(scenario(2, ["distance": distance], trial: trial),
                        startOffset: Self.middleOffset,
                        [Command(delay: 0, target: Self.middleOffset - distance)])
            }
        }
    }

    /// `edge` 0 scrolls to the top, 1 to the bottom.
    func testCase03ScrollToEdge() {
        for edge in [0.0, 1] {
            for distance in [500.0, 5_000] {
                for trial in Self.trials {
                    let target = edge == 0 ? 0 : Self.maximumOffset
                    let start = edge == 0 ? distance : Self.maximumOffset - distance
                    capture(scenario(3, ["edge": edge, "distance": distance], trial: trial),
                            startOffset: start, [Command(delay: 0, target: target)])
                }
            }
        }
    }

    /// A second scroll-to further in the same direction while the first one runs.
    func testCase04RetargetSameDirection() {
        for delay in [50.0, 100, 200, 400] {
            for trial in Self.trials {
                capture(scenario(4, ["delay": delay], trial: trial),
                        startOffset: Self.middleOffset,
                        [Command(delay: 0, target: Self.middleOffset + 3_000),
                         Command(delay: delay / 1_000, target: Self.middleOffset + 6_000)])
            }
        }
    }

    /// A second scroll-to back to the start while the first one runs.
    func testCase05RetargetReverse() {
        for delay in [50.0, 100, 200, 400] {
            for trial in Self.trials {
                capture(scenario(5, ["delay": delay], trial: trial),
                        startOffset: Self.middleOffset,
                        [Command(delay: 0, target: Self.middleOffset + 3_000),
                         Command(delay: delay / 1_000, target: Self.middleOffset)])
            }
        }
    }

    /// A scroll-to during the deceleration of a fling towards larger offsets.
    /// `reverse` 0 targets an offset ahead of the fling, 1 an offset behind the start.
    func testCase06ScrollDuringFling() {
        for reverse in [0.0, 1] {
            for delay in [50.0, 150, 300] {
                for trial in Self.trials {
                    let target = Self.middleOffset + (reverse == 0 ? 3_000 : -1_000)
                    captureAfterFling(scenario(6, ["reverse": reverse, "delay": delay], trial: trial),
                                      startOffset: Self.middleOffset, dy: -120, speed: 2_000,
                                      [Command(delay: delay / 1_000, target: target)])
                }
            }
        }
    }
}
