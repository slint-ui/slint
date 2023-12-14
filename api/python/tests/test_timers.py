# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import pytest
import slint
from slint import ValueType
from datetime import timedelta

def test_timer():
    global counter
    counter = 0
    def quit_after_two_invocations():
        global counter
        counter = counter + 1
        if counter >= 2:
            slint.quit_event_loop()

    test_timer = slint.Timer()        
    test_timer.start(slint.TimerMode.Repeated, timedelta(milliseconds=100), quit_after_two_invocations)
    slint.run_event_loop()
    test_timer.stop()
    assert(counter == 2)

def test_single_shot():
    slint.Timer.single_shot(timedelta(milliseconds=100), slint.quit_event_loop)
    slint.run_event_loop()
