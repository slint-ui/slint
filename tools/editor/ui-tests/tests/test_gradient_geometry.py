# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from slint_testing import keys
from ui_driver import (
    first_window,
    launch_editor,
    select_outline_row,
    wait_until,
    window_element_with_label,
)


@pytest.mark.parametrize("kind", ["radial", "conic"])
def test_custom_gradient_geometry_uses_layout_size(
    editor_binary: Path,
    editor_environment: dict[str, str],
    tmp_path: Path,
    kind: str,
) -> None:
    source_file = tmp_path / "LayoutGradient.slint"
    gradient = (
        "@radial-gradient(circle, red 0%, blue 100%)"
        if kind == "radial"
        else "@conic-gradient(from 0deg, red 0deg, blue 360deg)"
    )
    source = f"""export component LayoutGradient inherits Window {{
    width: 400px;
    height: 400px;
    VerticalLayout {{
        fill := Rectangle {{
            background: {gradient};
        }}
    }}
}}
"""
    source_file.write_text(source)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        rectangle = wait_until(
            lambda: next(iter(window.find_elements_by_id("LayoutGradient::fill")), None)
        )
        assert rectangle.size.width == pytest.approx(400)
        assert rectangle.size.height == pytest.approx(400)

        def field(label, role):
            return window_element_with_label(window, label, role)

        field(
            "Rectangle background color picker", slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        field(
            "Gradient center", slint_testing.AccessibleRole.Combobox
        ).accessible_value = "Custom"
        for axis in ("X", "Y"):
            assert float(
                field(
                    f"Gradient center {axis}", slint_testing.AccessibleRole.TextInput
                ).accessible_value
            ) == pytest.approx(200)
        if kind == "radial":
            field(
                "Gradient radius mode", slint_testing.AccessibleRole.Combobox
            ).accessible_value = "Custom"
            assert float(
                field(
                    "Gradient radius", slint_testing.AccessibleRole.TextInput
                ).accessible_value
            ) == pytest.approx(282.8)
        assert source_file.read_text() == source
        field(
            "Close Custom", slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        wait_until(
            lambda: True if "at 200px 200px" in source_file.read_text() else None
        )
        if kind == "radial":
            assert "circle 282.842" in source_file.read_text()


def picker_field(window, label, role=slint_testing.AccessibleRole.TextInput):
    return window_element_with_label(window, label, role)


def set_picker_mode(window, label, value):
    picker_field(
        window, label, slint_testing.AccessibleRole.Combobox
    ).accessible_value = value


def open_gradient(window):
    picker_field(
        window, "Rectangle background color picker", slint_testing.AccessibleRole.Button
    ).invoke_accessible_default_action()


def gradient_document(directory, expression):
    file = directory / "Gradient.slint"
    file.write_text(f"""export component Gradient inherits Window {{
    width: 400px;
    height: 400px;
    VerticalLayout {{
        fill := Rectangle {{ background: {expression}; }}
    }}
}}
""")
    return file


@pytest.mark.parametrize("loaded_custom", [False, True])
def test_custom_geometry_survives_mode_changes(
    editor_binary, editor_environment, tmp_path, loaded_custom
):
    from ui_driver import press_key
    from source_snapshot import SourceSnapshot

    expression = (
        "@radial-gradient(circle 95px at 37px 61px, red 0%, blue 100%)"
        if loaded_custom
        else "@radial-gradient(circle, red 0%, blue 100%)"
    )
    file = gradient_document(tmp_path, expression)
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        if not loaded_custom:
            set_picker_mode(window, "Gradient center", "Custom")
            picker_field(window, "Gradient center X").accessible_value = "37"
            picker_field(window, "Gradient center Y").accessible_value = "61"
            set_picker_mode(window, "Gradient radius mode", "Custom")
            picker_field(window, "Gradient radius").accessible_value = "95"
        set_picker_mode(window, "Gradient center", "Automatic")
        set_picker_mode(window, "Gradient radius mode", "Automatic")
        set_picker_mode(window, "Gradient type", "Conic")
        set_picker_mode(window, "Gradient center", "Custom")
        picker_field(window, "Gradient center X").accessible_value = "83"
        picker_field(window, "Gradient center Y").accessible_value = "109"
        set_picker_mode(window, "Gradient center", "Automatic")
        set_picker_mode(window, "Gradient type", "Radial")
        set_picker_mode(window, "Gradient center", "Custom")
        set_picker_mode(window, "Gradient radius mode", "Custom")
        for label, expected in [
            ("Gradient center X", 37),
            ("Gradient center Y", 61),
            ("Gradient radius", 95),
        ]:
            assert float(picker_field(window, label).accessible_value) == expected
        set_picker_mode(window, "Gradient type", "Conic")
        set_picker_mode(window, "Gradient center", "Custom")
        assert float(picker_field(window, "Gradient center X").accessible_value) == 83
        assert float(picker_field(window, "Gradient center Y").accessible_value) == 109
        press_key(window, keys.Escape)
        original.assert_unchanged()
