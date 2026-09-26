# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Run inside the target suite's Python environment and stream pytest events."""

import argparse
import importlib
import inspect
import json
import sys
import time
from pathlib import Path
from types import ModuleType

import pytest

from events import Writer


class StudioPlugin:
    def __init__(self, events, artifacts):
        self.writer = Writer(events)
        self.events, self.artifacts = events, artifacts
        self.application = None
        self.nodeid = ""
        self.sequence = 0
        self.outcome = "Passed"
        self.duration = 0.0
        self.observer_token = None
        self.reporting: ModuleType | None = None
        self.sections = set()
        self.strict_xpass = False
        self.application_started = False

    def emit(self, kind, **data):
        return self.writer.emit(kind, nodeid=self.nodeid, **data)

    def completed_step(self, name, status="Passed", duration=0.0):
        started = time.monotonic()
        path, warning = "", ""
        if self.application is not None:
            try:
                self.sequence += 1
                path = f"capture-{self.sequence:04d}.png"
                (self.artifacts / path).write_bytes(
                    self.application.first_window.grab_window_as_png()
                )
            except Exception as error:  # noqa: BLE001
                path, warning = "", f"Capture unavailable: {error}"
        self.emit(
            "step",
            title=name,
            status=status,
            duration=duration,
            screenshot=path,
            warning=warning,
            capture_duration=time.monotonic() - started,
        )

    def observe(self, kind, **data):
        if kind == "application-ready":
            self.application_started = True
            self.application = data["application"]
            self.completed_step("Launch editor")
        elif kind == "application-closing":
            if data.get("returncode") not in (None, 0):
                self.outcome = "Crashed"
                self.emit("application-crash", returncode=data["returncode"])
            self.completed_step(
                "Final state",
                "Failed"
                if data.get("failed") and self.outcome == "Passed"
                else self.outcome,
            )
            self.application = None
        elif kind == "stage-start":
            self.emit(kind, **data)
        elif kind == "stage-end":
            self.completed_step(
                data["title"],
                "Failed" if data["failed"] else "Passed",
                data["duration"],
            )

    def pytest_configure(self, config):
        if config.option.collectonly:
            return
        try:
            ui_reporting = importlib.import_module("ui_reporting")

            if getattr(ui_reporting, "OBSERVER_VERSION", None) != 1:
                raise ImportError("Reporting observer version 1 is unavailable")
            self.reporting = ui_reporting
            self.observer_token = ui_reporting.install_observer(self.observe)
        except ImportError as error:
            self.emit(
                "warning",
                detail=f"Detailed captures unavailable for this harness: {error}",
            )

    def pytest_unconfigure(self, config):
        if self.observer_token is not None and self.reporting is not None:
            self.reporting.reset_observer(self.observer_token)

    def pytest_collection_finish(self, session):
        for item in session.items:
            try:
                source = inspect.getsource(item.obj)
            except (OSError, TypeError, AttributeError):
                source = "Source unavailable."
            function, _, case = item.name.partition("[")
            groups = []
            path = Path(item.location[0])
            for parent in reversed(path.parents):
                if str(parent) != ".":
                    groups.append({"id": f"dir:{parent}", "title": parent.name})
            groups.append({"id": f"file:{path}", "title": path.name})
            for parent in item.listchain():
                if isinstance(parent, pytest.Class):
                    groups.append(
                        {"id": f"class:{parent.nodeid}", "title": parent.name}
                    )
            if case:
                groups.append(
                    {
                        "id": f"function:{item.nodeid.split('[')[0]}",
                        "title": function.removeprefix("test_")
                        .replace("_", " ")
                        .capitalize(),
                    }
                )
            self.emit(
                "collected",
                id=item.nodeid,
                title=function.removeprefix("test_").replace("_", " ").capitalize(),
                suite=path.stem,
                case=case.removesuffix("]"),
                source=source,
                path=f"{path}:{item.location[1] + 1}",
                groups=groups,
                markers=sorted({marker.name for marker in item.iter_markers()}),
            )

    def pytest_runtest_logstart(self, nodeid, location):
        self.nodeid, self.outcome, self.duration = nodeid, "Passed", 0.0
        self.sections = set()
        self.strict_xpass = False
        self.application_started = False
        self.emit("test-start")

    @pytest.hookimpl(hookwrapper=True)
    def pytest_runtest_makereport(self, item, call):
        result = yield
        report = result.get_result()
        report.studio_connection_error = bool(
            call.excinfo and call.excinfo.type.__name__ == "ApplicationConnectionError"
        )

    def pytest_runtest_logreport(self, report):
        self.duration += report.duration
        detail = report.longreprtext if report.longrepr else ""
        if self.outcome != "Crashed":
            if report.failed:
                self.strict_xpass = report.when == "call" and detail.startswith(
                    "[XPASS(strict)]"
                )
                self.outcome = (
                    "Error"
                    if report.when != "call"
                    or getattr(report, "studio_connection_error", False)
                    else "Unexpected pass"
                    if self.strict_xpass
                    else "Failed"
                )
            elif self.outcome not in ("Error", "Failed"):
                if hasattr(report, "wasxfail"):
                    self.outcome = (
                        "Expected failure" if report.skipped else "Unexpected pass"
                    )
                elif report.skipped:
                    self.outcome = "Skipped"
        if getattr(report, "studio_connection_error", False):
            self.emit(
                "error",
                category="application-disconnect"
                if self.application_started
                else "application-launch",
                detail=detail,
            )
        sections = []
        for section in report.sections:
            if section not in self.sections:
                self.sections.add(section)
                sections.append("\n".join(section))
        self.emit(
            "report",
            phase=report.when,
            status=self.outcome,
            detail=detail,
            output="\n".join(sections),
        )

    def pytest_runtest_logfinish(self, nodeid, location):
        self.emit(
            "test-end",
            status=self.outcome,
            duration=self.duration,
            strict_xpass=self.strict_xpass,
        )
        self.nodeid = ""

    def pytest_collectreport(self, report):
        if report.failed:
            self.emit("error", category="collection", detail=report.longreprtext)


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
    options = [
        "-q",
        "-ra",
        "--color=no",
        "-p",
        "no:cacheprovider",
        "-p",
        "no:xdist",
        "-o",
        "addopts=",
    ]
    if args.collect:
        options += ["--collect-only"]
    else:
        options += ["--basetemp", str(args.events.parent / "pytest")]
    return pytest.main(options + selectors, plugins=[plugin])


if __name__ == "__main__":
    raise SystemExit(main())
