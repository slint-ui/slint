# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json

import pytest
from editor_sync import EditorSync


def test_wait_rejects_old_response_and_unfinished_edit(tmp_path, monkeypatch):
    sync = EditorSync(tmp_path)
    response = tmp_path / "response.json"
    response.write_text(json.dumps({"id": 0, "ready": True}))
    replies = iter(
        [
            {
                "id": 1,
                "protocol": 2,
                "session": "s",
                "cursor": 0,
                "writes": 0,
                "accepted_edits": 0,
                "ready": True,
            },
            {
                "id": 2,
                "protocol": 2,
                "session": "s",
                "cursor": 1,
                "writes": 0,
                "accepted_edits": 0,
                "ready": False,
                "busy": True,
            },
            {
                "id": 2,
                "protocol": 2,
                "session": "s",
                "cursor": 1,
                "writes": 0,
                "accepted_edits": 0,
                "ready": False,
                "busy": False,
            },
            {
                "id": 2,
                "protocol": 2,
                "session": "s",
                "cursor": 2,
                "writes": 0,
                "accepted_edits": 0,
                "ready": True,
                "events": [],
            },
        ]
    )
    polls = []

    def advance(_):
        reply = next(replies)
        polls.append(reply)
        response.write_text(json.dumps(reply))

    monkeypatch.setattr("editor_sync.time.sleep", advance)
    sync.wait_for_source(tmp_path / "Main.slint", b"new source")
    assert len(polls) == 4
    assert not (tmp_path / "request.json").exists()


def test_wait_timeout_reports_last_state_and_removes_request(tmp_path):
    (tmp_path / "response.json").write_text(json.dumps({"id": 0, "ready": False}))
    with pytest.raises(AssertionError, match="did not reach handshake"):
        EditorSync(tmp_path).wait_for_source(
            tmp_path / "Main.slint", b"source", timeout=0
        )
    assert not (tmp_path / "request.json").exists()


def test_legacy_response_is_rejected(tmp_path):
    (tmp_path / "response.json").write_text(json.dumps({"id": 1, "ready": True}))
    with pytest.raises(AssertionError, match="protocol"):
        EditorSync(tmp_path).wait_for_source(
            tmp_path / "Main.slint", b"source", timeout=0
        )
