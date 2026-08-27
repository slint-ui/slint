# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import sys
from datetime import timedelta

import pytest

import slint
from slint import slint as native


def test_sysexit_exception() -> None:
    def call_sys_exit() -> None:
        sys.exit(42)

    slint.Timer.single_shot(timedelta(milliseconds=100), call_sys_exit)
    with pytest.raises(SystemExit) as exc_info:
        native.run_event_loop()
    assert (
        "unexpected failure running python singleshot timer callback"
        in exc_info.value.__notes__
    )
