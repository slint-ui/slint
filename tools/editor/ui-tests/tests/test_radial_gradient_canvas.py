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
    first_window,
    launch_editor,
    select_outline_row,
    press_key,
    press_shortcut,
    wait_until,
    elements_with_label,
)


def open_radial(window):
    select_outline_row(window, "fill")
    click(window, "Rectangle background color picker")
    control(window, "Gradient center handle")
    assert not elements_with_label(window.root_element, "Gradient center")
    assert not elements_with_label(window.root_element, "Gradient radius mode")
    control(window, "Add gradient stop")


def test_radial_activation_preserves_the_actual_picker(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    radial_scene.write_text(
        re.sub(r"@radial-gradient\([^;]+\)", "#7e3b66", radial_scene.read_text())
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
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
        control(window, "Gradient radius handle")
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
        window = first_window(editor)
        open_radial(window)
        c = center(control(window, "Gradient center handle"), rotation + 35)
        r = center(control(window, "Gradient radius handle"), rotation + 35)
        p = (
            c
            if handle == "Gradient center handle"
            else shifted(c, x=(r.x - c.x) * 0.7, y=(r.y - c.y) * 0.7)
        )
        gesture(window, p, shifted(p, x=17, y=23))
        after = center(control(window, "Gradient center handle"), rotation + 35)
        end = center(control(window, "Gradient radius handle"), rotation + 35)
        assert after.x == pytest.approx(c.x + 17, abs=0.01)
        assert after.y == pytest.approx(c.y + 23, abs=0.01)
        assert end.x == pytest.approx(r.x + 17, abs=0.01)
        assert end.y == pytest.approx(r.y + 23, abs=0.01)
        original.assert_unchanged_now()
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_radial_radius_save_reopen_and_history(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        window = first_window(editor)
        open_radial(window)
        c = center(control(window, "Gradient center handle"), 35)
        r = center(control(window, "Gradient radius handle"), 35)
        gesture(window, r, shifted(c, x=100))
        click(window, "Close Custom")
        saved = wait_until(
            lambda: radial_scene.read_bytes()
            if radial_scene.read_bytes() != original.sources[Path(radial_scene.name)]
            else None
        )
        assert float(
            re.search(rb"circle ([0-9.]+)px", saved).group(1)
        ) == pytest.approx(100, abs=0.001)
        original.wait_for_applied(saved, radial_scene.name)
        assert b" at " not in saved
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(
            original.sources[Path(radial_scene.name)], radial_scene.name
        )
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, radial_scene.name)
        open_radial(window)
        c = center(control(window, "Gradient center handle"), 35)
        r = center(control(window, "Gradient radius handle"), 35)
        assert math.hypot(r.x - c.x, r.y - c.y) == pytest.approx(100, abs=0.001)


def test_radial_guide_rotation_and_noop_do_not_write_source(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        window = first_window(editor)
        open_radial(window)
        c = center(control(window, "Gradient center handle"), 35)
        r = center(control(window, "Gradient radius handle"), 35)
        gesture(window, r, shifted(c, x=-math.hypot(r.x - c.x, r.y - c.y)))
        click(window, "Close Custom")
        original.assert_unchanged()
        open_radial(window)
        click(window, "Close Custom")
        original.assert_unchanged()


def test_radial_stops_cross_insert_delete_and_color(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        window = first_window(editor)
        open_radial(window)
        start = center(control(window, "Gradient stop 2"), 35)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        for distance in [40, 100, -80, 30]:
            p = shifted(
                start,
                x=distance * math.cos(math.radians(35)),
                y=distance * math.sin(math.radians(35)),
            )
            window.dispatch_event(slint_testing.PointerMoveEvent(p))
            actual = center(control(window, "Gradient stop 2"), 35)
            assert actual.x == pytest.approx(p.x, abs=0.001)
            assert actual.y == pytest.approx(p.y, abs=0.001)
        window.dispatch_event(slint_testing.PointerReleaseEvent(p, button))
        click(window, "Edit stop 2 color")
        field = control(window, "Hex color", slint_testing.AccessibleRole.TextInput)
        assert field.accessible_value == "#264052"
        field.accessible_value = "#abcdef80"
        click(window, "Close Stop color")
        c = center(control(window, "Gradient center handle"), 35)
        r = center(control(window, "Gradient radius handle"), 35)
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
