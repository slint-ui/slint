# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path
import math

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
        geometry = (
            radial_geometry(window, "LayoutGradient::fill")
            if kind == "radial"
            else conic_geometry(window, element_id="LayoutGradient::fill")
        )
        assert geometry == pytest.approx(
            (200, 200, math.hypot(200, 200) if kind == "radial" else 160), abs=0.001
        )
        click_picker_button(window, "Close Custom")
        assert source_file.read_text() == source


@pytest.mark.parametrize("kind", ["linear", "radial", "conic"])
def test_non_canvas_gradient_keeps_numeric_geometry(
    editor_binary, editor_environment, tmp_path, kind
):
    from source_snapshot import SourceSnapshot
    from ui_driver import elements_with_label, press_key

    prefix = {"linear": "0deg", "radial": "circle", "conic": "from 0deg"}[kind]
    stops = "red 0deg, blue 360deg" if kind == "conic" else "red 0%, blue 100%"
    file = tmp_path / "TextGradient.slint"
    file.write_text(f"""export component TextGradient inherits Window {{
    width: 400px;
    height: 400px;
    label := Text {{
        width: 200px;
        height: 200px;
        text: "Gradient";
        color: @{kind}-gradient({prefix}, {stops});
    }}
}}
""")
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        window = first_window(editor)
        select_outline_row(window, "label")
        click_picker_button(window, "Text color color picker")
        assert not elements_with_label(window.root_element, "Gradient center handle")
        assert not elements_with_label(window.root_element, "Gradient start")
        if kind != "radial":
            picker_field(window, "Gradient angle degrees").accessible_value = "36"
        if kind != "linear":
            set_picker_mode(window, "Gradient center", "Custom")
            picker_field(window, "Gradient center X").accessible_value = "37"
            picker_field(window, "Gradient center Y").accessible_value = "61"
        if kind == "radial":
            set_picker_mode(window, "Gradient radius mode", "Custom")
            picker_field(window, "Gradient radius").accessible_value = "95"
        click_picker_button(window, "Close Custom")
        saved = wait_until(
            lambda: file.read_bytes()
            if file.read_bytes() != original.sources[Path(file.name)]
            else None
        )
        original.wait_for_applied(saved, file.name)
        if kind != "radial":
            assert b"36deg" in saved
        if kind != "linear":
            assert b"at 37px 61px" in saved
        if kind == "radial":
            assert b"circle 95px" in saved
        click_picker_button(window, "Text color color picker")
        click_picker_button(window, "Add gradient stop")
        press_key(window, keys.Escape)
        assert file.read_bytes() == saved


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


@pytest.mark.parametrize("kind", ["radial", "conic"])
def test_picker_uses_live_preview_stop_markers(
    editor_binary, editor_environment, tmp_path, kind
):
    expression = (
        "@radial-gradient(circle, red, blue)"
        if kind == "radial"
        else "@conic-gradient(from 0deg, red 0deg, blue 360deg)"
    )
    file = gradient_document(tmp_path, expression)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        stop = picker_field(
            window, "Gradient stop 1", slint_testing.AccessibleRole.Slider
        )
        assert stop.size.width == 24
        assert stop.size.height == 24
        (tmp_path / "picker-stop-markers.png").write_bytes(window.grab_window_as_png())


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
            set_radial_geometry(window, 37, 61, 95)
        set_picker_mode(window, "Gradient type", "Conic")
        set_conic_center(window, 83, 109)
        set_picker_mode(window, "Gradient type", "Radial")
        assert radial_geometry(window) == pytest.approx((37, 61, 95), abs=0.001)
        set_picker_mode(window, "Gradient type", "Conic")
        assert conic_geometry(window)[:2] == pytest.approx((83, 109), abs=0.001)
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
        if kind != "linear":
            picker_field(window, "Stop 2 position").accessible_value = "33.333333"
        if kind == "linear":
            click_picker_button(window, "Edit stop 1 color")
            picker_field(window, "Hex color").accessible_value = "#ff0100"
        elif kind == "radial":
            click_picker_button(window, "Gradient center handle")
            from ui_driver import press_key

            press_key(window, keys.RightArrow)
        else:
            rotate_conic(window, 0, 45)
        first = save()
        assert b"33.33%" not in first
        assert b"33.3333" in first
        select_outline_row(window, "fill")
        open_gradient(window)
        if kind == "linear":
            click_picker_button(window, "Edit stop 1 color")
            picker_field(window, "Hex color").accessible_value = "#ff0200"
        elif kind == "radial":
            click_picker_button(window, "Gradient center handle")
            press_key(window, keys.RightArrow)
        else:
            rotate_conic(window, 45, 46)
        picker_field(
            window, "Close Custom", slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        second = wait_until(
            lambda: file.read_bytes() if file.read_bytes() != first else None
        )
        snapshot.wait_for_applied(second, file.name)
        if kind == "linear":
            import re

            assert re.findall(rb"[-0-9.]+%", first) == re.findall(rb"[-0-9.]+%", second)
        else:
            assert first.split(b",", 1)[1] == second.split(b",", 1)[1]


def click_picker_button(window, label):
    picker_field(
        window, label, slint_testing.AccessibleRole.Button
    ).invoke_accessible_default_action()


@pytest.mark.parametrize("kind", ["linear", "radial", "conic"])
def test_picker_crossing_keeps_canvas_identity_and_orders_rows(
    editor_binary, editor_environment, tmp_path, kind
):
    from source_snapshot import SourceSnapshot
    from test_linear_gradient_canvas import center, control, shifted
    from ui_driver import press_key

    units = 360 if kind == "conic" else 100
    suffix = "deg" if kind == "conic" else "%"
    prefix = {"linear": "90deg", "radial": "circle", "conic": "from 0deg"}[kind]
    file = gradient_document(
        tmp_path,
        f"@{kind}-gradient({prefix}, red 0{suffix}, #0000ff80 {units * 0.4}{suffix}, lime {units * 0.6}{suffix}, white {units}{suffix})",
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        role = slint_testing.AccessibleRole.Slider
        left = center(control(window, "Gradient stop 1", role))
        right = center(control(window, "Gradient stop 4", role))
        start = center(control(window, "Gradient stop 2", role))
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        for position in [0.65, 0.85, 0.3, 0.75]:
            point = shifted(left, x=(right.x - left.x) * position)
            window.dispatch_event(slint_testing.PointerMoveEvent(point))
            assert float(
                picker_field(window, "Stop 2 position").accessible_value
            ) == pytest.approx(position * units, abs=0.01)
        window.dispatch_event(slint_testing.PointerReleaseEvent(point, button))
        assert (
            picker_field(window, "Stop 3 position").absolute_position.y
            < picker_field(window, "Stop 2 position").absolute_position.y
        )
        click_picker_button(window, "Edit stop 2 color")
        assert picker_field(window, "Hex color").accessible_value == "#0000ff80"
        click_picker_button(window, "Close Stop color")
        click_picker_button(window, "Gradient stop 2")
        press_key(window, keys.RightArrow)
        assert float(
            picker_field(window, "Stop 2 position").accessible_value
        ) == pytest.approx(units * 0.75 + 1, abs=0.01)
        click_picker_button(window, "Remove stop 1")
        click_picker_button(window, "Edit stop 1 color")
        assert picker_field(window, "Hex color").accessible_value == "#0000ff80"
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_stop_interactions_preserve_color_identity(
    editor_binary, editor_environment, tmp_path
):
    from source_snapshot import SourceSnapshot
    from test_linear_gradient_canvas import center, control, gesture, shifted
    from ui_driver import press_key

    file = gradient_document(
        tmp_path, "@linear-gradient(90deg, red 0%, #0000ff80 50%, white 100%)"
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        left = center(control(window, "Gradient start"))
        right = center(control(window, "Gradient end"))
        insertion = shifted(left, x=(right.x - left.x) * 0.25)
        for _ in range(2):
            gesture(window, insertion, insertion)
        control(window, "Gradient stop 4")
        click_picker_button(window, "Edit stop 2 color")
        assert picker_field(window, "Hex color").accessible_value == "#aa0055c0"
        picker_field(window, "Hex color").accessible_value = "#00ff00b0"
        click_picker_button(window, "Close Stop color")
        original.assert_unchanged_now()
        start = center(control(window, "Gradient stop 2"))
        gesture(window, start, shifted(start, x=(right.x - left.x) * 0.5))
        click_picker_button(window, "Edit stop 2 color")
        assert picker_field(window, "Hex color").accessible_value == "#00ff00b0"
        click_picker_button(window, "Close Stop color")
        click_picker_button(window, "Gradient stop 1")
        press_key(window, keys.Delete)
        click_picker_button(window, "Gradient stop 3")
        press_key(window, keys.Delete)
        original.assert_unchanged_now()
        click_picker_button(window, "Close Custom")
        saved = wait_until(
            lambda: file.read_bytes()
            if file.read_bytes() != original.sources[Path(file.name)]
            else None
        )
        original.wait_for_applied(saved, file.name)
        assert b"#0000ff80 50%, #00ff00b0 75%" in saved
        select_outline_row(window, "fill")
        open_gradient(window)
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
            rotate_conic(window, 0, 37)
            set_picker_mode(window, "Gradient type", "Linear")
            click_picker_button(window, "Solid")
            assert picker_field(window, "Hex color").accessible_value == "#12345680"
            click_picker_button(window, "Gradient")
            set_picker_mode(window, "Gradient type", "Radial")
            assert radial_geometry(window) == pytest.approx((40, 60, 90), abs=0.001)
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
        assert radial_geometry(window) == pytest.approx((40, 60, 90), abs=0.001)
        click_picker_button(window, "Edit stop 2 color")
        assert picker_field(window, "Hex color").accessible_value == "#12345680"


def test_recent_gradient_resets_custom_geometry_initialization(
    editor_binary, editor_environment, tmp_path
):
    from source_snapshot import SourceSnapshot

    file = gradient_document(tmp_path, "@radial-gradient(circle, red 0%, blue 100%)")
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        click_picker_button(window, "Add gradient stop")
        click_picker_button(window, "Close Custom")
        saved = wait_until(
            lambda: (
                file.read_bytes()
                if file.read_bytes() != original.sources[Path(file.name)]
                else None
            )
        )
        original.wait_for_applied(saved, file.name)
        select_outline_row(window, "fill")
        open_gradient(window)
        set_radial_geometry(window, 37, 200, 95)
        recent = [
            element
            for element in window.root_element.query_descendants()
            .match_accessible_role(slint_testing.AccessibleRole.Button)
            .find_all()
            if element.accessible_label.startswith("Recent fill @radial-gradient")
        ]
        assert len(recent) == 1
        recent[0].invoke_accessible_default_action()
        assert radial_geometry(window) == pytest.approx(
            (200, 200, math.hypot(200, 200)), abs=0.001
        )


def radial_geometry(window, element_id="Gradient::fill"):
    from test_linear_gradient_canvas import center, control

    rectangle = wait_until(
        lambda: next(iter(window.find_elements_by_id(element_id)), None)
    )
    c = center(control(window, "Gradient center handle"), 35)
    r = center(control(window, "Gradient radius handle"), 35)
    return (
        c.x - rectangle.absolute_position.x,
        c.y - rectangle.absolute_position.y,
        math.hypot(r.x - c.x, r.y - c.y),
    )


def conic_geometry(window, angle=0, element_id="Gradient::fill"):
    from test_linear_gradient_canvas import center, control

    rectangle = wait_until(
        lambda: next(iter(window.find_elements_by_id(element_id)), None)
    )
    c = center(control(window, "Gradient center handle"), angle - 90)
    r = center(control(window, "Gradient rotation handle"), angle - 90)
    return (
        c.x - rectangle.absolute_position.x,
        c.y - rectangle.absolute_position.y,
        math.hypot(r.x - c.x, r.y - c.y),
    )


def set_conic_center(window, x, y, angle=0):
    from test_linear_gradient_canvas import center, control, gesture, shifted

    old_x, old_y, _ = conic_geometry(window, angle)
    c = center(control(window, "Gradient center handle"), angle - 90)
    gesture(window, c, shifted(c, x=x - old_x, y=y - old_y))


def rotate_conic(window, previous, next_angle):
    from test_conic_gradient_canvas import around
    from test_linear_gradient_canvas import center, control, gesture

    c = center(control(window, "Gradient center handle"), previous - 90)
    r = center(control(window, "Gradient rotation handle"), previous - 90)
    gesture(window, r, around(c, math.hypot(r.x - c.x, r.y - c.y), next_angle))


def set_radial_geometry(window, x, y, radius):
    from test_linear_gradient_canvas import center, control, gesture, shifted

    old_x, old_y, _ = radial_geometry(window)
    c = center(control(window, "Gradient center handle"), 35)
    gesture(window, c, shifted(c, x=x - old_x, y=y - old_y))
    c = center(control(window, "Gradient center handle"), 35)
    r = center(control(window, "Gradient radius handle"), 35)
    gesture(
        window,
        r,
        shifted(
            c,
            x=radius * math.cos(math.radians(35)),
            y=radius * math.sin(math.radians(35)),
        ),
    )
