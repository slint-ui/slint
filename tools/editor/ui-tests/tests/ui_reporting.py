# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import contextlib
import json
import logging
import time
from collections.abc import Callable, Iterator
from contextvars import ContextVar
from dataclasses import dataclass
from pathlib import Path
from typing import TypeVar

import slint_testing

T = TypeVar("T")


@dataclass
class TestReport:
    name: str
    artifacts: Path
    pause_seconds: float
    completed_stages: int = 0
    screenshots: int = 0


current_report: ContextVar[TestReport | None] = ContextVar(
    "current_report", default=None
)


@contextlib.contextmanager
def replay_stage(name: str) -> Iterator[None]:
    report = current_report.get()
    try:
        yield
    except Exception as error:
        error.add_note(f"Stage: {name}")
        raise
    else:
        if report is not None:
            report.completed_stages += 1
            if report.pause_seconds:
                print(f"{report.name}: {name}", flush=True)
                time.sleep(report.pause_seconds)


def capture_failure(application: slint_testing.Application, error: Exception) -> None:
    report = current_report.get()
    if report is None:
        return
    try:
        report.artifacts.mkdir(parents=True, exist_ok=True)
        report.screenshots += 1
        artifact = report.artifacts / f"failure-{report.screenshots}"
        failures: dict[str, str] = {}

        def capture(name: str, action: Callable[[], T]) -> T | None:
            try:
                return action()
            except Exception as diagnostic_error:
                logging.getLogger(__name__).exception(
                    "Failure diagnostic unavailable: %s", name
                )
                failures[name] = str(diagnostic_error)
                return None

        state = {"test": report.name, "errors": failures}
        window = capture("window", lambda: application.first_window)
        if window is not None:
            screenshot = capture("screenshot", window.grab_window_as_png)
            if screenshot is not None:
                image = artifact.with_suffix(".png")
                capture("screenshot file", lambda: image.write_bytes(screenshot))
                if "screenshot file" not in failures:
                    error.add_note(
                        f"Failure screenshot (pytest temporary directory): {image}"
                    )

            def describe(element: slint_testing.Element) -> dict:
                return {
                    "label": capture("label", lambda: element.accessible_label),
                    "value": capture("value", lambda: element.accessible_value),
                    "rect": capture("rect", lambda: list(element.absolute_rect)),
                }

            state["selection"] = capture(
                "selection",
                lambda: [
                    describe(element)
                    for element in window.query_descendants()
                    .match_accessible_role(slint_testing.AccessibleRole.Region)
                    .find_all()
                    if element.accessible_label.startswith("Selected ")
                ],
            )
            state["inline_text_editor"] = capture(
                "inline_text_editor",
                lambda: [
                    describe(element)
                    for element in window.query_descendants()
                    .match_accessible_label("Inline text editor")
                    .find_all()
                ],
            )
        artifact = artifact.with_suffix(".json")
        artifact.write_text(json.dumps(state, indent=2) + "\n")
        error.add_note(f"Failure UI state: {artifact}")
    except Exception:
        logging.getLogger(__name__).exception("Failure diagnostics unavailable")
