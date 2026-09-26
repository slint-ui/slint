# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import contextlib
import inspect
import linecache
import logging
import time
import uuid
from collections.abc import Callable, Iterator
from contextvars import ContextVar
from pathlib import Path
from typing import Any

from .control import active_time, controller

Observer = Callable[[dict[str, Any]], None]
_observer: ContextVar[Observer | None] = ContextVar("slint_test_observer", default=None)
_sources: ContextVar[tuple[str, ...]] = ContextVar("slint_test_sources", default=())
_parent: ContextVar[str | None] = ContextVar("slint_test_parent", default=None)


@contextlib.contextmanager
def reporting(observer: Observer) -> Iterator[None]:
    token = _observer.set(observer)
    try:
        yield
    finally:
        _observer.reset(token)


def emit(event: dict[str, Any]) -> None:
    observer = _observer.get()
    if observer is not None:
        try:
            observer({"version": 1, "timestamp": time.time(), **event})
        except Exception:
            logging.getLogger(__name__).exception("Test reporting observer failed")


@contextlib.contextmanager
def step(
    title: str, *, layer: str = "test", **arguments: Any
) -> Iterator[dict[str, Any]]:
    identifier = uuid.uuid4().hex
    source = {}
    frame = inspect.currentframe()
    try:
        while frame is not None:
            filename = frame.f_code.co_filename
            if "slint_test/" not in filename and not filename.endswith("contextlib.py"):
                source = {
                    "file": filename,
                    "line": frame.f_lineno,
                    "text": linecache.getline(filename, frame.f_lineno).strip(),
                }
                break
            frame = frame.f_back
    finally:
        del frame
    event = {
        "action_id": identifier,
        "parent_id": _parent.get(),
        "title": title,
        "layer": layer,
        "arguments": arguments,
        "source": source,
    }
    emit(dict(event, kind="action-start", timestamp=time.time()))
    token = _parent.set(identifier)
    started = active_time()
    detail: dict[str, Any] = {}
    control = controller()
    try:
        if control is not None:
            control.before(event)
        yield detail
    except BaseException as error:
        diagnostic = getattr(error, "diagnostic", None)
        if isinstance(diagnostic, dict):
            event = {**event, "diagnostic": diagnostic}
        if control is not None and not isinstance(
            error, (KeyboardInterrupt, InterruptedError)
        ):
            control.failed(event, error)
        emit(
            dict(
                event,
                kind="action-end",
                status="Cancelled"
                if isinstance(error, (KeyboardInterrupt, InterruptedError))
                else "Failed",
                duration=active_time() - started,
                **{**detail, "detail": str(error)},
            )
        )
        raise
    else:
        emit(
            dict(
                event,
                kind="action-end",
                status="Passed",
                duration=active_time() - started,
                **detail,
            )
        )
    finally:
        try:
            if control is not None:
                control.after(event)
        finally:
            _parent.reset(token)


@contextlib.contextmanager
def inspection_sources(*paths: Path) -> Iterator[None]:
    """Supply optional application sources to an inspector reporting observer."""
    token = _sources.set(tuple(str(path.resolve()) for path in paths))
    emit({"kind": "inspection-context", "sources": list(_sources.get())})
    try:
        yield
    finally:
        _sources.reset(token)
        emit({"kind": "inspection-context", "sources": list(_sources.get())})
