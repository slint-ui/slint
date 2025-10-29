# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import asyncio
from datetime import timedelta
import sys

import pytest

import slint
import slint.api
from slint import core


def test_sysexit_exception() -> None:
    def call_sys_exit() -> None:
        sys.exit(42)

    slint.Timer.single_shot(timedelta(milliseconds=100), call_sys_exit)
    with pytest.raises(SystemExit) as exc_info:
        core.run_event_loop()
    assert (
        "unexpected failure running python singleshot timer callback"
        in exc_info.value.__notes__
    )


def test_quit_event_loop_calls_core(monkeypatch: pytest.MonkeyPatch) -> None:
    toggle = False

    def fake_quit() -> None:
        nonlocal toggle
        toggle = True

    monkeypatch.setattr(core, "quit_event_loop", fake_quit)
    monkeypatch.setattr(core, "invoke_from_event_loop", lambda cb: cb())

    slint.api.quit_event = asyncio.Event()

    slint.quit_event_loop()

    assert toggle is True
    assert slint.api.quit_event.is_set()


def test_quit_event_loop_falls_back_when_invoke_fails(monkeypatch: pytest.MonkeyPatch) -> None:
    toggle = False

    def fake_quit() -> None:
        nonlocal toggle
        toggle = True

    def fake_invoke(cb):
        raise RuntimeError("invoke unavailable")

    monkeypatch.setattr(core, "quit_event_loop", fake_quit)
    monkeypatch.setattr(core, "invoke_from_event_loop", fake_invoke)

    slint.api.quit_event = asyncio.Event()

    slint.quit_event_loop()

    assert toggle is True
