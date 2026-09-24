# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
import re
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center_canvas_selection, zoom_canvas
from editor_sync import wait_for_source
from gradient_interactions import center, click, control, gesture, open_radial, shifted
from slint_testing import keys
from source_snapshot import SourceSnapshot, wait_for_source_change
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    press_key,
    press_shortcut,
    select_outline_row,
)


def test_radial_activation_preserves_the_actual_picker(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    radial_scene.write_text(
        re.sub(r"@radial-gradient\([^;]+\)", "#7e3b66", radial_scene.read_text())
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        assert not elements_with_label(window.root_element, "Gradient center handle")
        click(window, "Rectangle background color picker")
        assert not elements_with_label(window.root_element, "Gradient center handle")
        click(window, "Gradient")
        control(
            window, "Gradient type", slint_testing.AccessibleRole.Combobox
        ).accessible_value = "Radial"
        control(window, "Gradient center handle")
        control(window, "Gradient radius X and rectangle rotation handle")
        control(window, "Gradient radius Y handle")
        control(window, "Gradient stop 1", slint_testing.AccessibleRole.Slider)
        control(window, "Edit stop 1 color")
        assert not elements_with_label(window.root_element, "Hex color")
        (tmp_path / "radial-picker-and-canvas.png").write_bytes(
            window.grab_window_as_png()
        )
        click(window, "Solid")
        assert not elements_with_label(window.root_element, "Gradient center handle")
        control(window, "Hex color", slint_testing.AccessibleRole.TextInput)
        click(window, "Gradient")
        control(window, "Gradient center handle")
        press_key(window, keys.Escape)
        original.assert_unchanged()


@pytest.mark.parametrize("rotation", [0, 45, 90])
@pytest.mark.parametrize("handle", ["Gradient center handle", "Gradient axis"])
def test_radial_translation(
    editor_binary, editor_environment, radial_scene, tmp_path, rotation, handle
):
    radial_scene.write_text(
        radial_scene.read_text().replace(
            "        width: 200px;",
            f"        transform-rotation: {rotation}deg;\n        width: 200px;",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        c = center(control(window, "Gradient center handle"), rotation)
        r = center(
            control(window, "Gradient radius X and rectangle rotation handle"), rotation
        )
        p = (
            c
            if handle == "Gradient center handle"
            else shifted(c, x=(r.x - c.x) * 0.7, y=(r.y - c.y) * 0.7)
        )
        gesture(window, p, shifted(p, x=17, y=23))
        after = center(control(window, "Gradient center handle"), rotation)
        end = center(
            control(window, "Gradient radius X and rectangle rotation handle"), rotation
        )
        assert after.x == pytest.approx(c.x + 17, abs=0.01)
        assert after.y == pytest.approx(c.y + 23, abs=0.01)
        assert end.x == pytest.approx(r.x + 17, abs=0.01)
        assert end.y == pytest.approx(r.y + 23, abs=0.01)
        original.assert_unchanged_now()
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_ellipse_endpoint_cancel_restores_radius_and_rectangle_rotation(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    radial_scene.write_text(
        radial_scene.read_text().replace(
            "@radial-gradient(circle,", "@radial-gradient(ellipse 130px 70px,"
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        handle = "Gradient radius X and rectangle rotation handle"
        start = center(control(window, handle))
        end = shifted(start, x=30, y=40)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        rotation = math.degrees(math.atan2(40, 160))
        c = center(control(window, "Gradient center handle"), rotation)
        x = center(control(window, handle), rotation)
        y = center(control(window, "Gradient radius Y handle"), rotation + 90)
        assert math.hypot(x.x - c.x, x.y - c.y) == pytest.approx(
            math.hypot(y.x - c.x, y.y - c.y), abs=0.01
        )
        press_key(window, keys.Escape)
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
        restored = center(control(window, handle))
        restored_y = center(control(window, "Gradient radius Y handle"), 90)
        assert restored.x == pytest.approx(start.x, abs=0.01)
        assert restored.y == pytest.approx(start.y, abs=0.01)
        assert restored_y.y - c.y == pytest.approx(70, abs=0.01)
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_off_center_ellipse_endpoint_tracks_pointer_while_rotating_element(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    radial_scene.write_text(
        radial_scene.read_text().replace(
            "@radial-gradient(circle,",
            "@radial-gradient(ellipse 130px 70px at 70px 80px,",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        handle = "Gradient radius X and rectangle rotation handle"
        start = center(control(window, handle))
        gesture(window, start, shifted(start, x=10, y=30))
        rotation = math.degrees(math.atan2(10, 110) - math.atan2(-20, math.sqrt(11800)))
        end = center(control(window, handle), rotation)
        assert end.x == pytest.approx(start.x + 10, abs=0.01)
        assert end.y == pytest.approx(start.y + 30, abs=0.01)
        original.assert_unchanged_now()
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_off_center_ellipse_radius_crosses_rectangle_pivot_without_rotation(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    radial_scene.write_text(
        radial_scene.read_text().replace(
            "@radial-gradient(circle,",
            "@radial-gradient(ellipse 130px 70px at 70px 80px,",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        handle = "Gradient radius X and rectangle rotation handle"
        start = center(control(window, handle))
        gesture(window, start, shifted(start, x=-110))
        end = center(control(window, handle))
        assert end.x == pytest.approx(start.x - 110, abs=0.01)
        assert end.y == pytest.approx(start.y, abs=0.01)
        click(window, "Close Custom")
        saved = wait_for_source_change(
            radial_scene, original.sources[Path(radial_scene.name)]
        )
        assert b"ellipse 20px 70px at 70px 80px" in saved
        assert b"transform-rotation:" not in saved
        original.wait_for_applied(saved, radial_scene.name)


@pytest.mark.parametrize("percent", [50, 100, 200])
def test_radial_radius_save_reopen_and_history(
    editor_binary, editor_environment, radial_scene, tmp_path, percent
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        zoom_canvas(window, percent)
        center_canvas_selection(window)
        open_radial(window)
        c = center(control(window, "Gradient center handle"))
        r = center(control(window, "Gradient radius Y handle"), 90)
        gesture(window, r, shifted(c, y=percent), shift=True)
        click(window, "Close Custom")
        saved = wait_for_source_change(
            radial_scene, original.sources[Path(radial_scene.name)]
        )
        radius = re.search(rb"circle ([0-9.]+)px", saved)
        assert radius is not None
        assert float(radius.group(1)) == pytest.approx(100, abs=0.001)
        original.wait_for_applied(saved, radial_scene.name)
        assert b" at " not in saved
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(
            original.sources[Path(radial_scene.name)], radial_scene.name
        )
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, radial_scene.name)
        open_radial(window)
        c = center(control(window, "Gradient center handle"))
        r = center(control(window, "Gradient radius X and rectangle rotation handle"))
        assert math.hypot(r.x - c.x, r.y - c.y) == pytest.approx(percent, abs=0.001)


@pytest.mark.parametrize("rotation", [0, 45])
def test_ellipse_radii_save_and_reopen(
    editor_binary, editor_environment, radial_scene, tmp_path, rotation
):
    radial_scene.write_text(
        radial_scene.read_text()
        .replace("@radial-gradient(circle,", "@radial-gradient(ellipse 130px 70px,")
        .replace(
            "        width: 200px;",
            f"        transform-rotation: {rotation}deg;\n        width: 200px;",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        assert not elements_with_label(window.root_element, "Gradient shape")
        if rotation == 0:
            (tmp_path / "ellipse-gradient-editor.png").write_bytes(
                window.grab_window_as_png()
            )
        angle = math.radians(rotation)
        x = center(
            control(window, "Gradient radius X and rectangle rotation handle"), rotation
        )
        gesture(window, x, shifted(x, x=20 * math.cos(angle), y=20 * math.sin(angle)))
        y = center(control(window, "Gradient radius Y handle"), rotation + 90)
        gesture(window, y, shifted(y, x=-25 * math.sin(angle), y=25 * math.cos(angle)))
        click(window, "Close Custom")
        saved = wait_for_source_change(
            radial_scene, original.sources[Path(radial_scene.name)]
        )
        radii = re.search(rb"ellipse ([0-9.]+)px ([0-9.]+)px", saved)
        assert radii is not None
        assert (float(radii.group(1)), float(radii.group(2))) == pytest.approx(
            (150, 95), abs=0.001
        )
        original.wait_for_applied(saved, radial_scene.name)
        open_radial(window)
        c = center(control(window, "Gradient center handle"), rotation)
        x = center(
            control(window, "Gradient radius X and rectangle rotation handle"), rotation
        )
        y = center(control(window, "Gradient radius Y handle"), rotation + 90)
        assert math.hypot(x.x - c.x, x.y - c.y) == pytest.approx(150, abs=0.01)
        assert math.hypot(y.x - c.x, y.y - c.y) == pytest.approx(95, abs=0.01)


@pytest.mark.parametrize("initial_rotation", [0, 45])
def test_ellipse_endpoint_rotates_rectangle_and_saves_one_edit(
    editor_binary, editor_environment, radial_scene, tmp_path, initial_rotation
):
    radial_scene.write_text(
        radial_scene.read_text()
        .replace("@radial-gradient(circle,", "@radial-gradient(ellipse 130px 70px,")
        .replace(
            "        width: 200px;",
            f"        transform-rotation: {initial_rotation}deg;\n        width: 200px;",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        handle = "Gradient radius X and rectangle rotation handle"
        start = center(control(window, handle), initial_rotation)
        angle = math.radians(initial_rotation)
        dx = 30 * math.cos(angle) - 40 * math.sin(angle)
        dy = 30 * math.sin(angle) + 40 * math.cos(angle)
        gesture(window, start, shifted(start, x=dx, y=dy))
        control(window, handle)
        control(window, "Gradient radius Y handle")
        expected_rotation = initial_rotation + math.degrees(math.atan2(40, 160))
        expected_radius = math.hypot(160, 40)
        x = center(control(window, handle), expected_rotation)
        c = center(control(window, "Gradient center handle"), expected_rotation)
        assert math.hypot(x.x - c.x, x.y - c.y) == pytest.approx(
            expected_radius, abs=0.01
        )
        if initial_rotation == 0:
            (tmp_path / "ellipse-rotation-preview.png").write_bytes(
                window.grab_window_as_png()
            )
        original.assert_unchanged_now()
        click(window, "Close Custom")
        saved = wait_for_source_change(
            radial_scene, original.sources[Path(radial_scene.name)]
        )
        rotation = re.search(rb"transform-rotation: ([0-9.]+)deg", saved)
        radii = re.search(rb"ellipse ([0-9.]+)px ([0-9.]+)px", saved)
        assert rotation is not None and radii is not None
        assert float(rotation.group(1)) == pytest.approx(expected_rotation, abs=0.01)
        assert (float(radii.group(1)), float(radii.group(2))) == pytest.approx(
            (expected_radius, 70), abs=0.01
        )
        original.wait_for_applied(saved, radial_scene.name)
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(
            original.sources[Path(radial_scene.name)], radial_scene.name
        )
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, radial_scene.name)
        open_radial(window)
        assert center(control(window, handle), expected_rotation).x == pytest.approx(
            x.x, abs=0.01
        )


def test_ellipse_handle_rotation_keeps_picker_and_saves_without_radius_change(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    radial_scene.write_text(
        radial_scene.read_text().replace(
            "@radial-gradient(circle,", "@radial-gradient(ellipse 130px 70px,"
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        assert not elements_with_label(window.root_element, "Gradient shape")
        assert not elements_with_label(
            window.root_element, "Rectangle rotation degrees in fill editor"
        )
        handle = "Gradient radius X and rectangle rotation handle"
        c = center(control(window, "Gradient center handle"))
        x = center(control(window, handle))
        gesture(
            window,
            x,
            shifted(
                c,
                x=130 * math.cos(math.radians(30)),
                y=130 * math.sin(math.radians(30)),
            ),
        )
        c = center(control(window, "Gradient center handle"), 30)
        x = center(control(window, handle), 30)
        assert x.x - c.x == pytest.approx(130 * math.cos(math.radians(30)), abs=0.01)
        assert x.y - c.y == pytest.approx(130 * math.sin(math.radians(30)), abs=0.01)
        original.assert_unchanged_now()
        control(window, "Close Custom")
        click(window, "Close Custom")
        saved = wait_for_source_change(
            radial_scene, original.sources[Path(radial_scene.name)]
        )
        rotation = re.search(rb"transform-rotation: ([0-9.]+)deg", saved)
        assert rotation is not None
        assert float(rotation.group(1)) == pytest.approx(30, abs=0.01)
        assert b"@radial-gradient(ellipse 130px 70px," in saved
        original.wait_for_applied(saved, radial_scene.name)
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(
            original.sources[Path(radial_scene.name)], radial_scene.name
        )


def test_explicit_equal_radius_ellipse_survives_stop_edit(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    radial_scene.write_text(
        radial_scene.read_text().replace(
            "@radial-gradient(circle,", "@radial-gradient(ellipse 130px 130px,"
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        assert not elements_with_label(window.root_element, "Gradient shape")
        control(window, "Gradient radius X and rectangle rotation handle")
        control(window, "Gradient radius Y handle")
        control(
            window, "Stop 2 position", slint_testing.AccessibleRole.TextInput
        ).accessible_value = "50"
        click(window, "Close Custom")
        saved = wait_for_source_change(
            radial_scene, original.sources[Path(radial_scene.name)]
        )
        assert b"@radial-gradient(ellipse 130px 130px," in saved
        original.wait_for_applied(saved, radial_scene.name)
        open_radial(window)
        control(window, "Gradient radius Y handle")


@pytest.mark.parametrize("axis,expected", [("x", 150), ("y", 90)])
def test_shift_resize_either_ellipse_axis_makes_circle(
    editor_binary, editor_environment, radial_scene, tmp_path, axis, expected
):
    radial_scene.write_text(
        radial_scene.read_text().replace(
            "@radial-gradient(circle,", "@radial-gradient(ellipse 130px 70px,"
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        label = (
            "Gradient radius X and rectangle rotation handle"
            if axis == "x"
            else "Gradient radius Y handle"
        )
        start = center(control(window, label), 0 if axis == "x" else 90)
        gesture(
            window,
            start,
            shifted(start, x=20 if axis == "x" else 0, y=20 if axis == "y" else 0),
            shift=True,
        )
        control(window, "Gradient radius X and rectangle rotation handle")
        control(window, "Gradient radius Y handle")
        original.assert_unchanged_now()
        click(window, "Close Custom")
        saved = wait_for_source_change(
            radial_scene, original.sources[Path(radial_scene.name)]
        )
        radius = re.search(rb"@radial-gradient\(circle ([0-9.]+)px", saved)
        assert radius is not None
        assert float(radius.group(1)) == pytest.approx(expected, abs=0.001)
        original.wait_for_applied(saved, radial_scene.name)
        open_radial(window)
        c = center(control(window, "Gradient center handle"))
        x = center(control(window, "Gradient radius X and rectangle rotation handle"))
        y = center(control(window, "Gradient radius Y handle"), 90)
        assert x.x - c.x == pytest.approx(expected, abs=0.001)
        assert y.y - c.y == pytest.approx(expected, abs=0.001)


@pytest.mark.parametrize("axis", ["x", "y"])
def test_dragging_circle_radius_creates_ellipse(
    editor_binary, editor_environment, radial_scene, tmp_path, axis
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        label = (
            "Gradient radius X and rectangle rotation handle"
            if axis == "x"
            else "Gradient radius Y handle"
        )
        start = center(control(window, label), 0 if axis == "x" else 90)
        gesture(
            window,
            start,
            shifted(start, x=20 if axis == "x" else 0, y=20 if axis == "y" else 0),
        )
        click(window, "Close Custom")
        saved = wait_for_source_change(
            radial_scene, original.sources[Path(radial_scene.name)]
        )
        radii = re.search(rb"ellipse ([0-9.]+)px ([0-9.]+)px", saved)
        assert radii is not None
        default_radius = math.hypot(200, 200) / 2
        expected = (
            (default_radius + 20, default_radius)
            if axis == "x"
            else (default_radius, default_radius + 20)
        )
        assert (float(radii.group(1)), float(radii.group(2))) == pytest.approx(
            expected, abs=0.001
        )
        original.wait_for_applied(saved, radial_scene.name)


def test_ellipse_handles_keyboard_and_drag_cancel(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    radial_scene.write_text(
        radial_scene.read_text().replace(
            "@radial-gradient(circle,", "@radial-gradient(ellipse 130px 70px,"
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        click(window, "Gradient radius X and rectangle rotation handle")
        press_key(window, keys.LeftArrow)
        press_shortcut(window, keys.Shift, keys.RightArrow)
        click(window, "Gradient radius Y handle")
        press_key(window, keys.UpArrow)
        press_shortcut(window, keys.Shift, keys.DownArrow)
        c = center(control(window, "Gradient center handle"))
        x = center(control(window, "Gradient radius X and rectangle rotation handle"))
        y = center(control(window, "Gradient radius Y handle"), 90)
        assert x.x - c.x == pytest.approx(148, abs=0.001)
        assert y.y - c.y == pytest.approx(148, abs=0.001)
        target = shifted(y, y=25)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(y, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(target))
        press_key(window, keys.Escape)
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        restored = center(control(window, "Gradient radius Y handle"), 90)
        restored_x = center(
            control(window, "Gradient radius X and rectangle rotation handle")
        )
        assert restored.y == pytest.approx(y.y, abs=0.001)
        assert restored_x.x == pytest.approx(x.x, abs=0.001)
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_ellipse_center_and_stop_save_with_history(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    radial_scene.write_text(
        radial_scene.read_text().replace(
            "@radial-gradient(circle,", "@radial-gradient(ellipse 130px 70px,"
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        c = center(control(window, "Gradient center handle"))
        gesture(window, c, shifted(c, x=17, y=23))
        stop = center(control(window, "Gradient stop 2"))
        gesture(window, stop, shifted(stop, x=19.5))
        click(window, "Close Custom")
        saved = wait_for_source_change(
            radial_scene, original.sources[Path(radial_scene.name)]
        )
        assert b"ellipse 130px 70px at 117px 123px" in saved
        assert b"#264052 60%" in saved
        original.wait_for_applied(saved, radial_scene.name)
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(
            original.sources[Path(radial_scene.name)], radial_scene.name
        )
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, radial_scene.name)
        open_radial(window)
        moved = center(control(window, "Gradient center handle"))
        assert moved.x == pytest.approx(c.x + 17, abs=0.001)
        assert moved.y == pytest.approx(c.y + 23, abs=0.001)


def test_radial_endpoint_rotation_saves_rectangle_and_noop_does_not_write_source(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        c = center(control(window, "Gradient center handle"))
        r = center(control(window, "Gradient radius X and rectangle rotation handle"))
        radius = math.hypot(r.x - c.x, r.y - c.y)
        gesture(
            window,
            r,
            shifted(
                c,
                x=radius * math.cos(math.radians(30)),
                y=radius * math.sin(math.radians(30)),
            ),
        )
        click(window, "Close Custom")
        saved = wait_for_source_change(
            radial_scene, original.sources[Path(radial_scene.name)]
        )
        rotation = re.search(rb"transform-rotation: ([0-9.]+)deg", saved)
        assert rotation is not None
        assert float(rotation.group(1)) == pytest.approx(30, abs=0.001)
        assert b"@radial-gradient(circle," in saved
        original.wait_for_applied(saved, radial_scene.name)
        open_radial(window)
        click(window, "Close Custom")
        original.wait_for_applied(saved, radial_scene.name)


def test_radial_stops_cross_insert_delete_and_color(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        start = center(control(window, "Gradient stop 2"))
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        for distance in [40, 100, -80, 30]:
            p = shifted(
                start,
                x=distance,
            )
            window.dispatch_event(slint_testing.PointerMoveEvent(p))
            actual = center(control(window, "Gradient stop 2"))
            assert actual.x == pytest.approx(p.x, abs=0.001)
            assert actual.y == pytest.approx(p.y, abs=0.001)
        window.dispatch_event(slint_testing.PointerReleaseEvent(p, button))
        click(window, "Edit stop 2 color")
        field = control(window, "Hex color", slint_testing.AccessibleRole.TextInput)
        assert field.accessible_value == "#264052"
        field.accessible_value = "#abcdef80"
        click(window, "Close Stop color")
        c = center(control(window, "Gradient center handle"))
        r = center(control(window, "Gradient radius X and rectangle rotation handle"))
        p = shifted(c, x=(r.x - c.x) * 0.3, y=(r.y - c.y) * 0.3)
        gesture(window, p, p)
        gesture(window, p, p)
        control(window, "Gradient stop 4")
        press_key(window, keys.Delete)
        assert not elements_with_label(window.root_element, "Gradient stop 4")
        click(window, "Gradient stop 2")
        press_key(window, keys.Delete)
        press_key(window, keys.Delete)
        control(window, "Gradient center handle")
        control(window, "Gradient stop 2")
        (tmp_path / "radial-gradient-editor.png").write_bytes(
            window.grab_window_as_png()
        )
        press_key(window, keys.Escape)
        original.assert_unchanged()
