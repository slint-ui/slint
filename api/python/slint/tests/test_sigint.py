# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import signal
import threading
import time

import pytest

import slint


def test_run_event_loop_handles_sigint():
    def trigger_sigint() -> None:
        # Allow the event loop to start before raising the signal.
        time.sleep(0.1)
        signal.raise_signal(signal.SIGINT)

    sender = threading.Thread(target=trigger_sigint)
    sender.start()
    try:
        with pytest.raises(KeyboardInterrupt):
            slint.run_event_loop()
    finally:
        sender.join()
