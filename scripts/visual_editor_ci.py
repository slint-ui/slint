#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Run UI tests alongside sequential Visual Editor Rust checks."""

import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path


def main():
    root = Path(__file__).resolve().parents[1]
    target = Path(os.environ.get("CARGO_TARGET_DIR", root / "target")).resolve()
    reports = root / "target" / "visual-editor-ci"
    reports.mkdir(parents=True, exist_ok=True)
    common = [
        "--locked",
        "-p",
        "slint-editor",
        "--all-features",
        "--features",
        "slint/mcp",
        "--timings",
    ]
    records = []
    started = time.monotonic()

    def run(name, command, cwd=root, env=None):
        timing = target / "cargo-timings" / "cargo-timing.html"
        previous = timing.stat().st_mtime_ns if timing.exists() else None
        begin = time.monotonic()
        print(f"Starting {name}: {' '.join(command)}", flush=True)
        with (reports / f"{name}.log").open("w") as log:
            process = subprocess.run(
                command,
                cwd=cwd,
                env=env,
                stdout=log,
                stderr=subprocess.STDOUT,
                check=False,
            )
        record = {
            "phase": name,
            "command": command,
            "start_seconds": begin - started,
            "seconds": time.monotonic() - begin,
            "exit_code": process.returncode,
        }
        records.append(record)
        if name != "ui" and timing.exists() and timing.stat().st_mtime_ns != previous:
            shutil.copyfile(timing, reports / f"{name}-timing.html")
        print(
            f"Finished {name}: {record['seconds']:.2f}s, exit {process.returncode}",
            flush=True,
        )
        if process.returncode:
            print((reports / f"{name}.log").read_text()[-12000:], flush=True)
        return process.returncode

    try:
        if run("build", ["cargo", "build", *common]):
            return 1
        with tempfile.TemporaryDirectory(prefix="slint-editor-ci-") as directory:
            binary = Path(directory) / "slint-editor"
            shutil.copy2(target / "debug" / "slint-editor", binary)
            with binary.open("rb") as file:
                (reports / "binary-sha256.txt").write_text(
                    hashlib.file_digest(file, "sha256").hexdigest() + "\n"
                )
            with ThreadPoolExecutor(max_workers=1) as executor:
                ui = None
                if os.environ.get("UI_TESTS", "true") == "true":
                    ui = executor.submit(
                        run,
                        "ui",
                        ["./run-tests.sh"],
                        root / "tools/editor/ui-tests",
                        os.environ
                        | {
                            "SLINT_BACKEND": "headless-skia",
                            "SLINT_EDITOR_BINARY": str(binary),
                        },
                    )
                lint_status = run(
                    "clippy",
                    [
                        "cargo",
                        "clippy",
                        *common,
                        "--all-targets",
                        "--",
                        "-D",
                        "warnings",
                    ],
                )
                test_status = run("test", ["cargo", "test", *common])
                ui_status = ui.result() if ui else 0
        return int(bool(lint_status or test_status or ui_status))
    finally:
        elapsed = time.monotonic() - started
        metadata = {
            "seconds": elapsed,
            "phases": sorted(records, key=lambda r: r["start_seconds"]),
            "environment": {
                key: os.environ.get(key)
                for key in [
                    "CARGO_INCREMENTAL",
                    "CARGO_PROFILE_DEV_DEBUG",
                    "RUSTFLAGS",
                    "SLINT_EMIT_DEBUG_INFO",
                    "MACOSX_DEPLOYMENT_TARGET",
                    "SLINT_COMPILER_DENY_WARNINGS",
                    "UI_TESTS",
                ]
            },
        }
        (reports / "timings.json").write_text(json.dumps(metadata, indent=2) + "\n")
        summary = [
            "## Visual Editor overlap experiment",
            "",
            "| Phase | Start | Duration | Exit |",
            "| --- | ---: | ---: | ---: |",
        ]
        for record in metadata["phases"]:
            summary.append(
                f"| {record['phase']} | {record['start_seconds']:.2f}s | {record['seconds']:.2f}s | {record['exit_code']} |"
            )
        if os.environ.get("UI_TESTS", "true") != "true":
            summary.append("| UI tests | | Skipped: private dependency unavailable | |")
        summary.extend(["", f"Total check wall time: {elapsed:.2f}s", ""])
        (reports / "summary.md").write_text("\n".join(summary))
        if path := os.environ.get("GITHUB_STEP_SUMMARY"):
            with Path(path).open("a") as file:
                file.write("\n".join(summary))
        print(f"Total check wall time: {elapsed:.2f}s", flush=True)


if __name__ == "__main__":
    raise SystemExit(main())
