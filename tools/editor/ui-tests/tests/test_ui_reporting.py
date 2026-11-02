# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
from typing import Any, cast
from unittest.mock import Mock, PropertyMock

import pytest
from ui_reporting import TestReport as Report
from ui_reporting import capture_failure, current_report


@pytest.mark.parametrize("failure", ["window", "screenshot", "rect"])
def test_failure_state_survives_diagnostic_errors(tmp_path, failure):
    selected = Mock(accessible_label="Selected Text", accessible_value="Fixture text")
    selected.absolute_rect = (10, 20, 30, 40)
    window = Mock()
    window.grab_window_as_png.return_value = b"image"
    window.query_descendants.return_value.match_accessible_role.return_value.find_all.return_value = [
        selected
    ]
    window.query_descendants.return_value.match_accessible_label.return_value.find_all.return_value = []
    application = Mock(first_window=window)
    if failure == "window":
        type(application).first_window = PropertyMock(
            side_effect=RuntimeError("disconnected")
        )
    elif failure == "screenshot":
        window.grab_window_as_png.side_effect = RuntimeError("render failed")
    else:
        type(selected).absolute_rect = PropertyMock(
            side_effect=RuntimeError("geometry unavailable")
        )
    report = Report("failed test", tmp_path, 0)
    token = current_report.set(report)
    error = AssertionError("original failure")
    try:
        capture_failure(cast(Any, application), error)
    finally:
        current_report.reset(token)
    state = json.loads((tmp_path / "failure-1.json").read_text())
    assert state["test"] == "failed test"
    assert failure in state["errors"]
    assert any("Failure UI state:" in note for note in error.__notes__)
    if failure != "window":
        assert state["selection"][0]["label"] == "Selected Text"
        assert state["selection"][0]["value"] == "Fixture text"
        assert state["selection"][0]["rect"] == (
            None if failure == "rect" else [10, 20, 30, 40]
        )
        assert state["inline_text_editor"] == []
