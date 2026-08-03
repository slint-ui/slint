# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Keep the native event queue from leaking between tests.

The testing backend keeps one event queue for the whole process, and
`run_event_loop()` returns as soon as it pops a quit request, so whatever sits
behind that request stays queued for the next run.
A test that leaves a stray quit behind therefore cuts the next test's loop short,
and that test in turn strands its timers for a third one to trip over.
"""

import os

import pytest

from slint import slint as native

# A drain needs one run per stale quit, so a queue that outlasts this many is
# refilling itself faster than we can empty it.
_MAX_RUNS = 10


def drain_native_event_queue() -> None:
    """Empties the native event queue.

    Queues a sentinel behind whatever is already there and runs the loop until
    the sentinel fires: each run dispatches the events ahead of it and returns,
    either on the sentinel's own quit or on a stale one queued before it.
    Twice, because an event can queue a quit of its own once it runs.
    """
    if os.environ.get("SLINT_BACKEND") != "testing":
        # Only the testing backend keeps events queued from before the loop
        # started. Elsewhere the sentinel never fires and this would hang.
        return

    for _ in range(2):
        fired = False

        def sentinel() -> None:
            nonlocal fired
            fired = True
            native.quit_event_loop()

        native.invoke_from_event_loop(sentinel)

        runs = 0
        while not fired:
            native.run_event_loop()
            runs += 1
            if runs > _MAX_RUNS:
                raise RuntimeError(
                    "the native event queue keeps producing quit requests, "
                    "the sentinel never got to run"
                )


@pytest.fixture(autouse=True)
def drained_event_queue() -> None:
    """Starts every test with an empty native event queue."""
    drain_native_event_queue()
