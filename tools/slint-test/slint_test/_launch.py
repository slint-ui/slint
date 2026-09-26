# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Own a child process group until either the test parent or application exits."""

import os
import signal
import subprocess
import sys
import time


def signal_group(pid: int, sig: int) -> None:
    try:
        os.killpg(pid, sig)
    except ProcessLookupError:
        pass


def main() -> int:
    parent = int(sys.argv[1])
    stopping = False

    def stop(*_):
        nonlocal stopping
        stopping = True

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    if os.getppid() != parent:
        return 1
    child = subprocess.Popen(sys.argv[2:], start_new_session=True)
    while not stopping and child.poll() is None and os.getppid() == parent:
        time.sleep(0.02)
    code = child.poll()
    signal_group(child.pid, signal.SIGTERM)
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        child.poll()
        time.sleep(0.02)
    # Descendants can outlive the immediate child and ignore SIGTERM.
    signal_group(child.pid, signal.SIGKILL)
    child.wait()
    return code or 0


if __name__ == "__main__":
    raise SystemExit(main())
