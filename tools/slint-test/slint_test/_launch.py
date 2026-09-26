# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Own a subprocess tree until its controlling process stops."""

import os
import signal
import subprocess
import sys
import threading
import time


def signal_group(pid: int, signal_number: int) -> None:
    try:
        os.killpg(pid, signal_number)
    except ProcessLookupError:
        pass


def terminate_tree(child: subprocess.Popen) -> None:
    if os.name == "nt":
        subprocess.run(
            ["taskkill", "/PID", str(child.pid), "/T", "/F"],
            capture_output=True,
            check=False,
        )
        child.wait()
        return
    signal_group(child.pid, signal.SIGTERM)
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        child.poll()
        try:
            os.killpg(child.pid, 0)
        except ProcessLookupError:
            break
        time.sleep(0.02)
    else:
        signal_group(child.pid, signal.SIGKILL)
    child.wait()


def supervise(command, should_stop, stopped_code: int) -> int:
    stopping = threading.Event()

    def stop(*_):
        stopping.set()

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    child = subprocess.Popen(
        command, stdin=subprocess.DEVNULL, start_new_session=os.name != "nt"
    )
    while child.poll() is None and not stopping.is_set() and not should_stop():
        time.sleep(0.02)
    code = child.poll()
    terminate_tree(child)
    return code if code is not None else stopped_code


def main() -> int:
    mode = sys.argv[1]
    if mode == "--parent":
        parent = int(sys.argv[2])
        if os.getppid() != parent:
            return 1
        return supervise(sys.argv[3:], lambda: os.getppid() != parent, 0)
    if mode == "--pipe":
        closed = threading.Event()
        threading.Thread(
            target=lambda: (os.read(0, 1), closed.set()), daemon=True
        ).start()
        return supervise(sys.argv[2:], closed.is_set, 130)
    raise SystemExit("Expected --parent PID or --pipe")


if __name__ == "__main__":
    raise SystemExit(main())
