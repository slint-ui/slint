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
    from source_snapshot import SourceSnapshot
    from ui_driver import press_key

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


@pytest.mark.parametrize("kind", ["linear", "radial", "conic"])
def test_stop_precision_survives_save_and_reopen(
    editor_binary, editor_environment, tmp_path, kind
):
    from source_snapshot import SourceSnapshot

    prefix = {"linear": "90deg", "radial": "circle", "conic": "from 0deg"}[kind]
    unit = "deg" if kind == "conic" else "%"
    expression = f"@{kind}-gradient({prefix}, #ff0000 0{unit} - 12.345678{unit}, #00ff0080 33.333333{unit}, blue 33.333333{unit}, white 123.456789{unit})"
    file = gradient_document(tmp_path, expression)
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")

        def save():
            picker_field(
                window, "Close Custom", slint_testing.AccessibleRole.Button
            ).invoke_accessible_default_action()
            content = wait_until(
                lambda: (
                    file.read_bytes()
                    if file.read_bytes() != snapshot.sources[Path(file.name)]
                    else None
                )
            )
            snapshot.wait_for_applied(content, file.name)
            return content

        open_gradient(window)
        picker_field(window, "Stop 2 position").accessible_value = "33.333333"
        if kind == "radial":
            set_picker_mode(window, "Gradient center", "Custom")
        else:
            picker_field(window, "Gradient angle degrees").accessible_value = "45"
        first = save()
        assert b"33.33%" not in first
        assert b"33.3333" in first
        select_outline_row(window, "fill")
        open_gradient(window)
        if kind == "radial":
            picker_field(window, "Gradient center X").accessible_value = "201"
        else:
            picker_field(window, "Gradient angle degrees").accessible_value = "46"
        picker_field(
            window, "Close Custom", slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        second = wait_until(
            lambda: file.read_bytes() if file.read_bytes() != first else None
        )
        snapshot.wait_for_applied(second, file.name)
        assert first.split(b",", 1)[1] == second.split(b",", 1)[1]


def click_picker_button(window, label):
    picker_field(
        window, label, slint_testing.AccessibleRole.Button
    ).invoke_accessible_default_action()


def stop_point(window, index, side_open=False):
    handle = picker_field(
        window, f"Gradient stop {index}", slint_testing.AccessibleRole.Slider
    )
    anchor = picker_field(
        window, "Rectangle background color picker", slint_testing.AccessibleRole.Button
    )
    # Embedded popup element positions omit the popup origin in the input coordinate system.
    popup_x = anchor.absolute_position.x - 308 - (308 if side_open else 0)
    return slint_testing.LogicalPosition(
        x=popup_x + handle.absolute_position.x + handle.size.width / 2,
        y=anchor.absolute_position.y
        + handle.absolute_position.y
        + handle.size.height / 2,
    )


def pointer_gesture(window, start, end):
    button = slint_testing.PointerEventButton.Left
    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    window.dispatch_event(slint_testing.PointerMoveEvent(end))
    window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))


def test_stop_interactions_preserve_color_identity(
    editor_binary, editor_environment, tmp_path
):
    from source_snapshot import SourceSnapshot
    from ui_driver import elements_with_label

    file = gradient_document(
        tmp_path, "@linear-gradient(90deg, red 0%, #0000ff80 50%, white 100%)"
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        click_picker_button(window, "Add gradient stop")
        assert float(picker_field(window, "Stop 2 position").accessible_value) == 25
        click_picker_button(window, "Edit stop 2 color")
        assert picker_field(window, "Hex color").accessible_value == "#aa0055c0"
        picker_field(window, "Hex color").accessible_value = "#00ff00b0"
        original.assert_unchanged_now()
        start, left, right = (
            stop_point(window, 2, True),
            stop_point(window, 1, True),
            stop_point(window, 4, True),
        )
        end = slint_testing.LogicalPosition(
            x=left.x + (right.x - left.x) * 0.75, y=start.y
        )
        pointer_gesture(window, start, end)
        assert float(
            picker_field(window, "Stop 3 position").accessible_value
        ) == pytest.approx(75, abs=0.1)
        assert picker_field(window, "Hex color").accessible_value == "#00ff00b0"
        click_picker_button(window, "Close Stop color")
        original.assert_unchanged_now()
        picker_field(window, "Stop 3 position").accessible_value = "50"
        left, right = stop_point(window, 1), stop_point(window, 4)
        insertion = slint_testing.LogicalPosition(
            x=left.x + (right.x - left.x) * 0.75, y=left.y - 16
        )
        pointer_gesture(window, insertion, insertion)
        assert float(
            picker_field(window, "Stop 4 position").accessible_value
        ) == pytest.approx(75, abs=0.1)
        click_picker_button(window, "Remove stop 5")
        click_picker_button(window, "Remove stop 1")
        click_picker_button(window, "Remove stop 3")
        assert not elements_with_label(
            window.root_element, "Remove stop 1", slint_testing.AccessibleRole.Button
        )
        original.assert_unchanged_now()
        click_picker_button(window, "Close Custom")
        saved = wait_until(
            lambda: (
                file.read_bytes()
                if file.read_bytes() != original.sources[Path(file.name)]
                else None
            )
        )
        original.wait_for_applied(saved, file.name)
        assert b"#0000ff80 50%, #00ff00b0 50%" in saved
        select_outline_row(window, "fill")
        open_gradient(window)
        assert float(picker_field(window, "Stop 1 position").accessible_value) == 50
        assert float(picker_field(window, "Stop 2 position").accessible_value) == 50
        click_picker_button(window, "Edit stop 2 color")
        assert picker_field(window, "Hex color").accessible_value == "#00ff00b0"


def test_gradient_session_cancel_undo_redo_and_reopen(
    editor_binary, editor_environment, tmp_path
):
    from source_snapshot import SourceSnapshot
    from ui_driver import press_key, press_shortcut

    file = gradient_document(tmp_path, "root.paint")
    source = file.read_text().replace(
        "    width: 400px;",
        "    private property <brush> paint: @radial-gradient(circle 90px at 40px 60px, red 0%, blue 100%);\n    width: 400px;",
    )
    file.write_text(source)
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        for cancel in [True, False]:
            open_gradient(window)
            picker_field(window, "No recent fills", slint_testing.AccessibleRole.Text)
            click_picker_button(window, "Add gradient stop")
            click_picker_button(window, "Edit stop 2 color")
            picker_field(window, "Hex color").accessible_value = "#12345680"
            click_picker_button(window, "Close Stop color")
            set_picker_mode(window, "Gradient type", "Conic")
            picker_field(window, "Gradient angle degrees").accessible_value = "37"
            set_picker_mode(window, "Gradient type", "Linear")
            click_picker_button(window, "Solid")
            assert picker_field(window, "Hex color").accessible_value == "#12345680"
            click_picker_button(window, "Gradient")
            set_picker_mode(window, "Gradient type", "Radial")
            assert (
                float(picker_field(window, "Gradient center X").accessible_value) == 40
            )
            assert float(picker_field(window, "Gradient radius").accessible_value) == 90
            original.assert_unchanged_now()
            if cancel:
                press_key(window, keys.Escape)
                original.assert_unchanged()
            else:
                click_picker_button(window, "Close Custom")
        saved = wait_until(
            lambda: file.read_bytes() if file.read_text() != source else None
        )
        original.wait_for_applied(saved, file.name)
        assert b"#12345680 50%" in saved
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(source.encode(), file.name)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, file.name)
        select_outline_row(window, "fill")
        open_gradient(window)
        assert float(picker_field(window, "Gradient radius").accessible_value) == 90
        assert float(picker_field(window, "Gradient center Y").accessible_value) == 60
        click_picker_button(window, "Edit stop 2 color")
        assert picker_field(window, "Hex color").accessible_value == "#12345680"
