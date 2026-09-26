# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import contextlib
import logging
import time
from collections.abc import Callable, Iterator
from contextvars import ContextVar
from dataclasses import dataclass
from pathlib import Path

import slint_testing


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


OBSERVER_VERSION = 1
_observer: ContextVar[Callable[..., None] | None] = ContextVar(
    "ui_test_observer", default=None
)


def install_observer(observer: Callable[..., None]):
    return _observer.set(observer)


def reset_observer(token) -> None:
    _observer.reset(token)


def notify_observer(kind: str, **data) -> None:
    observer = _observer.get()
    if observer is not None:
        try:
            observer(kind, **data)
        except Exception:
            logging.getLogger(__name__).exception("UI test observer unavailable")


@contextlib.contextmanager
def replay_stage(name: str) -> Iterator[None]:
    report = current_report.get()
    started = time.monotonic()
    notify_observer("stage-start", title=name)
    try:
        yield
    except BaseException as error:
        notify_observer(
            "stage-end", title=name, failed=True, duration=time.monotonic() - started
        )
        error.add_note(f"Stage: {name}")
        raise
    else:
        notify_observer(
            "stage-end", title=name, failed=False, duration=time.monotonic() - started
        )
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
        window = application.first_window
        if window is None:
            return
        report.artifacts.mkdir(parents=True, exist_ok=True)
        report.screenshots += 1
        artifact = report.artifacts / f"failure-{report.screenshots}.png"
        artifact.write_bytes(window.grab_window_as_png())
        error.add_note(f"Failure screenshot (pytest temporary directory): {artifact}")
    except Exception:
        logging.getLogger(__name__).exception("Screenshot unavailable")
