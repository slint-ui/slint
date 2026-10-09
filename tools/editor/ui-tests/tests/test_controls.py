# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import begin_palette_drag
from editor_sync import wait_for_source
from inspector_interactions import edit_field
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_driver import element, first_window, launch_editor, press_shortcut


@pytest.mark.parametrize(
    ("label", "type_name", "property_name", "initial", "updated", "width"),
    [
        ("Button", "ControlButton", "text", '"Button"', '"Tap me"', 160),
        ("Slider", "ControlSlider", "value", "42", "65", 200),
        (
            "ComboBox",
            "ControlComboBox",
            "model",
            '["First", "Second", "Third"]',
            '["Small", "Large"]',
            160,
        ),
    ],
)
def test_controls_drop_edit_and_undo(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    label: str,
    type_name: str,
    property_name: str,
    initial: str,
    updated: str,
    width: int,
) -> None:
    source_file = fixture_project / "Palette.slint"
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        element(window, "Controls", role=slint_testing.AccessibleRole.Button)
        artboard = element(window, "Artboard", role=slint_testing.AccessibleRole.Region)
        target = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + 195,
            y=artboard.absolute_position.y + 360,
        )
        begin_palette_drag(window, label, target)
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(target, slint_testing.PointerEventButton.Left)
        )
        expected = baseline.replace(
            b"export component",
            f'import {{ {type_name} }} from "@editor-controls";\nexport component'.encode(),
            1,
        ).replace(
            b"    background: #f8fafc;\n",
            (
                "    background: #f8fafc;\n"
                f"    {type_name} {{\n"
                f"        {property_name}: {initial};\n"
                f"        x: {195 - width // 2}px;\n"
                "        y: 342px;\n"
                f"        width: {width}px;\n"
                "        height: 36px;\n"
                "    }\n"
            ).encode(),
            1,
        )
        snapshot.wait_for_applied(expected, "Palette.slint")
        element(window, f"Selected {type_name}", role=slint_testing.AccessibleRole.Region)
        edit_field(window, f"Control {property_name}", updated)
        edited = replace_once(
            expected,
            f"        {property_name}: {initial};".encode(),
            f"        {property_name}: {updated};".encode(),
        )
        snapshot.wait_for_applied(edited, "Palette.slint")
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(expected, "Palette.slint")
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, "Palette.slint")
