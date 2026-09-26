# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json

import pytest

from bridge import StudioPlugin
from state import RunState, action_rows


def test_nested_journal_replays_and_preserves_positions(tmp_path):
    events = [
        {"kind": "collected", "id": "test", "title": "Test"},
        {"kind": "test-start", "nodeid": "test"},
        {
            "kind": "action-start",
            "nodeid": "test",
            "action_id": "outer",
            "title": "Adapter",
        },
        {
            "kind": "action-start",
            "nodeid": "test",
            "action_id": "inner",
            "parent_id": "outer",
            "title": "Fill",
        },
        {
            "kind": "action-end",
            "nodeid": "test",
            "action_id": "inner",
            "status": "Passed",
            "duration": 0.2,
        },
        {"kind": "finished", "code": 2, "cancelled": True},
    ]
    state = RunState()
    for event in events:
        state.apply(event)
    path = tmp_path / "events.jsonl"
    path.write_text("\n".join(json.dumps(e) for e in events))
    restored = RunState()
    for line in path.read_text().splitlines():
        restored.apply(json.loads(line))
    assert state == restored
    actions = state.records["test"]["steps"]
    assert [(a["title"], a["status"]) for a in actions] == [
        ("Adapter", "Cancelled"),
        ("Fill", "Passed"),
    ]
    assert actions[1]["depth"] == 1
    assert [r["row_index"] for r in action_rows(actions, {"outer"})] == [0]


def test_bridge_action_events_use_relative_failure_capture(tmp_path):
    class Window:
        def grab_window_as_png(self):
            return b"capture"

    class Application:
        first_window = Window()

    plugin = StudioPlugin(tmp_path / "events.jsonl", tmp_path)
    plugin.nodeid = "test"
    plugin.application = Application()
    plugin.observe_action(
        {
            "kind": "action-end",
            "action_id": "a",
            "status": "Failed",
            "detail": "expected 2, observed 1",
        }
    )
    events = [
        json.loads(line)
        for line in (tmp_path / "events.jsonl").read_text().splitlines()
    ]
    assert events[-1]["screenshot"] == "capture-0001.png"
    assert (tmp_path / events[-1]["screenshot"]).read_bytes() == b"capture"
    assert events[-1]["nodeid"] == "test"


@pytest.mark.parametrize("policy", ["none", "failures", "boundaries"])
def test_capture_policy(tmp_path, monkeypatch, policy):
    class Window:
        def grab_window_as_png(self):
            return b"capture"

    class Application:
        first_window = Window()

    monkeypatch.setenv("SLINT_STUDIO_CAPTURES", policy)
    plugin = StudioPlugin(tmp_path / "events.jsonl", tmp_path)
    plugin.application = Application()
    plugin.completed_action("Launch")
    plugin.observe_action(
        {
            "kind": "action-end",
            "action_id": "a",
            "status": "Failed",
            "title": "Assertion",
        }
    )
    events = [
        json.loads(line)
        for line in (tmp_path / "events.jsonl").read_text().splitlines()
    ]
    assert [event["kind"] for event in events[:2]] == [
        "action-start",
        "action-end",
    ]
    assert bool(events[1]["screenshot"]) == (policy == "boundaries")
    assert bool(events[2]["screenshot"]) == (policy != "none")


def test_legacy_stages_use_the_action_timeline(tmp_path):
    plugin = StudioPlugin(tmp_path / "events.jsonl", tmp_path)

    plugin.observe("stage-start", title="Edit")
    plugin.observe("stage-end", title="Edit", failed=False, duration=0.2)

    events = [
        json.loads(line)
        for line in (tmp_path / "events.jsonl").read_text().splitlines()
    ]
    assert [event["kind"] for event in events] == ["action-start", "action-end"]
    assert events[0]["action_id"] == events[1]["action_id"]
    assert events[0]["layer"] == "stage"


def test_concise_failure_keeps_technical_details_and_replays():
    from state import action_details, failure_presentation

    diagnostic = {
        "kind": "assertion",
        "summary": "Expected 250, observed 200",
        "target": "Width · value",
        "timeout_ms": 500,
    }
    action = {
        "title": "Expect very long locator",
        "layer": "assertion",
        "diagnostic": diagnostic,
        "source": {
            "file": "/checkout/tests/demo.py",
            "line": 16,
            "text": 'expect(width).to_have_value("250")',
        },
        "arguments": {"locator": "full scoped locator"},
    }
    event = {
        "kind": "debug-state",
        "paused": True,
        "action": action,
        "error": "full timeout and protocol details",
    }
    state = RunState()
    state.apply(json.loads(json.dumps(event)))
    view = failure_presentation(state.debug)
    assert view["summary"] == "Expected 250, observed 200"
    assert view["context"] == "Width · value · timeout 500 ms"
    assert view["source"].startswith("demo.py:16\n")
    assert "full timeout and protocol details" in view["details"]
    assert "/checkout/tests/demo.py:16" in view["details"]
    assert "full scoped locator" in view["details"]
    assert action_details(dict(action, status="Failed")).startswith(
        "Expected 250, observed 200"
    )


def test_unstructured_failures_do_not_invent_expected_values():
    from state import failure_presentation

    result = failure_presentation(
        {"action": {"layer": "assertion"}, "error": "arbitrary old failure"}
    )
    assert result["summary"] == "Assertion failed"
    assert "arbitrary old failure" in result["details"]
