# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math

import pytest
import slint_testing
from slint_testing import keys
from source_snapshot import SourceSnapshot
from test_linear_gradient_canvas import center, click, control, gesture, shifted
from test_radial_gradient_canvas import open_radial
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    press_key,
    press_shortcut,
)


@pytest.mark.parametrize(
    "label", ["Gradient center handle", "Gradient radius handle", "Gradient stop 2"]
)
def test_escape_restores_radial_gesture(
    editor_binary, editor_environment, radial_scene, tmp_path, label
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        window = first_window(editor)
        open_radial(window)
        start = center(control(window, label), 35)
        end = shifted(start, x=25, y=-15)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        press_key(window, keys.Escape)
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        restored = center(control(window, label), 35)
        assert restored.x == pytest.approx(start.x, abs=0.001)
        assert restored.y == pytest.approx(start.y, abs=0.001)
        control(window, "Close Custom")
        click(window, "Close Custom")
        original.assert_unchanged()


def test_radial_keyboard_and_collapsed_radius(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        window = first_window(editor)
        open_radial(window)
        c = center(control(window, "Gradient center handle"), 35)
        r = center(control(window, "Gradient radius handle"), 35)
        gesture(window, r, c)
        unchanged = center(control(window, "Gradient radius handle"), 35)
        assert unchanged.x == pytest.approx(r.x, abs=0.001)
        assert unchanged.y == pytest.approx(r.y, abs=0.001)
        click(window, "Gradient center handle")
        press_key(window, keys.RightArrow)
        press_shortcut(window, keys.Shift, keys.DownArrow)
        after = center(control(window, "Gradient center handle"), 35)
        end = center(control(window, "Gradient radius handle"), 35)
        assert after.x == pytest.approx(c.x + 1, abs=0.001)
        assert after.y == pytest.approx(c.y + 10, abs=0.001)
        assert math.hypot(end.x - after.x, end.y - after.y) == pytest.approx(
            math.hypot(r.x - c.x, r.y - c.y), abs=0.001
        )
        click(window, "Gradient stop 2")
        press_key(window, keys.RightArrow)
        press_shortcut(window, keys.Shift, keys.LeftArrow)
        assert float(
            control(
                window, "Stop 2 position", slint_testing.AccessibleRole.TextInput
            ).accessible_value
        ) == pytest.approx(36)
        press_key(window, keys.Backspace)
        press_key(window, keys.Backspace)
        control(window, "Gradient stop 2")
        assert not elements_with_label(window.root_element, "Gradient stop 3")
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_external_edit_invalidates_radial_session(
    editor_binary, editor_environment, radial_scene
):
    from editor_sync import wait_for_source

    original = radial_scene.read_text()
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        window = first_window(editor)
        open_radial(window)
        c = center(control(window, "Gradient center handle"), 35)
        gesture(window, c, shifted(c, x=25, y=15))
        external = original.replace("#7e3b66", "#abcdef")
        radial_scene.write_text(external)
        wait_for_source(radial_scene, external.encode())
        assert not elements_with_label(window.root_element, "Gradient center handle")
        assert not elements_with_label(window.root_element, "Close Custom")
        assert radial_scene.read_text() == external
        open_radial(window)
        click(window, "Edit stop 1 color")
        assert (
            control(
                window, "Hex color", slint_testing.AccessibleRole.TextInput
            ).accessible_value
            == "#abcdef"
        )
