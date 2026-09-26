# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import os
import signal
import subprocess
import sys
import time
from pathlib import Path

import pytest

HELPER = Path(__file__).parents[1] / "slint_test/_launch.py"


def wait_for(read, timeout=6):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if read():
            return
        time.sleep(0.02)
    raise AssertionError("Process cleanup deadline exceeded")


def running(pid):
    result = subprocess.run(
        ["ps", "-o", "stat=", "-p", str(pid)],
        check=False,
        capture_output=True,
        text=True,
    )
    return bool(result.stdout.strip()) and not result.stdout.lstrip().startswith("Z")


@pytest.mark.skipif(os.name == "nt", reason="POSIX process ownership")
@pytest.mark.parametrize("termination", ["parent", "child"])
def test_monitor_cleans_stubborn_descendants(tmp_path, termination):
    marker = tmp_path / "descendant"
    child_file = tmp_path / "child.py"
    child_file.write_text("""import os, signal, subprocess, sys, time
from pathlib import Path
p = subprocess.Popen([sys.executable, '-c', 'import signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(60)'])
Path(sys.argv[1]).write_text(str(p.pid))
if sys.argv[2] == 'child':
    time.sleep(0.2)
else:
    time.sleep(60)
""")
    parent_code = "import os,subprocess,sys,time; p=subprocess.Popen([sys.executable,sys.argv[1],'--parent',str(os.getpid()),sys.executable,*sys.argv[2:]]); p.wait()"
    parent = subprocess.Popen(
        [
            sys.executable,
            "-c",
            parent_code,
            str(HELPER),
            str(child_file),
            str(marker),
            termination,
        ]
    )
    descendant = None
    try:
        wait_for(marker.exists)
        descendant = int(marker.read_text())
        if termination == "parent":
            parent.kill()
        wait_for(lambda: not running(descendant))
    finally:
        if parent.poll() is None:
            parent.terminate()
        parent.wait(timeout=5)
        if descendant is not None and running(descendant):
            os.kill(descendant, signal.SIGKILL)


def test_parent_already_dead_does_not_launch(tmp_path):
    marker = tmp_path / "should-not-exist"
    result = subprocess.run(
        [
            sys.executable,
            str(HELPER),
            "--parent",
            "-1",
            sys.executable,
            "-c",
            f"open({str(marker)!r}, 'w').close()",
        ],
        timeout=3,
        check=False,
    )
    assert result.returncode == 1
    assert not marker.exists()
