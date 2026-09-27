# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
import re
from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from gradient_interactions import (
    around,
    center,
    gesture,
    gradient_document,
    open_gradient,
    picker_field,
    shifted,
)
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once, wait_for_source_change
from ui_driver import (
    first_window,
    launch_editor,
    select_outline_row,
    wait_until,
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
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        rectangle = wait_until(
            lambda: next(iter(window.get_by_id("LayoutGradient::fill").all()), None)
        )
        assert rectangle.size.width == pytest.approx(400)
        assert rectangle.size.height == pytest.approx(400)

        window.get_by_role(
            slint_testing.AccessibleRole.Button,
            name="Rectangle background color picker",
        ).activate()
        geometry = (
            radial_geometry(window, "LayoutGradient::fill")
            if kind == "radial"
            else conic_geometry(window, element_id="LayoutGradient::fill")
        )
        assert geometry == pytest.approx(
            (200, 200, math.hypot(200, 200) if kind == "radial" else 160), abs=0.001
        )
        window.get_by_role("button", name="Close Custom").activate()
        assert source_file.read_text() == source


@pytest.mark.parametrize("kind", ["linear", "radial", "conic"])
@pytest.mark.parametrize("target", ["text", "root"])
def test_non_canvas_gradient_keeps_numeric_geometry(
    editor_binary, editor_environment, tmp_path, kind, target
):

    prefix = {"linear": "0deg", "radial": "circle", "conic": "from 0deg"}[kind]
    stops = "red 0deg, blue 360deg" if kind == "conic" else "red 0%, blue 100%"
    file = tmp_path / "TextGradient.slint"
    file.write_text(f"""export component TextGradient inherits Window {{
    width: 400px;
    height: 400px;
    background: @{kind}-gradient({prefix}, {stops});
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
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        if target == "text":
            select_outline_row(window, "label")
        else:
            window.get_by_role("list", name="Current file outline").get_by_role(
                "list-item"
            ).nth(0).activate()
        picker = (
            "Text color color picker"
            if target == "text"
            else "Root background color picker"
        )
        window.get_by_role("button", name=picker).activate()
        assert not window.get_by_accessible_name("Gradient center handle").all()
        assert not window.get_by_accessible_name("Gradient start").all()
        if kind != "radial":
            picker_field(window, "Gradient angle degrees").accessible_value = "36"
        if kind != "linear":
            set_picker_mode(window, "Gradient center", "Custom")
            picker_field(window, "Gradient center X").accessible_value = "37"
            picker_field(window, "Gradient center Y").accessible_value = "61"
        if kind == "radial":
            set_picker_mode(window, "Gradient radius mode", "Custom")
            picker_field(window, "Gradient radius").accessible_value = "95"
        labels = [
            text.accessible_label
            for text in window.root_element.query_descendants()
            .match_accessible_role(slint_testing.AccessibleRole.Text)
            .find_all()
        ]
        assert "px" not in labels
        assert "X / Y px" not in labels
        window.get_by_role("button", name="Close Custom").activate()
        geometry = {
            "linear": "36deg",
            "radial": "circle 95px at 37px 61px",
            "conic": "from 36deg at 37px 61px",
        }[kind]
        saved_stops = (
            "#ff0000 0deg, #0000ff 360deg"
            if kind == "conic"
            else "#ff0000 0%, #0000ff 100%"
        )
        property_name = "color" if target == "text" else "background"
        saved = replace_once(
            original.sources[Path(file.name)],
            f"{property_name}: @{kind}-gradient({prefix}, {stops});".encode(),
            f"{property_name}: @{kind}-gradient({geometry}, {saved_stops});".encode(),
        )
        original.wait_for_applied(saved, file.name)
        window.get_by_role("button", name=picker).activate()
        window.get_by_role("button", name="Add gradient stop").activate()
        window.keyboard.press(keys.Escape)
        assert file.read_bytes() == saved


def set_picker_mode(window, label, value):
    picker_field(
        window, label, slint_testing.AccessibleRole.Combobox
    ).accessible_value = value


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
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        stop = picker_field(
            window, "Gradient stop 1", slint_testing.AccessibleRole.Slider
        )
        assert stop.size.width == 40
        assert stop.size.height == 40
        (tmp_path / "picker-stop-markers.png").write_bytes(window.screenshot())


@pytest.mark.parametrize("loaded_custom", [False, True])
def test_custom_geometry_survives_mode_changes(
    editor_binary, editor_environment, tmp_path, loaded_custom
):

    expression = (
        "@radial-gradient(circle 95px at 37px 61px, red 0%, blue 100%)"
        if loaded_custom
        else "@radial-gradient(circle, red 0%, blue 100%)"
    )
    file = gradient_document(tmp_path, expression)
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        wait_for_source(file, file.read_bytes())
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
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


@pytest.mark.parametrize("kind", ["linear", "radial", "conic"])
def test_stop_precision_survives_save_and_reopen(
    editor_binary, editor_environment, tmp_path, kind
):

    prefix = {"linear": "90deg", "radial": "circle", "conic": "from 0deg"}[kind]
    unit = "deg" if kind == "conic" else "%"
    expression = f"@{kind}-gradient({prefix}, #ff0000 0{unit} - 12.345678{unit}, #00ff0080 33.333333{unit}, blue 33.333333{unit}, white 123.456789{unit})"
    file = gradient_document(tmp_path, expression)
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")

        def save():
            window.get_by_role("button", name="Close Custom").activate()
            content = wait_for_source_change(file, snapshot.sources[Path(file.name)])
            snapshot.wait_for_applied(content, file.name)
            return content

        open_gradient(window)
        if kind != "linear":
            picker_field(window, "Stop 2 position").accessible_value = "33.333333"
        if kind == "linear":
            window.get_by_role("button", name="Edit stop 1 color").activate()
            picker_field(window, "Hex color").accessible_value = "#ff0100"
        elif kind == "radial":
            window.get_by_role("button", name="Gradient center handle").activate()

            window.keyboard.press(keys.RightArrow)
        else:
            rotate_conic(window, 0, 45)
        first = save()
        assert b"33.33%" not in first
        assert b"33.3333" in first
        select_outline_row(window, "fill")
        open_gradient(window)
        if kind == "linear":
            window.get_by_role("button", name="Edit stop 1 color").activate()
            picker_field(window, "Hex color").accessible_value = "#ff0200"
        elif kind == "radial":
            window.get_by_role("button", name="Gradient center handle").activate()
            window.keyboard.press(keys.RightArrow)
        else:
            rotate_conic(window, 45, 46)
        window.get_by_role("button", name="Close Custom").activate()
        second = wait_for_source_change(file, first)
        snapshot.wait_for_applied(second, file.name)
        if kind == "linear":
            assert re.findall(rb"[-0-9.]+%", first) == re.findall(rb"[-0-9.]+%", second)
        else:
            assert first.split(b",", 1)[1] == second.split(b",", 1)[1]


@pytest.mark.parametrize("kind", ["linear", "radial", "conic"])
def test_picker_crossing_keeps_canvas_identity_and_orders_rows(
    editor_binary, editor_environment, tmp_path, kind
):

    units = 360 if kind == "conic" else 100
    suffix = "deg" if kind == "conic" else "%"
    prefix = {"linear": "90deg", "radial": "circle", "conic": "from 0deg"}[kind]
    file = gradient_document(
        tmp_path,
        f"@{kind}-gradient({prefix}, red 0{suffix}, #0000ff80 {units * 0.4}{suffix}, lime {units * 0.6}{suffix}, white {units}{suffix})",
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        role = slint_testing.AccessibleRole.Slider
        left = center(window.get_by_role(role, name="Gradient stop 1").resolve())
        right = center(window.get_by_role(role, name="Gradient stop 4").resolve())
        start = center(window.get_by_role(role, name="Gradient stop 2").resolve())
        window.pointer.press_at(start)
        for position in [0.65, 0.85, 0.3, 0.75]:
            point = shifted(left, x=(right.x - left.x) * position)
            window.pointer.move_to(point)
            assert float(
                picker_field(window, "Stop 2 position").accessible_value
            ) == pytest.approx(position * units, abs=0.01)
        window.pointer.release_at(point)
        assert (
            picker_field(window, "Stop 3 position").absolute_position.y
            < picker_field(window, "Stop 2 position").absolute_position.y
        )
        window.get_by_role("button", name="Edit stop 2 color").activate()
        assert picker_field(window, "Hex color").accessible_value == "#0000ff80"
        window.get_by_role("button", name="Close Stop color").activate()
        window.get_by_role("button", name="Gradient stop 2").activate()
        window.keyboard.press(keys.RightArrow)
        assert float(
            picker_field(window, "Stop 2 position").accessible_value
        ) == pytest.approx(units * 0.75 + 1, abs=0.01)
        window.get_by_role("button", name="Remove stop 1").activate()
        window.get_by_role("button", name="Edit stop 1 color").activate()
        assert picker_field(window, "Hex color").accessible_value == "#0000ff80"
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


@pytest.mark.parametrize("insert", [False, True])
def test_picker_pointer_cancel_restores_stops(
    editor_binary, editor_environment, tmp_path, insert
):

    file = gradient_document(
        tmp_path, "@linear-gradient(90deg, red 0%, blue 50%, white 100%)"
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        role = slint_testing.AccessibleRole.Slider
        left = center(window.get_by_role(role, name="Gradient stop 1").resolve())
        right = center(window.get_by_role(role, name="Gradient stop 3").resolve())
        start = shifted(left, x=(right.x - left.x) * (0.25 if insert else 0.5))
        window.pointer.press_at(start)
        window.pointer.move_to(right)
        window.pointer.exit()
        for index, position in enumerate([0, 50, 100], 1):
            assert (
                float(picker_field(window, f"Stop {index} position").accessible_value)
                == position
            )
        window.get_by_role("button", name="Close Custom").activate()
        original.assert_unchanged()


def test_stop_interactions_preserve_color_identity(
    editor_binary, editor_environment, tmp_path
):

    file = gradient_document(
        tmp_path, "@linear-gradient(90deg, red 0%, #0000ff80 50%, white 100%)"
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        left = center(window.get_by_role("button", name="Gradient start").resolve())
        right = center(window.get_by_role("button", name="Gradient end").resolve())
        insertion = shifted(left, x=(right.x - left.x) * 0.25)
        for _ in range(2):
            gesture(window, insertion, insertion)
        window.get_by_role("button", name="Gradient stop 4").resolve()
        window.get_by_role("button", name="Edit stop 2 color").activate()
        assert picker_field(window, "Hex color").accessible_value == "#aa0055c0"
        picker_field(window, "Hex color").accessible_value = "#00ff00b0"
        window.get_by_role("button", name="Close Stop color").activate()
        original.assert_unchanged_now()
        start = center(window.get_by_role("button", name="Gradient stop 2").resolve())
        gesture(window, start, shifted(start, x=(right.x - left.x) * 0.5))
        window.get_by_role("button", name="Edit stop 2 color").activate()
        assert picker_field(window, "Hex color").accessible_value == "#00ff00b0"
        window.get_by_role("button", name="Close Stop color").activate()
        window.get_by_role("button", name="Gradient stop 1").activate()
        window.keyboard.press(keys.Delete)
        window.get_by_role("button", name="Gradient stop 3").activate()
        window.keyboard.press(keys.Delete)
        original.assert_unchanged_now()
        window.get_by_role("button", name="Close Custom").activate()
        saved = wait_for_source_change(file, original.sources[Path(file.name)])
        original.wait_for_applied(saved, file.name)
        assert b"#0000ff80 50%, #00ff00b0 75%" in saved
        select_outline_row(window, "fill")
        open_gradient(window)
        window.get_by_role("button", name="Edit stop 2 color").activate()
        assert picker_field(window, "Hex color").accessible_value == "#00ff00b0"


def test_gradient_session_cancel_undo_redo_and_reopen(
    editor_binary, editor_environment, tmp_path
):

    file = gradient_document(tmp_path, "root.paint")
    source = file.read_text().replace(
        "    width: 400px;",
        "    private property <brush> paint: @radial-gradient(circle 90px at 40px 60px, red 0%, blue 100%);\n    width: 400px;",
    )
    file.write_text(source)
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        for cancel in [True, False]:
            open_gradient(window)
            picker_field(window, "No recent fills", slint_testing.AccessibleRole.Text)
            window.get_by_role("button", name="Add gradient stop").activate()
            window.get_by_role("button", name="Edit stop 2 color").activate()
            color = picker_field(window, "Hex color")
            color.accessible_value = "#12345680"
            wait_until(
                lambda color=color: (
                    color if color.accessible_value == "#12345680" else None
                )
            )
            window.get_by_role("button", name="Close Stop color").activate()
            set_picker_mode(window, "Gradient type", "Conic")
            rotate_conic(window, 0, 37)
            set_picker_mode(window, "Gradient type", "Linear")
            window.get_by_role("button", name="Solid").activate()
            assert picker_field(window, "Hex color").accessible_value == "#12345680"
            window.get_by_role("button", name="Gradient").activate()
            set_picker_mode(window, "Gradient type", "Radial")
            assert radial_geometry(window) == pytest.approx((40, 60, 90), abs=0.001)
            original.assert_unchanged_now()
            if cancel:
                window.keyboard.press(keys.Escape)
                original.assert_unchanged()
            else:
                window.get_by_role("button", name="Close Custom").activate()
        saved = wait_for_source_change(file, source.encode())
        original.wait_for_applied(saved, file.name)
        assert b"#12345680 50%" in saved
        window.keyboard.shortcut(keys.Control, "z")
        original.wait_for_applied(source.encode(), file.name)
        window.keyboard.shortcut(keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, file.name)
        select_outline_row(window, "fill")
        open_gradient(window)
        assert radial_geometry(window) == pytest.approx((40, 60, 90), abs=0.001)
        window.get_by_role("button", name="Edit stop 2 color").activate()
        assert picker_field(window, "Hex color").accessible_value == "#12345680"


def test_recent_gradient_resets_custom_geometry_initialization(
    editor_binary, editor_environment, tmp_path
):

    file = gradient_document(tmp_path, "@radial-gradient(circle, red 0%, blue 100%)")
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        window.get_by_role("button", name="Add gradient stop").activate()
        window.get_by_role("button", name="Close Custom").activate()
        saved = wait_for_source_change(file, original.sources[Path(file.name)])
        original.wait_for_applied(saved, file.name)
        select_outline_row(window, "fill")
        open_gradient(window)
        set_radial_geometry(window, 37, 200, 95)
        recent = window.get_by_role(
            "button", name=re.compile(r"^Recent fill @radial-gradient")
        )
        assert recent.count() == 1
        recent.activate()
        assert radial_geometry(window) == pytest.approx(
            (200, 200, math.hypot(200, 200)), abs=0.001
        )


def radial_geometry(window, element_id="Gradient::fill"):

    rectangle = wait_until(lambda: next(iter(window.get_by_id(element_id).all()), None))
    c = center(
        window.get_by_role("button", name="Gradient center handle").resolve(), 35
    )
    r = center(
        window.get_by_role("button", name="Gradient radius handle").resolve(), 35
    )
    return (
        c.x - rectangle.absolute_position.x,
        c.y - rectangle.absolute_position.y,
        math.hypot(r.x - c.x, r.y - c.y),
    )


def conic_geometry(window, angle=0, element_id="Gradient::fill"):

    rectangle = wait_until(lambda: next(iter(window.get_by_id(element_id).all()), None))
    c = center(
        window.get_by_role("button", name="Gradient center handle").resolve(),
        angle - 90,
    )
    r = center(
        window.get_by_role("button", name="Gradient rotation handle").resolve(),
        angle - 90,
    )
    return (
        c.x - rectangle.absolute_position.x,
        c.y - rectangle.absolute_position.y,
        math.hypot(r.x - c.x, r.y - c.y),
    )


def set_conic_center(window, x, y, angle=0):

    old_x, old_y, _ = conic_geometry(window, angle)
    c = center(
        window.get_by_role("button", name="Gradient center handle").resolve(),
        angle - 90,
    )
    gesture(window, c, shifted(c, x=x - old_x, y=y - old_y))


def rotate_conic(window, previous, next_angle):

    c = center(
        window.get_by_role("button", name="Gradient center handle").resolve(),
        previous - 90,
    )
    r = center(
        window.get_by_role("button", name="Gradient rotation handle").resolve(),
        previous - 90,
    )
    gesture(window, r, around(c, math.hypot(r.x - c.x, r.y - c.y), next_angle))


def set_radial_geometry(window, x, y, radius):

    old_x, old_y, _ = radial_geometry(window)
    c = center(
        window.get_by_role("button", name="Gradient center handle").resolve(), 35
    )
    gesture(window, c, shifted(c, x=x - old_x, y=y - old_y))
    c = center(
        window.get_by_role("button", name="Gradient center handle").resolve(), 35
    )
    r = center(
        window.get_by_role("button", name="Gradient radius handle").resolve(), 35
    )
    gesture(
        window,
        r,
        shifted(
            c,
            x=radius * math.cos(math.radians(35)),
            y=radius * math.sin(math.radians(35)),
        ),
    )
