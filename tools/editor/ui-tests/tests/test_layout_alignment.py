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

LAYOUT_ALIGNMENT_SOURCE = "LayoutAlignmentCases.slint"
LABELS = {
    "stretch": "Stretch",
    "center": "Center",
    "start": "Start",
    "end": "End",
    "space-between": "Space between",
    "space-around": "Space around",
    "space-evenly": "Space evenly",
}


@pytest.mark.parametrize(
    ("row", "kind", "field", "current", "value", "old", "new"),
    [
        (
            "default-horizontal-layout",
            "HorizontalLayout",
            "Layout Alignment",
            "stretch",
            "end",
            b"        width: 200px;\n        height: 64px;\n",
            b"        width: 200px;\n        alignment: end;\n        height: 64px;\n",
        ),
        (
            "center-vertical-layout",
            "VerticalLayout",
            "Layout Alignment",
            "center",
            "space-between",
            b"        alignment: center;\n",
            b"        alignment: space-between;\n",
        ),
        (
            "default-flexbox-layout",
            "FlexboxLayout",
            "Layout Alignment",
            "start",
            "space-evenly",
            b"        y: 280px;\n        width: 200px;\n",
            b"        y: 280px;\n        width: 200px;\n        alignment: space-evenly;\n",
        ),
        (
            "default-flexbox-layout",
            "FlexboxLayout",
            "Cross Axis Line Alignment",
            "stretch",
            "center",
            b"        y: 280px;\n        width: 200px;\n",
            b"        y: 280px;\n        width: 200px;\n        cross-axis-line-alignment: center;\n",
        ),
    ],
    ids=(
        "horizontal-layout-alignment",
        "vertical-layout-alignment",
        "flexbox-layout-alignment",
        "flexbox-layout-line-alignment",
    ),
)
def test_layout_alignment_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    kind: str,
    field: str,
    current: str,
    value: str,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / LAYOUT_ALIGNMENT_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_outline_row(window, row)
        element(window, f"Selected {kind}", role=slint_testing.AccessibleRole.Region)
        role = slint_testing.AccessibleRole.Combobox
        wait_for_field(window, field, LABELS[current], role)
        edit_field(window, field, value, role)
        snapshot.wait_for_applied(expected, relative_path=LAYOUT_ALIGNMENT_SOURCE)
        wait_for_field(window, field, LABELS[value], role)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=LAYOUT_ALIGNMENT_SOURCE)
        wait_for_field(window, field, LABELS[current], role)
