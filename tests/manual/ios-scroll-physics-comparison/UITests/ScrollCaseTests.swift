// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:ignore XCUI

import XCTest

/// Captures UIKit and Slint scrolling the same delivered touches, one scenario per app launch.
/// `scripts/collect.py` sorts the saved traces into the folders under `cases/`.
final class ScrollCaseTests: XCTestCase {
    private static let trials = 1...2
    private static let touchSampleRate = 120.0
    private static let settleSeconds = 4.0
    private static let viewportHeight = 774.0
    private static let middleOffset = 7_200.0

    /// One touch: down at the first point, moved through the others, lifted at `lift`.
    private struct TouchPath {
        var points: [(time: Double, x: Double, y: Double)]
        var lift: Double
    }

    private struct PathBuilder {
        private(set) var points: [(time: Double, x: Double, y: Double)]
        private(set) var time: Double

        init(x: Double, y: Double, at time: Double = 0) {
            points = [(time, x, y)]
            self.time = time
        }

        mutating func hold(_ seconds: Double) {
            time += seconds
        }

        mutating func move(by dy: Double, speed: Double) {
            let duration = abs(dy) / speed
            let steps = max(1, Int((duration * ScrollCaseTests.touchSampleRate).rounded(.up)))
            let start = points.last!
            for step in 1...steps {
                let progress = Double(step) / Double(steps)
                points.append((time + duration * progress, start.x, start.y + dy * progress))
            }
            time += duration
        }

        func lifted() -> TouchPath {
            TouchPath(points: points, lift: time)
        }
    }

    private func scenario(_ number: Int, _ parameters: KeyValuePairs<String, Double>,
                          trial: Int) -> String {
        let values = parameters.map { "\($0.key)\(String(format: "%04d", Int($0.value)))" }
        return ([String(format: "case%02d", number)] + values + ["trial\(trial)"])
            .joined(separator: "-")
    }

    private func capture(_ scenario: String, startOffset: Double,
                         _ makePaths: (CGRect) -> [TouchPath]) {
        let app = XCUIApplication()
        app.launchEnvironment["SLINT_BACKEND"] = "winit-skia"
        app.launchEnvironment["SCROLL_SCENARIO"] = scenario
        app.launchEnvironment["START_OFFSET"] = String(startOffset)
        app.launchEnvironment["VIEWPORT_HEIGHT"] = String(Self.viewportHeight)
        app.launchEnvironment["INPUT_TRACE"] = "1"
        app.launchEnvironment["TRACE_SAVE_DELAY_MS"] = String(Int(Self.settleSeconds * 1_000))
        app.launch()
        XCTAssertTrue(app.staticTexts["Scroll comparison metrics"].waitForExistence(timeout: 10))

        let processID = (app.value(forKey: "processID") as! NSNumber).int32Value
        let paths = makePaths(app.frame)
        let counts = paths.map { Int32($0.points.count) }
        let points = paths.flatMap(\.points)
        let lifts = paths.map(\.lift)
        XCTAssertTrue(
            synthesizeTouchPaths(processID, Int32(paths.count), counts, points.map { $0.time },
                                 points.map { $0.x }, points.map { $0.y }, lifts),
            "Failed to deliver \(scenario)")

        // The app saves its traces `settleSeconds` after the last lift.
        let saved = expectation(description: "Traces of \(scenario) saved")
        let wait = lifts.max()! + Self.settleSeconds + 0.5
        DispatchQueue.main.asyncAfter(deadline: .now() + wait) { saved.fulfill() }
        self.wait(for: [saved], timeout: wait + 2)
        app.terminate()
    }

    private func flick(_ frame: CGRect, by dy: Double, speed: Double) -> PathBuilder {
        var touch = PathBuilder(x: frame.width * 0.5, y: frame.height * 0.65)
        touch.hold(0.08)
        touch.move(by: dy, speed: speed)
        return touch
    }

    private func pull(_ frame: CGRect, from relativeY: Double = 0.18, by dy: Double,
                      speed: Double) -> PathBuilder {
        var touch = PathBuilder(x: frame.width * 0.2, y: frame.height * relativeY)
        touch.hold(0.08)
        touch.move(by: dy, speed: speed)
        return touch
    }

    func testCase01FlingInside() {
        for speed in [300.0, 500, 800, 1_200, 2_000, 3_000] {
            for trial in Self.trials {
                capture(scenario(1, ["speed": speed], trial: trial),
                        startOffset: Self.middleOffset) { frame in
                    [flick(frame, by: -120, speed: speed).lifted()]
                }
            }
        }
    }

    func testCase02FlingIntoEdge() {
        for offset in [200.0, 600] {
            for speed in [1_500.0, 2_500, 3_500, 4_500] {
                for trial in Self.trials {
                    capture(scenario(2, ["offset": offset, "speed": speed], trial: trial),
                            startOffset: offset) { frame in
                        var touch = PathBuilder(x: frame.width * 0.5, y: frame.height * 0.3)
                        touch.hold(0.08)
                        touch.move(by: 100, speed: speed)
                        return [touch.lifted()]
                    }
                }
            }
        }
    }

    func testCase03ReleaseOutsideMovingOutward() {
        for distance in [100.0, 300, 600] {
            for speed in [200.0, 400, 600, 800, 1_200, 1_600, 2_000] {
                for trial in Self.trials {
                    capture(scenario(3, ["distance": distance, "speed": speed], trial: trial),
                            startOffset: 0) { frame in
                        [pull(frame, by: distance, speed: speed).lifted()]
                    }
                }
            }
        }
    }

    func testCase04ReleaseOutsideHeld() {
        for distance in [50.0, 100, 200, 300, 600] {
            for trial in Self.trials {
                let name = scenario(4, ["distance": distance], trial: trial)
                capture(name, startOffset: 0) { frame in
                    var touch = pull(frame, by: distance, speed: min(400, distance / 0.5))
                    touch.hold(0.4)
                    return [touch.lifted()]
                }
            }
        }
    }

    func testCase05ReleaseOutsideMovingInward() {
        for speed in [300.0, 800, 1_500] {
            for trial in Self.trials {
                capture(scenario(5, ["speed": speed], trial: trial), startOffset: 0) { frame in
                    var touch = pull(frame, by: 300, speed: 400)
                    touch.hold(0.2)
                    touch.move(by: -150, speed: speed)
                    return [touch.lifted()]
                }
            }
        }
    }

    func testCase06FlingNearMinimumSpeed() {
        for speed in [100.0, 150, 200, 250, 300, 350, 400, 500] {
            for trial in Self.trials {
                capture(scenario(6, ["speed": speed], trial: trial),
                        startOffset: Self.middleOffset) { frame in
                    [flick(frame, by: -40, speed: speed).lifted()]
                }
            }
        }
    }

    func testCase07ReversalInOverscroll() {
        for back in [150.0, 300, 450] {
            for trial in Self.trials {
                capture(scenario(7, ["back": back], trial: trial), startOffset: 0) { frame in
                    var touch = pull(frame, from: 0.3, by: 300, speed: 400)
                    touch.move(by: -back, speed: 400)
                    touch.hold(0.3)
                    return [touch.lifted()]
                }
            }
        }
    }

    func testCase08TouchDuringDeceleration() {
        for delay in [100.0, 300, 600] {
            for trial in Self.trials {
                capture(scenario(8, ["delay": delay], trial: trial),
                        startOffset: Self.middleOffset) { frame in
                    let fling = flick(frame, by: -120, speed: 1_500).lifted()
                    var stop = PathBuilder(x: frame.width * 0.5, y: frame.height * 0.5,
                                           at: fling.lift + delay / 1_000)
                    stop.hold(0.3)
                    return [fling, stop.lifted()]
                }
            }
        }
    }

    func testCase09TouchDuringSpringBack() {
        for delay in [30.0, 100, 250] {
            for trial in Self.trials {
                capture(scenario(9, ["delay": delay], trial: trial), startOffset: 0) { frame in
                    var first = pull(frame, by: 300, speed: 400)
                    first.hold(0.4)
                    let release = first.lifted()
                    var catchTouch = PathBuilder(x: frame.width * 0.5, y: frame.height * 0.4,
                                                 at: release.lift + delay / 1_000)
                    catchTouch.hold(0.08)
                    catchTouch.move(by: 50, speed: 250)
                    catchTouch.hold(0.2)
                    return [release, catchTouch.lifted()]
                }
            }
        }
    }
}
