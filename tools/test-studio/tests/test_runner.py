# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
import os
import sys
import time
from pathlib import Path
from types import SimpleNamespace

import pytest
from bridge import StudioPlugin
from runner import TestProcess as Process


@pytest.fixture
def sample_repo(tmp_path):
    suite = tmp_path / "tools/editor/ui-tests"
    tests = suite / "tests"
    tests.mkdir(parents=True)
    (tests / "slint_testing.py").write_text(
        "class Application:\n def __enter__(self): pass\n def __exit__(self, *args): pass\n"
    )
    (tests / "ui_reporting.py").write_text(
        "from contextlib import nullcontext\nreplay_stage = nullcontext\n"
    )
    (tests / "test_cases.py").write_text("""import pytest
import subprocess
import sys
import time
from pathlib import Path

@pytest.mark.parametrize("value", [1, 2])
def test_pass(value):
    assert value > 0

def test_fail():
    assert 1 == 2

@pytest.mark.skip(reason="sample skip")
def test_skip():
    pass

@pytest.fixture
def broken():
    raise RuntimeError("setup failed")

def test_setup_failure(broken):
    pass

def test_slow():
    child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
    Path("child.pid").write_text(str(child.pid))
    time.sleep(60)
""")
    return tmp_path


def wait_for(process, predicate, timeout=15):
    deadline = time.monotonic() + timeout
    collected = []
    while time.monotonic() < deadline:
        collected.extend(process.poll())
        if predicate(collected):
            return collected
        time.sleep(0.02)
    raise AssertionError(process.log_path.read_text())


def test_collection_preserves_parameterized_ids_and_source(sample_repo):
    process = Process(
        sample_repo,
        Path(sys.executable),
        Path("unused"),
        ["tests/test_cases.py"],
        collect=True,
    )
    try:
        events = wait_for(
            process, lambda events: any(e["kind"] == "finished" for e in events)
        )
        rows = [e for e in events if e["kind"] == "collected"]
        assert len(rows) == 6
        assert rows[0]["id"] == "tests/test_cases.py::test_pass[1]"
        assert "assert value > 0" in rows[0]["source"]
        assert events[-1]["code"] == 0
        assert not any(e["kind"] == "test-start" for e in events)
    finally:
        process.close()


def test_reports_pass_fail_skip_and_setup_failure(sample_repo):
    selectors = [
        f"tests/test_cases.py::{name}"
        for name in ("test_pass[1]", "test_fail", "test_skip", "test_setup_failure")
    ]
    process = Process(sample_repo, Path(sys.executable), Path("unused"), selectors)
    try:
        events = wait_for(
            process, lambda events: any(e["kind"] == "finished" for e in events)
        )
        assert [e["status"] for e in events if e["kind"] == "test-end"] == [
            "Passed",
            "Failed",
            "Skipped",
            "Failed",
        ]
        assert any("setup failed" in e.get("detail", "") for e in events)
        assert events[-1]["code"] == 1
    finally:
        process.close()


@pytest.mark.skipif(os.name == "nt", reason="Checks POSIX process-group cleanup")
def test_stop_terminates_the_test_and_its_child(sample_repo):
    process = Process(
        sample_repo,
        Path(sys.executable),
        Path("unused"),
        ["tests/test_cases.py::test_slow"],
    )
    pid_file = sample_repo / "tools/editor/ui-tests/child.pid"
    try:
        wait_for(process, lambda events: pid_file.exists())
        child = int(pid_file.read_text())
        process.stop()
        events = wait_for(
            process, lambda events: any(e["kind"] == "finished" for e in events)
        )
        assert events[-1]["cancelled"] is True
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            try:
                os.kill(child, 0)
            except ProcessLookupError:
                break
            time.sleep(0.02)
        else:
            pytest.fail("The child process survived Stop")
    finally:
        process.close()


def test_event_reader_waits_for_a_complete_line(tmp_path):
    process = Process.__new__(Process)
    process.events_path = tmp_path / "events.jsonl"
    process.events_path.write_text('{"kind": "test-')
    process.offset = 0
    process.finished = False
    process.process = SimpleNamespace(poll=lambda: None)
    assert process.poll() == []
    with process.events_path.open("a") as stream:
        stream.write('start"}\n')
    assert process.poll() == [{"kind": "test-start"}]
    assert process.poll() == []


def test_capture_error_does_not_change_the_test_outcome(tmp_path):
    plugin = StudioPlugin(tmp_path / "events.jsonl", tmp_path)

    def unavailable():
        raise RuntimeError("window closed")

    plugin.application = SimpleNamespace(
        first_window=SimpleNamespace(grab_window_as_png=unavailable)
    )
    plugin.completed_step("Final state")
    event = json.loads(plugin.events.read_text())
    assert event["status"] == "Passed"
    assert event["screenshot"] == ""
    assert "window closed" in event["warning"]
