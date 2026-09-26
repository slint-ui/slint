# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Own a subprocess tree until it exits or the parent's pipe closes."""

import os
import signal
import subprocess
import sys
import threading
import time


def supervise(command):
    stopped = threading.Event()
    signal.signal(signal.SIGTERM, lambda *_: stopped.set())
    signal.signal(signal.SIGINT, lambda *_: stopped.set())
    threading.Thread(target=lambda: (os.read(0, 1), stopped.set()), daemon=True).start()
    child = subprocess.Popen(
        command, stdin=subprocess.DEVNULL, start_new_session=os.name != "nt"
    )
    while child.poll() is None and not stopped.wait(0.03):
        pass
    code = child.poll()
    if os.name == "nt":
        subprocess.run(
            ["taskkill", "/PID", str(child.pid), "/T", "/F"],
            capture_output=True,
            check=False,
        )
    else:
        try:
            os.killpg(child.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            child.poll()
            try:
                os.killpg(child.pid, 0)
            except ProcessLookupError:
                break
            time.sleep(0.03)
        else:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    child.wait()
    return code if code is not None else 130


if __name__ == "__main__":
    raise SystemExit(supervise(sys.argv[1:]))
