# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
import re
from pathlib import Path

import pytest
import slint_testing
from slint_testing import keys
from source_snapshot import SourceSnapshot
from test_linear_gradient_canvas import center, click, control, gesture, shifted
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    press_key,
    press_shortcut,
    select_outline_row,
    wait_until,
)


@pytest.fixture
def conic_scene(tmp_path):
    path = tmp_path / "ConicGradientScene.slint"
    path.write_text("""export component ConicGradientScene inherits Window {
    width: 400px;
    height: 400px;
    fill := Rectangle {
        width: 200px;
        height: 200px;
        background: @conic-gradient(from 220deg, #7e3b66 0deg, #264052 198deg, #568fb8 360deg);
    }
}
""")
    return path


def open_conic(window):
    select_outline_row(window, "fill")
    click(window, "Rectangle background color picker")
    control(window, "Gradient rotation handle")
    assert not elements_with_label(window.root_element, "Gradient angle degrees")
    assert not elements_with_label(window.root_element, "Gradient center")
    control(window, "Add gradient stop")


def around(c, radius, degrees):
    angle = math.radians(degrees - 90)
    return shifted(c, x=radius * math.cos(angle), y=radius * math.sin(angle))


def stop_center(window, index, position, start=220, rotation=0):
    return center(
        control(window, f"Gradient stop {index}"),
        rotation + start - 90 + position + (90 if position >= 360 else -90),
    )


@pytest.mark.parametrize("kind", ["center", "rotation", "stop"])
def test_conic_escape_restores_gesture(
    editor_binary, editor_environment, conic_scene, tmp_path, kind
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        window = first_window(editor)
        open_conic(window)
        label = {
            "center": "Gradient center handle",
            "rotation": "Gradient rotation handle",
            "stop": "Gradient stop 2",
        }[kind]
        start = (
            stop_center(window, 2, 198)
            if kind == "stop"
            else center(control(window, label), 130)
        )
        end = shifted(start, x=25, y=-15)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        press_key(window, keys.Escape)
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        restored = (
            stop_center(window, 2, 198)
            if kind == "stop"
            else center(control(window, label), 130)
        )
        assert restored.x == pytest.approx(start.x, abs=0.001)
        assert restored.y == pytest.approx(start.y, abs=0.001)
        click(window, "Close Custom")
        original.assert_unchanged()


def test_conic_keyboard_and_seam_neighbor(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        window = first_window(editor)
        open_conic(window)
        c = center(control(window, "Gradient center handle"), 130)
        gesture(window, c, c)
        press_key(window, keys.RightArrow)
        press_shortcut(window, keys.Shift, keys.DownArrow)
        moved = center(control(window, "Gradient center handle"), 130)
        assert moved.x == pytest.approx(c.x + 1, abs=0.001)
        assert moved.y == pytest.approx(c.y + 10, abs=0.001)
        r = center(control(window, "Gradient rotation handle"), 130)
        gesture(window, r, r)
        press_key(window, keys.RightArrow)
        press_shortcut(window, keys.Shift, keys.LeftArrow)
        r = center(control(window, "Gradient rotation handle"), 121)
        expected = around(moved, 126, 211)
        assert r.x == pytest.approx(expected.x, abs=0.001)
        assert r.y == pytest.approx(expected.y, abs=0.001)
        p = stop_center(window, 2, 198, start=211)
        gesture(window, p, p)
        press_key(window, keys.RightArrow)
        press_shortcut(window, keys.Shift, keys.LeftArrow)
        assert float(
            control(
                window, "Stop 2 position", slint_testing.AccessibleRole.TextInput
            ).accessible_value
        ) == pytest.approx(189)
        p = stop_center(window, 1, 0, start=211)
        gesture(window, p, p)
        press_key(window, keys.Delete)
        press_key(window, keys.LeftArrow)
        assert float(
            control(
                window, "Stop 2 position", slint_testing.AccessibleRole.TextInput
            ).accessible_value
        ) == pytest.approx(359)
        press_key(window, keys.Backspace)
        press_key(window, keys.Delete)
        control(window, "Gradient stop 2")
        assert not elements_with_label(window.root_element, "Gradient stop 3")
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_external_edit_invalidates_conic_session(
    editor_binary, editor_environment, conic_scene
):
    from editor_sync import wait_for_source

    original = conic_scene.read_text()
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        window = first_window(editor)
        open_conic(window)
        c = center(control(window, "Gradient center handle"), 130)
        gesture(window, c, shifted(c, x=25, y=15))
        external = original.replace("#7e3b66", "#abcdef")
        conic_scene.write_text(external)
        wait_for_source(conic_scene, external.encode())
        assert not elements_with_label(window.root_element, "Gradient center handle")


@pytest.mark.parametrize("rotation", [0, 45, 90])
def test_conic_center_translation(
    editor_binary, editor_environment, conic_scene, tmp_path, rotation
):
    conic_scene.write_text(
        conic_scene.read_text().replace(
            "        width: 200px;",
            f"        transform-rotation: {rotation}deg;\n        width: 200px;",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        window = first_window(editor)
        open_conic(window)
        c = center(control(window, "Gradient center handle"), rotation + 130)
        r = center(control(window, "Gradient rotation handle"), rotation + 130)
        gesture(window, c, shifted(c, x=17, y=23))
        moved = center(control(window, "Gradient center handle"), rotation + 130)
        end = center(control(window, "Gradient rotation handle"), rotation + 130)
        assert moved.x == pytest.approx(c.x + 17, abs=0.001)
        assert moved.y == pytest.approx(c.y + 23, abs=0.001)
        assert end.x == pytest.approx(r.x + 17, abs=0.001)
        assert end.y == pytest.approx(r.y + 23, abs=0.001)
        original.assert_unchanged_now()
        press_key(window, keys.Escape)
        original.assert_unchanged()


@pytest.mark.parametrize("handle", ["endpoint", "ray"])
def test_conic_rotation_crosses_the_seam(
    editor_binary, editor_environment, conic_scene, tmp_path, handle
):
    conic_scene.write_text(
        conic_scene.read_text().replace("from 220deg", "from 350deg")
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        window = first_window(editor)
        open_conic(window)
        c = center(control(window, "Gradient center handle"), 260)
        radius = 126 if handle == "endpoint" else 70
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(
            slint_testing.PointerPressEvent(around(c, radius, 350), button)
        )
        for angle in [355, 359, 1, 7]:
            window.dispatch_event(
                slint_testing.PointerMoveEvent(around(c, radius, angle))
            )
            actual = center(control(window, "Gradient rotation handle"), angle - 90)
            expected = around(c, 126, angle)
            assert actual.x == pytest.approx(expected.x, abs=0.01)
            assert actual.y == pytest.approx(expected.y, abs=0.01)
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(around(c, radius, 7), button)
        )
        original.assert_unchanged_now()
        click(window, "Close Custom")
        saved = wait_until(
            lambda: (
                conic_scene.read_bytes()
                if conic_scene.read_bytes() != original.sources[Path(conic_scene.name)]
                else None
            )
        )
        original.wait_for_applied(saved, conic_scene.name)
        angle = re.search(rb"from ([0-9.]+)deg", saved)
        assert angle is not None
        assert float(angle.group(1)) == pytest.approx(367, abs=0.001)
        assert b" at " not in saved
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(
            original.sources[Path(conic_scene.name)], conic_scene.name
        )
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, conic_scene.name)
        open_conic(window)
        reopened = center(control(window, "Gradient rotation handle"), 277)
        expected = around(c, 126, 367)
        assert reopened.x == pytest.approx(expected.x, abs=0.01)
        assert reopened.y == pytest.approx(expected.y, abs=0.01)
        click(window, "Close Custom")
        assert conic_scene.read_bytes() == saved


def test_conic_noop_and_collapsed_rotation_do_not_write_source(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        window = first_window(editor)
        open_conic(window)
        c = center(control(window, "Gradient center handle"), 130)
        r = center(control(window, "Gradient rotation handle"), 130)
        gesture(window, r, c)
        actual = center(control(window, "Gradient rotation handle"), 130)
        assert actual.x == pytest.approx(r.x, abs=0.001)
        assert actual.y == pytest.approx(r.y, abs=0.001)
        click(window, "Close Custom")
        original.assert_unchanged()
        open_conic(window)
        click(window, "Close Custom")
        original.assert_unchanged()


def test_conic_seam_handles_and_stop_crossing(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        window = first_window(editor)
        open_conic(window)
        c = center(control(window, "Gradient center handle"), 130)
        first = stop_center(window, 1, 0)
        last = stop_center(window, 3, 360)
        assert math.hypot(first.x - c.x, first.y - c.y) == pytest.approx(148, abs=0.001)
        assert math.hypot(last.x - c.x, last.y - c.y) == pytest.approx(104, abs=0.001)
        gesture(window, first, around(c, 148, 240))
        assert float(
            control(
                window, "Stop 1 position", slint_testing.AccessibleRole.TextInput
            ).accessible_value
        ) == pytest.approx(20, abs=0.01)
        gesture(window, last, around(c, 104, 200))
        assert float(
            control(
                window, "Stop 3 position", slint_testing.AccessibleRole.TextInput
            ).accessible_value
        ) == pytest.approx(340, abs=0.01)
        start = stop_center(window, 2, 198)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        for degrees in [250, 320, 350, 280, 180, 90, 10, 60]:
            p = around(c, 148, 220 + degrees)
            window.dispatch_event(slint_testing.PointerMoveEvent(p))
            assert float(
                control(
                    window, "Stop 2 position", slint_testing.AccessibleRole.TextInput
                ).accessible_value
            ) == pytest.approx(degrees, abs=0.01)
        window.dispatch_event(slint_testing.PointerReleaseEvent(p, button))
        click(window, "Edit stop 2 color")
        assert (
            control(
                window, "Hex color", slint_testing.AccessibleRole.TextInput
            ).accessible_value
            == "#264052"
        )
        click(window, "Close Stop color")
        (tmp_path / "conic-editor.png").write_bytes(window.grab_window_as_png())
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_conic_ring_insertion(editor_binary, editor_environment, conic_scene, tmp_path):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        window = first_window(editor)
        open_conic(window)
        c = center(control(window, "Gradient center handle"), 130)
        p = around(c, 126, 220 + 90)
        gesture(window, p, p)
        gesture(window, p, p)
        control(window, "Gradient stop 4")
        assert float(
            control(
                window, "Stop 2 position", slint_testing.AccessibleRole.TextInput
            ).accessible_value
        ) == pytest.approx(90, abs=0.01)
        press_key(window, keys.Delete)
        assert not elements_with_label(window.root_element, "Gradient stop 4")
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_conic_insertion_samples_straight_alpha(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    conic_scene.write_text(
        conic_scene.read_text().replace(
            "#7e3b66 0deg, #264052 198deg, #568fb8 360deg",
            "#ff000000 0deg, #0000ff 360deg",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        window = first_window(editor)
        open_conic(window)
        c = center(control(window, "Gradient center handle"), 130)
        p = around(c, 126, 400)
        gesture(window, p, p)
        gesture(window, p, p)
        click(window, "Edit stop 2 color")
        value = control(
            window, "Hex color", slint_testing.AccessibleRole.TextInput
        ).accessible_value
        assert value in ("#80008080", "#7f008080", "#80007f7f", "#7f00807f")
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_conic_coincident_stops_keep_keyboard_focus(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    conic_scene.write_text(
        conic_scene.read_text().replace("#264052 198deg", "#264052 0deg")
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        window = first_window(editor)
        open_conic(window)
        click(window, "Gradient stop 1")
        press_key(window, keys.Tab)
        press_key(window, keys.RightArrow)
        assert float(
            control(
                window, "Stop 2 position", slint_testing.AccessibleRole.TextInput
            ).accessible_value
        ) == pytest.approx(1)
        assert float(
            control(
                window, "Stop 1 position", slint_testing.AccessibleRole.TextInput
            ).accessible_value
        ) == pytest.approx(0)
        click(window, "Edit stop 2 color")
        field = control(window, "Hex color", slint_testing.AccessibleRole.TextInput)
        p = center(field)
        gesture(window, p, p)
        press_shortcut(window, keys.Control, "a")
        press_key(window, keys.Backspace)
        control(window, "Gradient stop 3")
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_conic_picker_and_canvas_share_selection_and_color(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        window = first_window(editor)
        open_conic(window)
        (tmp_path / "conic-picker-and-canvas.png").write_bytes(
            window.grab_window_as_png()
        )
        control(window, "Gradient stop 2", slint_testing.AccessibleRole.Slider)
        assert not elements_with_label(window.root_element, "Hex color")
        click(window, "Edit stop 2 color")
        field = control(window, "Hex color", slint_testing.AccessibleRole.TextInput)
        assert field.accessible_value == "#264052"
        click(window, "Gradient stop 1")
        assert field.accessible_value == "#7e3b66"
        click(window, "Gradient stop 2")
        field.accessible_value = "#abcdef80"
        click(window, "Close Stop color")
        control(
            window, "Stop 2 position", slint_testing.AccessibleRole.TextInput
        ).accessible_value = "162"
        c = center(control(window, "Gradient center handle"), 130)
        actual = stop_center(window, 2, 162)
        expected = around(c, 148, 220 + 162)
        assert actual.x == pytest.approx(expected.x, abs=0.001)
        assert actual.y == pytest.approx(expected.y, abs=0.001)
        click(window, "Edit stop 2 color")
        assert (
            control(
                window, "Hex color", slint_testing.AccessibleRole.TextInput
            ).accessible_value
            == "#abcdef80"
        )
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_conic_activation_from_solid(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    conic_scene.write_text(
        re.sub(r"@conic-gradient\([^;]+\)", "#7e3b66", conic_scene.read_text())
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        click(window, "Rectangle background color picker")
        assert not elements_with_label(window.root_element, "Gradient rotation handle")
        click(window, "Gradient")
        control(
            window, "Gradient type", slint_testing.AccessibleRole.Combobox
        ).accessible_value = "Conic"
        control(window, "Gradient rotation handle")
        assert not elements_with_label(window.root_element, "Gradient angle degrees")
        control(window, "Edit stop 1 color")
        click(window, "Solid")
        assert not elements_with_label(window.root_element, "Gradient rotation handle")
        click(window, "Gradient")
        control(window, "Gradient rotation handle")
        press_key(window, keys.Escape)
        original.assert_unchanged()
