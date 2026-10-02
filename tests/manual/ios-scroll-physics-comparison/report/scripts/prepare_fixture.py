#!/usr/bin/env python3
"""Build a disposable capture fixture without replacing this checkout's engine."""

# cspell:ignore Murmele Overscroll
import argparse
import subprocess
from pathlib import Path

HARNESS_COMMIT = "cc987e7d93d841c09f52d416044d2ca6fc35691f"
PREFIX = "tests/manual/ios-scroll-physics-comparison/"
FILES = [
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "src/main.rs",
    "native_scroll.m",
    "Info.plist",
    "project.yml",
    "UITests/ScrollComparisonTests.swift",
    "UITests/DisableQuiescence.m",
    "UITests/QuiescenceControl.h",
]
METHOD = """
    func testMurmeleLatestSpringValidation() {
        let cases: [(Double, Double, Double, Double)] = [
            (774, 50, 400, 0.4), (774, 200, 400, 0.4),
            (774, 600, 400, 0.4), (387, 500, 400, 0.4),
            (774, 100, 400, 0), (774, 100, 1200, 0),
        ]
        for (viewport, distance, speed, hold) in cases {
            for trial in 1...2 {
                let name = "validation-vp\\(Int(viewport))-d\\(Int(distance))-v\\(Int(speed))"
                    + "-hold\\(Int(hold * 1000))-trial\\(trial)"
                captureOverscrollPull(name: name, distance: distance,
                    duration: hold > 0 ? max(0.5, distance / speed) : distance / speed,
                    holdDuration: hold, physicsVariant: "baseline", viewportHeight: viewport)
            }
        }
    }

    // Each pull keeps at least five synthesized moves, at 60 per second.
    func testMurmeleReleaseSpeedSweep() {
        let cases: [(Double, [Double])] = [
            (100, [200, 300, 400, 500, 600, 800, 1000, 1200]),
            (300, [200, 300, 400, 500, 600, 800, 1000, 1200, 1500, 2000]),
            (600, [200, 400, 600, 800, 1000, 1200, 1500, 2000]),
        ]
        for (distance, speeds) in cases {
            for speed in speeds {
                for trial in 1...2 {
                    let name = "sweep-vp774-d\\(Int(distance))-v\\(Int(speed))-hold0-trial\\(trial)"
                    captureOverscrollPull(name: name, distance: distance, duration: distance / speed,
                        holdDuration: 0, physicsVariant: "baseline", viewportHeight: 774)
                }
            }
        }
    }

"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--fixture-dir", type=Path, required=True)
    args = parser.parse_args()
    source = args.source_root.resolve()
    fixture = args.fixture_dir.resolve()
    if fixture.exists():
        raise SystemExit(
            "Choose a new fixture directory; existing files are preserved."
        )
    contents = {}
    for name in FILES:
        contents[name] = subprocess.check_output(
            ["git", "show", f"{HARNESS_COMMIT}:{PREFIX}{name}"], cwd=source
        ).decode()
    contents["Cargo.toml"] = (
        contents["Cargo.toml"]
        .replace("../../../api/rs/slint", str(source / "api/rs/slint"))
        .replace("../../../internal/core", str(source / "internal/core"))
    )
    contents["project.yml"] = contents["project.yml"].replace(
        "$SRCROOT/../../../scripts/build_for_ios_with_cargo.bash",
        str(source / "scripts/build_for_ios_with_cargo.bash"),
    )
    anchor = "    private let returnCurveViewports = [774.0, 387.0]"
    if anchor not in contents["UITests/ScrollComparisonTests.swift"]:
        raise SystemExit("Unexpected archived harness layout.")
    contents["UITests/ScrollComparisonTests.swift"] = contents[
        "UITests/ScrollComparisonTests.swift"
    ].replace(anchor, METHOD + anchor, 1)
    for name, text in contents.items():
        path = fixture / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    print(fixture)


if __name__ == "__main__":
    main()
