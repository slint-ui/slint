# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import edit_field, wait_for_field
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_driver import (
    element,
    first_window,
    launch_editor,
    press_shortcut,
    select_outline_row,
)

TITLE_SOURCE = "TitleCases.slint"


@pytest.mark.parametrize(
    ("row", "kind", "current", "old", "new"),
    [
        (
            "titled-group-box",
            "GroupBox",
            '"Options"',
            b'        title: "Options";\n',
            b'        title: "Settings";\n',
        ),
        (
            "untitled-radio-group",
            "RadioGroup",
            '""',
            b"        y: 192px;\n",
            b'        y: 192px;\n        title: "Settings";\n',
        ),
        (
            "titled-tab",
            "Tab",
            '"General"',
            b'            title: "General";\n',
            b'            title: "Settings";\n',
        ),
    ],
    ids=("group-box", "radio-group", "tab"),
)
def test_title_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    kind: str,
    current: str,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / TITLE_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_outline_row(window, row)
        element(window, f"Selected {kind}", role=slint_testing.AccessibleRole.Region)
        wait_for_field(window, "Title", current, slint_testing.AccessibleRole.TextInput)
        edit_field(
            window, "Title", '"Settings"', slint_testing.AccessibleRole.TextInput
        )
        snapshot.wait_for_applied(expected, relative_path=TITLE_SOURCE)
        wait_for_field(
            window, "Title", '"Settings"', slint_testing.AccessibleRole.TextInput
        )
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=TITLE_SOURCE)
        wait_for_field(window, "Title", current, slint_testing.AccessibleRole.TextInput)
