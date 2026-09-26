# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
import sys
import threading
import time
from pathlib import Path
from types import SimpleNamespace

import pytest

from debugger import Debugger
from events import Writer
from state import RunState

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "slint-test"))


def controller(tmp_path, monkeypatch):
    # Load only control.py; Studio's interpreter does not need the test transport.
    import importlib.util

    spec = importlib.util.spec_from_file_location(
        "studio_test_clock",
        Path(__file__).resolve().parents[2] / "slint-test/slint_test/control.py",
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    monkeypatch.setattr("debugger.importlib.import_module", lambda _: module)
    events = []
    plugin = SimpleNamespace(
        artifacts=tmp_path,
        application=SimpleNamespace(process=SimpleNamespace(poll=lambda: None)),
        emit=lambda kind, **data: events.append(dict(kind=kind, **data)),
    )
    debug = Debugger(plugin)
    monkeypatch.setattr(debug, "inspect", lambda: events.append({"kind": "inspected"}))
    return debug, events


def wait_paused(events, count=1):
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        pauses = [e for e in events if e.get("paused")]
        if len(pauses) >= count:
            return pauses[-1]["pause_id"]
        time.sleep(0.01)
    pytest.fail("Debugger did not pause")


def test_step_over_nested_actions_and_reject_stale_commands(tmp_path, monkeypatch):
    debug, events = controller(tmp_path, monkeypatch)
    commands = Writer(tmp_path / "commands.jsonl")
    outer = {"action_id": "outer", "title": "Resize"}
    child = {"action_id": "child", "parent_id": "outer", "title": "Move"}

    def execute():
        debug.before(outer)
        debug.before(child)
        debug.after(child)
        debug.after(outer)
        debug.before({"action_id": "next", "title": "Verify"})

    thread = threading.Thread(target=execute, daemon=True)
    thread.start()
    pause = wait_paused(events)
    commands.emit("step", pause_id=pause)
    pause = wait_paused(events, 2)
    commands.emit("continue", pause_id=pause - 1)
    time.sleep(0.08)
    assert thread.is_alive()
    commands.emit("continue", pause_id=pause)
    thread.join(2)
    assert not thread.is_alive()
    assert not any(e["kind"] == "warning" for e in events)
    assert [e["action"]["title"] for e in events if e.get("paused")] == [
        "Resize",
        "Verify",
    ]


def test_failure_pauses_once_before_unwinding(tmp_path, monkeypatch):
    debug, events = controller(tmp_path, monkeypatch)
    error = AssertionError("expected 2, observed 1")

    def execute():
        debug.failed({"action_id": "inner", "title": "Assert"}, error)
        debug.failed({"action_id": "outer", "title": "Group"}, error)

    thread = threading.Thread(target=execute, daemon=True)
    thread.start()
    pause = wait_paused(events)
    Writer(tmp_path / "commands.jsonl").emit("continue", pause_id=pause)
    thread.join(2)
    assert not thread.is_alive()
    assert len([e for e in events if e.get("paused")]) == 1


def test_partial_command_and_history_recovery(tmp_path):
    debug = Debugger(SimpleNamespace(artifacts=tmp_path))
    command = {"version": 1, "run_id": tmp_path.name, "sequence": 1, "kind": "pause"}
    debug.path.write_text(json.dumps(command))
    assert list(debug.commands()) == []
    with debug.path.open("a") as stream:
        stream.write("\n")
    assert list(debug.commands()) == [command]
    state = RunState()
    state.apply({"kind": "debug-state", "paused": True, "pause_id": 1})
    state.apply({"kind": "inspection", "screenshot": "missing.png", "elements": []})
    state.apply({"kind": "finished", "code": 130, "cancelled": True})
    assert not state.debug["paused"]
    assert state.inspection["screenshot"] == "missing.png"


def test_command_consumption_survives_early_return(tmp_path):
    debug = Debugger(SimpleNamespace(artifacts=tmp_path))
    writer = Writer(debug.path)
    writer.emit("continue", pause_id=1)
    for command in debug.commands():
        assert command["kind"] == "continue"
        break
    writer.emit("step", pause_id=2)
    assert [c["kind"] for c in debug.commands()] == ["step"]
