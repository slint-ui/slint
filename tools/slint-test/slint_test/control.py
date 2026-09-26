# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from __future__ import annotations

import contextlib
import time
from collections.abc import Iterator
from contextvars import ContextVar
from typing import Any, Protocol


class Controller(Protocol):
    def before(self, event: dict[str, Any]) -> None: ...
    def failed(self, event: dict[str, Any], error: BaseException) -> None: ...
    def after(self, event: dict[str, Any]) -> None: ...


_controller: ContextVar[Controller | None] = ContextVar(
    "slint_test_controller", default=None
)
_paused: ContextVar[float] = ContextVar("slint_test_paused", default=0)


def active_time() -> float:
    """Return elapsed time excluding debugger pauses in this test context."""
    return time.monotonic() - _paused.get()


@contextlib.contextmanager
def suspended_clock() -> Iterator[None]:
    started = time.monotonic()
    try:
        yield
    finally:
        _paused.set(_paused.get() + time.monotonic() - started)


@contextlib.contextmanager
def debugging(controller: Controller) -> Iterator[None]:
    """Install action control separately from the best-effort reporting observer."""
    token = _controller.set(controller)
    try:
        yield
    finally:
        _controller.reset(token)


def controller() -> Controller | None:
    return _controller.get()
