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

INPUT_TYPE_SOURCE = "InputTypeCases.slint"


@pytest.mark.parametrize(
    ("row", "label", "value", "old", "new"),
    [
        (
            "default-line-edit",
            "Text",
            "number",
            b'        width: 200px;\n        text: "Default";\n',
            b'        width: 200px;\n        input-type: number;\n        text: "Default";\n',
        ),
        (
            "password-line-edit",
            "Password",
            "decimal",
            b"        input-type: password;\n",
            b"        input-type: decimal;\n",
        ),
    ],
    ids=("default", "password"),
)
def test_input_type_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    label: str,
    value: str,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / INPUT_TYPE_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_outline_row(window, row)
        element(window, "Selected LineEdit", role=slint_testing.AccessibleRole.Region)
        role = slint_testing.AccessibleRole.Combobox
        wait_for_field(window, "Input Type", label, role)
        edit_field(window, "Input Type", value, role)
        snapshot.wait_for_applied(expected, relative_path=INPUT_TYPE_SOURCE)
        wait_for_field(window, "Input Type", value.capitalize(), role)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=INPUT_TYPE_SOURCE)
        wait_for_field(window, "Input Type", label, role)
