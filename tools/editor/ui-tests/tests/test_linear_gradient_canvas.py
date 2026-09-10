# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path
import shutil

import pytest
import slint_testing
from slint_testing import keys

from source_snapshot import SourceSnapshot
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    press_key,
    press_shortcut,
    select_outline_row,
    wait_until,
    window_element_with_label,
)


@pytest.fixture
def scene(tmp_path):
    path = tmp_path / "LinearGradientScene.slint"
    shutil.copyfile(
        Path(__file__).parents[1] / "fixtures" / "linear-gradient.slint", path
    )
    return path


def control(window, label, role=slint_testing.AccessibleRole.Button):
    return window_element_with_label(window, label, role)


def click(window, label):
    control(window, label).invoke_accessible_default_action()


def center(element):
    return slint_testing.LogicalPosition(
        x=element.absolute_position.x + element.size.width / 2,
        y=element.absolute_position.y + element.size.height / 2,
    )


def shifted(point, x=0, y=0):
    return slint_testing.LogicalPosition(x=point.x + x, y=point.y + y)


def gesture(window, start, end):
    button = slint_testing.PointerEventButton.Left
    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    window.dispatch_event(slint_testing.PointerMoveEvent(end))
    window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))


def open_linear(window):
    select_outline_row(window, "fill")
    click(window, "Rectangle background color picker")
    control(window, "Gradient start")


def test_linear_canvas_activation_and_colour(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        assert not elements_with_label(window.root_element, "Gradient start")
        click(window, "Rectangle background color picker")
        start = center(control(window, "Gradient start"))
        end = center(control(window, "Gradient end"))
        assert end.x - start.x == pytest.approx(200)
        assert end.y == pytest.approx(start.y)
        assert not elements_with_label(window.root_element, "Gradient angle degrees")
        assert not elements_with_label(window.root_element, "Add gradient stop")
        assert not elements_with_label(window.root_element, "Close Stop color")
        click(window, "Gradient stop 2")
        hex_field = control(window, "Hex color", slint_testing.AccessibleRole.TextInput)
        assert hex_field.accessible_value == "#264052"
        hex_field.accessible_value = "#12ab3480"
        original.assert_unchanged_now()
        click(window, "Solid")
        assert not elements_with_label(window.root_element, "Gradient start")
        click(window, "Gradient")
        control(window, "Gradient start")
        original.assert_unchanged_now()
        (tmp_path / "linear-gradient-editor.png").write_bytes(window.grab_window_as_png())
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_linear_endpoint_drag_and_session_history(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        window = first_window(editor)
        open_linear(window)
        start = center(control(window, "Gradient start"))
        gesture(window, start, shifted(start, x=40))
        assert center(control(window, "Gradient start")).x == pytest.approx(start.x + 40)
        control(window, "Hex color", slint_testing.AccessibleRole.TextInput)
        original.assert_unchanged_now()
        click(window, "Close Custom")
        saved = wait_until(
            lambda: scene.read_bytes()
            if scene.read_bytes() != original.sources[Path(scene.name)]
            else None
        )
        original.wait_for_applied(saved, scene.name)
        assert b"20%" in saved
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(original.sources[Path(scene.name)], scene.name)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, scene.name)
        open_linear(window)
        assert center(control(window, "Gradient start")).x == pytest.approx(start.x + 40)


def test_linear_double_click_and_delete(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        window = first_window(editor)
        open_linear(window)
        axis = control(window, "Gradient axis")
        axis.double_click(slint_testing.PointerEventButton.Left)
        control(window, "Gradient stop 4")
        press_key(window, keys.Delete)
        assert not elements_with_label(window.root_element, "Gradient stop 4")
        click(window, "Gradient stop 2")
        press_key(window, keys.Delete)
        assert not elements_with_label(window.root_element, "Gradient stop 3")
        press_key(window, keys.Delete)
        control(window, "Gradient stop 2")
        control(window, "Gradient end")
        original.assert_unchanged_now()
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_linear_drag_escape_restores_gesture(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        window = first_window(editor)
        open_linear(window)
        start = center(control(window, "Gradient end"))
        end = shifted(start, x=-30, y=-50)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        press_key(window, keys.Escape)
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        restored = center(control(window, "Gradient end"))
        assert restored.x == pytest.approx(start.x)
        assert restored.y == pytest.approx(start.y)
        click(window, "Close Custom")
        original.assert_unchanged()
