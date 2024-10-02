# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

from slint import slint as native
import threading
from datetime import timedelta


def test_threads():
    global was_here
    was_here = False

    def invoked_from_event_loop():
        global was_here
        was_here = True
        native.quit_event_loop()

    def quit():
        native.invoke_from_event_loop(invoked_from_event_loop)

    thr = threading.Thread(target=quit)
    native.Timer.single_shot(timedelta(milliseconds=10), lambda: thr.start())
    fallback_timer = native.Timer()
    fallback_timer.start(native.TimerMode.Repeated, timedelta(
        milliseconds=100), native.quit_event_loop)
    native.run_event_loop()
    thr.join()
    fallback_timer.stop()
    assert was_here == True
