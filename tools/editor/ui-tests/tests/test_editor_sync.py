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
            {"id": 1, "ready": False, "busy": True},
            {"id": 1, "ready": False, "busy": False, "mismatches": ["old source"]},
            {"id": 1, "ready": True},
        ]
    )
    polls = []

    def advance(_):
        reply = next(replies)
        polls.append(reply)
        response.write_text(json.dumps(reply))

    monkeypatch.setattr("editor_sync.time.sleep", advance)
    sync.wait_for_source(tmp_path / "Main.slint", b"new source")
    assert len(polls) == 3
    assert not (tmp_path / "request.json").exists()


def test_wait_timeout_reports_last_state_and_removes_request(tmp_path):
    (tmp_path / "response.json").write_text(
        json.dumps({"id": 1, "ready": False, "busy": True})
    )
    with pytest.raises(AssertionError, match="'busy': True"):
        EditorSync(tmp_path).wait_for_source(
            tmp_path / "Main.slint", b"source", timeout=0
        )
    assert not (tmp_path / "request.json").exists()
