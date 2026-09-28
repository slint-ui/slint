# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import time
from datetime import timedelta

from conftest import drain_native_event_queue

from slint import slint as native


def test_drain_swallows_a_stray_quit() -> None:
    # A test can end with a quit still queued, for example when two quits raced
    # to end the same loop and only one of them was consumed.
    native.quit_event_loop()
    drain_native_event_queue()

    # Without the drain the stray quit ends this loop right away, leaving the
    # timer armed for whichever loop runs next.
    native.Timer.single_shot(timedelta(milliseconds=50), native.quit_event_loop)
    start = time.monotonic()
    native.run_event_loop()

    assert time.monotonic() - start >= 0.04
