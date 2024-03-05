# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from slint import slint as native
import threading
from datetime import timedelta

was_here = False


def test_threads() -> None:
    global was_here
    was_here = False

    def invoked_from_event_loop() -> None:
        global was_here
        was_here = True
        native.quit_event_loop()

    def quit() -> None:
        native.invoke_from_event_loop(invoked_from_event_loop)

    thr = threading.Thread(target=quit)
    native.Timer.single_shot(timedelta(milliseconds=10), lambda: thr.start())
    fallback_timer = native.Timer()
    fallback_timer.start(
        native.TimerMode.Repeated, timedelta(milliseconds=100), native.quit_event_loop
    )
    native.run_event_loop()
    thr.join()
    fallback_timer.stop()
    assert was_here
