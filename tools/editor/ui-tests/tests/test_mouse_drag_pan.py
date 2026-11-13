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

MOUSE_DRAG_PAN_SOURCE = "MouseDragPanCases.slint"


def mouse_drag_pan_switch(window: slint_testing.Window) -> slint_testing.Element:
    return inspector_field(
        window, "Mouse Drag Pan", slint_testing.AccessibleRole.Switch
    )


@pytest.mark.parametrize(
    ("row", "kind", "checked", "old", "new"),
    [
        (
            "default-scroll-view",
            "ScrollView",
            False,
            b"        height: 120px;\n    }\n\n    panning-list-view",
            b"        height: 120px;\n        mouse-drag-pan-enabled: true;\n    }\n\n    panning-list-view",
        ),
        (
            "panning-list-view",
            "ListView",
            True,
            b"        mouse-drag-pan-enabled: true;\n",
            b"        mouse-drag-pan-enabled: false;\n",
        ),
        (
            "default-flickable",
            "Flickable",
            True,
            b"        height: 120px;\n    }\n}\n",
            b"        height: 120px;\n        mouse-drag-pan-enabled: false;\n    }\n}\n",
        ),
    ],
    ids=("scroll-view", "list-view", "flickable"),
)
def test_mouse_drag_pan_switch_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    kind: str,
    checked: bool,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / MOUSE_DRAG_PAN_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_outline_row(window, row)
        element(window, f"Selected {kind}", role=slint_testing.AccessibleRole.Region)
        expect(mouse_drag_pan_switch(window)).to_be_checked(checked)
        mouse_drag_pan_switch(window).invoke_accessible_default_action()
        snapshot.wait_for_applied(expected, relative_path=MOUSE_DRAG_PAN_SOURCE)
        expect(mouse_drag_pan_switch(window)).to_be_checked(not checked)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=MOUSE_DRAG_PAN_SOURCE)
        expect(mouse_drag_pan_switch(window)).to_be_checked(checked)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, relative_path=MOUSE_DRAG_PAN_SOURCE)
