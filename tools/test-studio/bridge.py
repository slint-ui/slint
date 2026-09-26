# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Run inside the target suite's Python environment and stream pytest events."""

import argparse
import importlib
import inspect
import json
import os
import sys
import time
from pathlib import Path
from types import ModuleType

import pytest

from debugger import Debugger
from events import Writer


class StudioPlugin:
    def __init__(self, events, artifacts):
        self.writer = Writer(events)
        self.events, self.artifacts = events, artifacts
        self.application = None
        self.nodeid = ""
        self.inspection_sources = []
        self.action_count = 0
        self.sequence = 0
        self.outcome = "Passed"
        self.duration = 0.0
        self.observer_token = None
        self.reporting: ModuleType | None = None
        self.action_reporting = None
        self.action_control = None
        self.debugger = (
            Debugger(self) if os.environ.get("SLINT_STUDIO_DEBUG") == "1" else None
        )
        self.sections = set()
        self.strict_xpass = False
        self.application_started = False
        self.legacy_actions = []
        self.legacy_count = 0
        self.capture_policy = os.environ.get("SLINT_STUDIO_CAPTURES", "boundaries")
        if self.capture_policy not in {"boundaries", "failures", "none"}:
            self.capture_policy = "boundaries"

    def emit(self, kind, **data):
        return self.writer.emit(kind, nodeid=self.nodeid, **data)

    def capture(self, status, *, boundary=False):
        started = time.monotonic()
        path, warning = "", ""
        if self.application is not None and (
            (boundary and self.capture_policy == "boundaries")
            or (
                self.capture_policy != "none"
                and status in {"Failed", "Error", "Crashed"}
            )
        ):
            try:
                self.sequence += 1
                path = f"capture-{self.sequence:04d}.png"
                (self.artifacts / path).write_bytes(
                    self.application.first_window.grab_window_as_png()
                )
            except Exception as error:  # noqa: BLE001
                path, warning = "", f"Capture unavailable: {error}"
        return path, warning, time.monotonic() - started

    def start_legacy_action(self, name):
        self.legacy_count += 1
        action_id = f"legacy-{self.legacy_count}"
        self.emit(
            "action-start",
            action_id=action_id,
            parent_id=None,
            title=name,
            layer="stage",
            arguments={"trace_coverage": "group-only legacy helper"},
            source={},
        )
        return action_id

    def end_legacy_action(self, action_id, status="Passed", duration=0.0):
        path, warning, capture_duration = self.capture(status, boundary=True)
        self.emit(
            "action-end",
            action_id=action_id,
            status=status,
            duration=duration,
            screenshot=path,
            warning=warning,
            capture_duration=capture_duration,
        )

    def completed_action(self, name, status="Passed", duration=0.0):
        self.end_legacy_action(self.start_legacy_action(name), status, duration)

    def observe(self, kind, **data):
        if kind == "application-ready":
            self.application_started = True
            self.application = data["application"]
            self.completed_action("Launch application")
        elif kind == "application-closing":
            if data.get("returncode") not in (None, 0):
                self.outcome = "Crashed"
                self.emit("application-crash", returncode=data["returncode"])
            self.completed_action(
                "Final state",
                "Failed"
                if data.get("failed") and self.outcome == "Passed"
                else self.outcome,
            )
            self.application = None
        elif kind == "stage-start":
            self.legacy_actions.append(
                (data["title"], self.start_legacy_action(data["title"]))
            )
        elif kind == "stage-end":
            status = "Failed" if data["failed"] else "Passed"
            if self.legacy_actions and self.legacy_actions[-1][0] == data["title"]:
                _, action_id = self.legacy_actions.pop()
                self.end_legacy_action(action_id, status, data["duration"])
            else:
                self.completed_action(data["title"], status, data["duration"])

    def observe_action(self, event):
        kind = event["kind"]
        if kind == "inspection-context":
            self.inspection_sources = event["sources"]
            return
        if kind == "action-start":
            self.action_count += 1
        if kind == "application-ready":
            self.observe(kind, application=event["application"])
            return
        if kind == "application-closing":
            self.observe(
                kind,
                failed=event.get("failed", False),
                returncode=event.get("returncode"),
            )
            return
        data = {key: value for key, value in event.items() if key != "kind"}
        if kind == "application-exit":
            return
        if kind == "action-end":
            path, warning, capture_duration = self.capture(event["status"])
            data.update(
                screenshot=path,
                warning=warning,
                capture_duration=capture_duration,
            )
        self.emit(kind, **data)

    def pytest_configure(self, config):
        if config.option.collectonly:
            return
        generic = Path(__file__).resolve().parents[1] / "slint-test"
        if generic.is_dir():
            sys.path.insert(0, str(generic))
            try:
                action_api = importlib.import_module("slint_test")
            except ImportError as error:
                self.emit("warning", detail=f"Action reporting unavailable: {error}")
            else:
                self.action_reporting = action_api.reporting(self.observe_action)
                self.action_reporting.__enter__()
                if self.debugger is not None:
                    control = importlib.import_module("slint_test.control")
                    self.action_control = control.debugging(self.debugger)
                    self.action_control.__enter__()
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
        if self.action_control is not None:
            self.action_control.__exit__(None, None, None)
        if self.action_reporting is not None:
            self.action_reporting.__exit__(None, None, None)
        if self.observer_token is not None and self.reporting is not None:
            self.reporting.reset_observer(self.observer_token)

    def pytest_itemcollected(self, item):
        if not item.path.is_relative_to(item.config.rootpath):
            # Pytest can omit the filename for items outside rootdir.
            # Absolute IDs remain executable from the suite working directory.
            for node in item.listchain():
                if isinstance(node, (pytest.Class, pytest.Item)):
                    _, separator, suffix = node.nodeid.partition("::")
                    if separator:
                        node._nodeid = f"{item.path.as_posix()}::{suffix}"

    def pytest_collection_finish(self, session):
        for item in session.items:
            try:
                source = inspect.getsource(item.obj)
            except (OSError, TypeError, AttributeError):
                source = "Source unavailable."
            function, _, case = item.name.partition("[")
            groups = []
            path = item.path
            if path.is_relative_to(session.config.rootpath):
                path = path.relative_to(session.config.rootpath)
            for parent in reversed(path.parents):
                if parent.name:
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
        self.action_count = 0
        self.legacy_actions.clear()
        self.legacy_count = 0
        if self.debugger is not None:
            self.debugger.failure = None
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
        if self.debugger is not None and not self.action_count:
            self.emit(
                "warning",
                detail="No instrumented actions in this test; debugger could not pause it.",
            )
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
        f"--rootdir={args.suite}",
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
