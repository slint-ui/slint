# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import inspector_field
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_assertions import expect
from ui_driver import (
    element,
    first_window,
    launch_editor,
    press_shortcut,
    select_outline_row,
)

READ_ONLY_SOURCE = "ReadOnlyCases.slint"


def select_element(window: slint_testing.Window, row: str, kind: str) -> None:
    select_outline_row(window, row)
    element(window, f"Selected {kind}", role=slint_testing.AccessibleRole.Region)


def read_only_switch(window: slint_testing.Window) -> slint_testing.Element:
    return inspector_field(window, "Read-only", slint_testing.AccessibleRole.Switch)


@pytest.mark.parametrize(
    ("row", "kind", "checked", "old", "new"),
    [
        (
            "editable-line-edit",
            "LineEdit",
            False,
            b'        width: 200px;\n        text: "Editable";\n',
            b'        width: 200px;\n        read-only: true;\n        text: "Editable";\n',
        ),
        (
            "read-only-text-edit",
            "TextEdit",
            True,
            b"        read-only: true;\n",
            b"        read-only: false;\n",
        ),
        (
            "editable-spin-box",
            "SpinBox",
            False,
            b"        width: 200px;\n        value: 3;\n",
            b"        width: 200px;\n        read-only: true;\n        value: 3;\n",
        ),
    ],
    ids=("line-edit", "text-edit", "spin-box"),
)
def test_read_only_switch_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    kind: str,
    checked: bool,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / READ_ONLY_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_element(window, row, kind)
        expect(read_only_switch(window)).to_be_checked(checked)
        read_only_switch(window).invoke_accessible_default_action()
        snapshot.wait_for_applied(expected, relative_path=READ_ONLY_SOURCE)
        expect(read_only_switch(window)).to_be_checked(not checked)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=READ_ONLY_SOURCE)
        expect(read_only_switch(window)).to_be_checked(checked)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, relative_path=READ_ONLY_SOURCE)
