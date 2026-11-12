# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import edit_field, inspector_field, wait_for_field
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

LAYOUT_GEOMETRY_SOURCE = "LayoutGeometryCases.slint"


@pytest.mark.parametrize(
    ("row", "kind", "label", "current", "value", "old", "new"),
    [
        (
            "layout-child",
            "Rectangle",
            "Width",
            "80",
            "120",
            b"            width: 80px;\n",
            b"            width: 120px;\n",
        ),
        (
            "geometry-layout",
            "HorizontalLayout",
            "Height",
            "200",
            "160",
            b"        height: 200px;\n",
            b"        height: 160px;\n",
        ),
    ],
    ids=("layout-child", "layout"),
)
def test_layout_geometry_edits_size_binding(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    kind: str,
    label: str,
    current: str,
    value: str,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / LAYOUT_GEOMETRY_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_outline_row(window, row)
        element(window, f"Selected {kind}", role=slint_testing.AccessibleRole.Region)
        for position in ("Position X", "Position Y"):
            expect(
                inspector_field(
                    window, position, slint_testing.AccessibleRole.TextInput
                )
            ).to_be_enabled(False)
        wait_for_field(window, label, current, slint_testing.AccessibleRole.TextInput)
        edit_field(window, label, value, slint_testing.AccessibleRole.TextInput)
        snapshot.wait_for_applied(expected, relative_path=LAYOUT_GEOMETRY_SOURCE)
        wait_for_field(window, label, value, slint_testing.AccessibleRole.TextInput)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=LAYOUT_GEOMETRY_SOURCE)
        wait_for_field(window, label, current, slint_testing.AccessibleRole.TextInput)
