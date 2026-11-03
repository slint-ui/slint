# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import time
from pathlib import Path

import slint_testing
from canvas_interactions import center
from editor_sync import wait_for_source
from inspector_interactions import inspector_field
from slint_testing import keys
from ui_driver import (
    element,
    elements,
    first_window,
    launch_editor,
    press_key,
    press_keys,
    press_shortcut,
    screenshot,
    select_outline_row,
)


def test_valid_inspector_edits_do_not_show_failure_card(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tmp_path: Path,
) -> None:
    source = fixture_project / "Main.slint"
    original = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, original)
        select_outline_row(window, "root-rectangle")
        for width in (181, 182, 183):
            field = inspector_field(
                window, "Width", slint_testing.AccessibleRole.TextInput
            )
            position = center(field)
            button = slint_testing.PointerEventButton.Left
            window.dispatch_event(slint_testing.PointerPressEvent(position, button))
            window.dispatch_event(slint_testing.PointerReleaseEvent(position, button))
            press_shortcut(window, keys.Control, "a")
            press_keys(window, str(width))
            press_key(window, keys.Return)
            deadline = time.monotonic() + 0.5
            while time.monotonic() < deadline:
                for label in (
                    "Stale preview",
                    "Preview unavailable",
                    "Editing paused until the preview is available",
                ):
                    assert not elements(window, label), f"Valid edit displayed {label}"
                assert elements(
                    window, "Fixture text", role=slint_testing.AccessibleRole.Text
                )
            expected = original.replace(
                b"width: 180px;", f"width: {width}px;".encode(), 1
            )
            wait_for_source(source, expected)
            element(
                window, "Selected Rectangle", role=slint_testing.AccessibleRole.Region
            )
        screenshot(window).save(tmp_path / "valid-inspector-edit.png")
