# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from slint import core
from datetime import timedelta

counter: int


def test_timer() -> None:
    global counter
    counter = 0

    def quit_after_two_invocations() -> None:
        global counter
        counter = min(counter + 1, 2)
        if counter == 2:
            core.quit_event_loop()

    test_timer = core.Timer()
    test_timer.start(
        core.TimerMode.Repeated,
        timedelta(milliseconds=100),
        quit_after_two_invocations,
    )
    core.run_event_loop()
    test_timer.stop()
    assert counter == 2


def test_single_shot() -> None:
    core.Timer.single_shot(timedelta(milliseconds=100), core.quit_event_loop)
    core.run_event_loop()
