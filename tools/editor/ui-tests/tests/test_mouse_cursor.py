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

MOUSE_CURSOR_SOURCE = "MouseCursorCases.slint"
LABELS = {
    "default": "Default",
    "pointer": "Pointer",
    "grab": "Grab",
    "crosshair": "Crosshair",
}


@pytest.mark.parametrize(
    ("row", "current", "value", "old", "new"),
    [
        (
            "default-touch-area",
            "default",
            "grab",
            b"        height: 100px;\n    }\n\n    pointer-touch-area",
            b"        height: 100px;\n        mouse-cursor: grab;\n    }\n\n    pointer-touch-area",
        ),
        (
            "pointer-touch-area",
            "pointer",
            "crosshair",
            b"        mouse-cursor: pointer;\n",
            b"        mouse-cursor: crosshair;\n",
        ),
    ],
    ids=("default", "pointer"),
)
def test_mouse_cursor_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    current: str,
    value: str,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / MOUSE_CURSOR_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_outline_row(window, row)
        element(window, "Selected TouchArea", role=slint_testing.AccessibleRole.Region)
        role = slint_testing.AccessibleRole.Combobox
        wait_for_field(window, "Mouse Cursor", LABELS[current], role)
        edit_field(window, "Mouse Cursor", value, role)
        snapshot.wait_for_applied(expected, relative_path=MOUSE_CURSOR_SOURCE)
        wait_for_field(window, "Mouse Cursor", LABELS[value], role)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=MOUSE_CURSOR_SOURCE)
        wait_for_field(window, "Mouse Cursor", LABELS[current], role)
