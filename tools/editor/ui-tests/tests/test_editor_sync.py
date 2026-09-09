# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
from types import SimpleNamespace

import pytest
from editor_sync import (
    PROTOCOL_VERSION,
    EditorAction,
    EditorSync,
    SyncCheckpoint,
    SyncResult,
)


def reply(request_id=1, **changes):
    return {
        "id": request_id,
        "protocol": PROTOCOL_VERSION,
        "session": "s",
        "cursor": 0,
        "writes": 0,
        "accepted_edits": 0,
        "ready": True,
        "events": [],
    } | changes


@pytest.fixture
def clock(monkeypatch):
    clock = SimpleNamespace(now=0.0, steps=[])
    monkeypatch.setattr("editor_sync.time.monotonic", lambda: clock.now)

    def advance(seconds):
        clock.now += seconds
        if clock.steps:
            clock.steps.pop(0)()

    monkeypatch.setattr("editor_sync.time.sleep", advance)
    return clock


def test_wait_rejects_old_response_and_waits_for_terminal_state(tmp_path, clock):
    response = tmp_path / "response.json"
    response.write_text(json.dumps(reply(0)))
    replies = [
        reply(1),
        reply(2, ready=False, operations={"3": {"pending": ["publication"]}}),
        reply(2, ready=True),
    ]
    clock.steps = [lambda r=r: response.write_text(json.dumps(r)) for r in replies]
    EditorSync(tmp_path).wait_for_applied(tmp_path / "Main.slint", b"new source")
    assert not clock.steps
    assert not (tmp_path / "request.json").exists()
    assert "publication" in (tmp_path / "trace.jsonl").read_text()


def test_one_deadline_includes_handshake(tmp_path, clock):
    response = tmp_path / "response.json"
    clock.steps = [lambda: response.write_text(json.dumps(reply()))]
    with pytest.raises(AssertionError, match="did not reach applied"):
        EditorSync(tmp_path).wait_for_applied(
            tmp_path / "Main.slint", b"source", timeout=0.04
        )
    assert clock.now == pytest.approx(0.04)
    assert not (tmp_path / "request.json").exists()


@pytest.mark.parametrize(
    "response,match",
    [
        (reply(protocol=2), "protocol"),
        (reply(session="old"), "stale"),
        (reply(ready=False, writes=True), "writes"),
        (reply(ready=False, events=None), "event history"),
        (reply(error="event history overflow"), "overflow"),
        (reply(error="request ID reused"), "reused"),
        ({"ready": True}, "malformed"),
    ],
)
def test_invalid_responses_fail_even_before_ready(tmp_path, clock, response, match):
    (tmp_path / "response.json").write_text(json.dumps(response))
    with pytest.raises((AssertionError, TypeError), match=match):
        EditorSync(tmp_path, session="s").checkpoint()
    assert clock.now == 0
    assert not (tmp_path / "request.json").exists()


def test_exit_fails_without_waiting(tmp_path, clock):
    class ExitedProcess:
        def poll(self) -> int:
            return 3

    with pytest.raises(AssertionError, match="editor exited"):
        EditorSync(tmp_path, process=ExitedProcess()).checkpoint()
    assert clock.now == 0


def test_stale_checkpoint_is_rejected_before_request(tmp_path, clock):
    sync = EditorSync(tmp_path, session="s")
    with pytest.raises(AssertionError, match="another editor session"):
        sync.wait_for_processed(
            tmp_path / "Main.slint", None, after=SyncCheckpoint("old", 0)
        )
    assert not (tmp_path / "request.json").exists()


def test_action_seals_without_inventing_an_outcome(tmp_path, monkeypatch):
    sync = EditorSync(tmp_path, session="s")
    requests = []

    def request(**kwargs):
        requests.append(kwargs)
        return SimpleNamespace(data={"operation": 7})

    monkeypatch.setattr(sync, "_request", request)
    with pytest.raises(ValueError), sync.action():
        raise ValueError("input failed")
    assert [r["mode"] for r in requests] == ["begin", "seal"]
    assert all("outcome" not in r for r in requests)


def test_gate_released_when_test_fails(tmp_path, monkeypatch):
    sync = EditorSync(tmp_path, session="s")
    requests = []

    def request(**kwargs):
        requests.append(kwargs)
        return SimpleNamespace(data={"gate": 8})

    monkeypatch.setattr(sync, "_request", request)
    with pytest.raises(ValueError), sync.gate("publication", tmp_path / "Main.slint"):
        raise ValueError("assertion failed")
    assert [r["mode"] for r in requests] == ["gate_open", "gate_release"]
    assert requests[-1]["gate"] == 8


@pytest.mark.parametrize(
    "state,message",
    [
        ({"writes": 1, "mutations": 1, "accepted_edits": 1}, "wrote source"),
        ({"writes": 0, "mutations": 1, "accepted_edits": 1}, "may have changed source"),
        ({"writes": 0, "mutations": 0, "accepted_edits": 1}, "accepted an edit"),
    ],
)
def test_no_write_assertion_rejects_each_mutation_evidence(
    tmp_path, monkeypatch, state, message
):
    action = EditorAction(EditorSync(tmp_path), 1, 5, sealed=True)
    monkeypatch.setattr(
        action, "wait_for_settled", lambda: SyncResult({"operation_state": state})
    )
    with pytest.raises(AssertionError, match=message):
        action.assert_no_source_writes()
