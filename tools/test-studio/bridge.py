# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Run inside the target suite's Python environment and stream pytest events."""

import argparse
import contextlib
import inspect
import json
import subprocess
import sys
import time
from pathlib import Path

import pytest


class StudioPlugin:
    def __init__(self, events: Path, artifacts: Path):
        self.events = events
        self.artifacts = artifacts
        self.application = None
        self.nodeid = ""
        self.sequence = 0
        self.outcome = "Passed"
        self.duration = 0.0
        self.restorations = []

    def emit(self, kind, **data):
        with self.events.open("a", encoding="utf-8") as stream:
            stream.write(
                json.dumps({"kind": kind, "nodeid": self.nodeid, **data}) + "\n"
            )

    def capture(self):
        if self.application is None:
            return "", ""
        try:
            window = self.application.first_window
            if window is None:
                return "", "No application window"
            self.sequence += 1
            path = self.artifacts / f"capture-{self.sequence:04d}.png"
            path.write_bytes(window.grab_window_as_png())
            return str(path), ""
        except Exception as error:  # noqa: BLE001
            return "", f"Capture unavailable: {error}"

    def completed_step(self, name, status="Passed", duration=0.0):
        path, warning = self.capture()
        self.emit(
            "step",
            title=name,
            status=status,
            duration=duration,
            screenshot=path,
            warning=warning,
        )

    def patch(self, target, name, replacement):
        self.restorations.append((target, name, getattr(target, name)))
        setattr(target, name, replacement)

    def pytest_configure(self, config):
        if config.option.collectonly:
            return
        import slint_testing
        import ui_reporting

        original_enter = slint_testing.Application.__enter__
        original_exit = slint_testing.Application.__exit__
        original_stage = ui_reporting.replay_stage
        plugin = self

        def enter(application):
            try:
                result = original_enter(application)
            except BaseException:
                process = getattr(application, "process", None)
                if process is not None:
                    process.terminate()
                    try:
                        process.wait(timeout=2)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
                application.test_server_socket.close()
                raise
            plugin.application = application
            plugin.completed_step("Launch editor")
            return result

        def leave(application, exc_type, exc_value, traceback):
            try:
                plugin.completed_step("Final state", "Failed" if exc_type else "Passed")
            finally:
                plugin.application = None
            return original_exit(application, exc_type, exc_value, traceback)

        @contextlib.contextmanager
        def stage(name):
            started = time.monotonic()
            plugin.emit("stage-start", title=name)
            try:
                with original_stage(name):
                    yield
            except BaseException:
                plugin.completed_step(name, "Failed", time.monotonic() - started)
                raise
            else:
                plugin.completed_step(name, "Passed", time.monotonic() - started)

        self.patch(slint_testing.Application, "__enter__", enter)
        self.patch(slint_testing.Application, "__exit__", leave)
        self.patch(ui_reporting, "replay_stage", stage)

    def pytest_unconfigure(self, config):
        for target, name, original in reversed(self.restorations):
            setattr(target, name, original)

    def pytest_collection_finish(self, session):
        for item in session.items:
            try:
                source = inspect.getsource(item.obj)
            except (OSError, TypeError):
                source = "Source unavailable."
            function, _, case = item.name.partition("[")
            self.emit(
                "collected",
                id=item.nodeid,
                title=function.removeprefix("test_").replace("_", " ").capitalize(),
                suite=Path(item.path)
                .stem.removeprefix("test_")
                .replace("_", " ")
                .title(),
                case=case.removesuffix("]"),
                source=source,
                path=f"{item.location[0]}:{item.location[1] + 1}",
            )

    def pytest_runtest_logstart(self, nodeid, location):
        self.nodeid = nodeid
        self.outcome = "Passed"
        self.duration = 0.0
        self.emit("test-start")

    def pytest_runtest_logreport(self, report):
        self.duration += report.duration
        if report.failed:
            self.outcome = "Failed"
        elif report.skipped and self.outcome != "Failed":
            self.outcome = "Skipped"
        self.emit(
            "report",
            phase=report.when,
            status=self.outcome,
            detail=report.longreprtext if report.longrepr else "",
            output="\n".join(f"{name}\n{content}" for name, content in report.sections),
        )

    def pytest_runtest_logfinish(self, nodeid, location):
        self.emit("test-end", status=self.outcome, duration=self.duration)
        self.nodeid = ""

    def pytest_collectreport(self, report):
        if report.failed:
            self.emit("error", detail=report.longreprtext)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", type=Path, required=True)
    parser.add_argument("--events", type=Path, required=True)
    parser.add_argument("--selectors", type=Path, required=True)
    parser.add_argument("--collect", action="store_true")
    args = parser.parse_args()
    sys.path.insert(0, str(args.suite / "tests"))
    plugin = StudioPlugin(args.events, args.events.parent)
    selectors = json.loads(args.selectors.read_text())
    options = ["-q", "-ra", "--color=no", "-p", "no:cacheprovider", "-o", "addopts="]
    if args.collect:
        options += ["--collect-only"]
    else:
        options += ["--basetemp", str(args.events.parent / "pytest")]
    return pytest.main(options + selectors, plugins=[plugin])


if __name__ == "__main__":
    raise SystemExit(main())
