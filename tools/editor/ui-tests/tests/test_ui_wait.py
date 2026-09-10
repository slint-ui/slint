# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from types import SimpleNamespace

import pytest
import ui_driver


@pytest.fixture
def clock(monkeypatch):
    now = 0.0
    sleeps = []

    def sleep(delay):
        nonlocal now
        sleeps.append(delay)
        now += delay

    monkeypatch.setattr(
        ui_driver, "time", SimpleNamespace(monotonic=lambda: now, sleep=sleep)
    )
    return sleeps


def test_ui_wait_rechecks_missing_and_stale_states(clock):
    states = iter([None, 32, 32, 62])
    assert (
        ui_driver.wait_for_ui(
            lambda: next(states), lambda value: value == 62, description="rotation 62"
        )
        == 62
    )
    assert clock == [0.02, 0.02, 0.02]


def test_ui_wait_accepts_a_matching_false_value_without_sleeping(clock):
    assert (
        ui_driver.wait_for_ui(
            lambda: False, lambda value: value is False, description="hidden preview"
        )
        is False
    )
    assert clock == []


def test_ui_wait_reports_last_state_at_the_deadline(clock):
    with pytest.raises(AssertionError, match="rotation 62; last UI state: 32"):
        ui_driver.wait_for_ui(
            lambda: 32,
            lambda value: value == 62,
            description="rotation 62",
            timeout=0.05,
        )
    assert sum(clock) == pytest.approx(0.05)
    assert clock[-1] == pytest.approx(0.01)


def test_ui_wait_does_not_hide_reader_errors(clock):
    def broken_reader():
        raise RuntimeError("UI connection lost")

    with pytest.raises(RuntimeError, match="UI connection lost"):
        ui_driver.wait_for_ui(broken_reader, lambda _: True, description="rectangle")
    assert clock == []
