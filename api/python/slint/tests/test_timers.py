# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import gc
import weakref
from datetime import timedelta

from slint import slint as native

counter: int


def test_timer() -> None:
    global counter
    counter = 0

    def quit_after_two_invocations() -> None:
        global counter
        counter = min(counter + 1, 2)
        if counter == 2:
            native.quit_event_loop()

    test_timer = native.Timer()
    test_timer.start(
        native.TimerMode.Repeated,
        timedelta(milliseconds=100),
        quit_after_two_invocations,
    )
    native.run_event_loop()
    test_timer.stop()
    assert counter == 2


def test_single_shot() -> None:
    native.Timer.single_shot(timedelta(milliseconds=100), native.quit_event_loop)
    native.run_event_loop()


def test_callback_cycle_is_collectable() -> None:
    # The pattern from Timer's own documentation - an object that owns the timer
    # and hands it one of its own bound methods - is a reference cycle through
    # the Rust closure. Without __traverse__/__clear__ it is never collected.
    class Owner:
        def __init__(self) -> None:
            self.timer = native.Timer()
            self.timer.start(
                native.TimerMode.Repeated, timedelta(seconds=1), self.on_tick
            )

        def on_tick(self) -> None:
            pass

    owner = Owner()
    weak_owner = weakref.ref(owner)

    # Reported via __traverse__; without it the type isn't GC-tracked at all and
    # the assertion below fails for a reason that is harder to read off.
    assert gc.is_tracked(owner.timer)

    del owner
    gc.collect()

    assert weak_owner() is None
