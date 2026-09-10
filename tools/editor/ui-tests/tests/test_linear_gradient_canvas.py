# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path
import math
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


def center(element, rotation=0):
    angle = math.radians(rotation)
    return slint_testing.LogicalPosition(
        x=element.absolute_position.x
        + element.size.width / 2 * math.cos(angle)
        - element.size.height / 2 * math.sin(angle),
        y=element.absolute_position.y
        + element.size.width / 2 * math.sin(angle)
        + element.size.height / 2 * math.cos(angle),
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


@pytest.mark.parametrize("rotation", [0, 45, 90, 180])
def test_stop_drag_crosses_neighbours_without_losing_capture(
    editor_binary, editor_environment, scene, tmp_path, rotation
):
    scene.write_text(
        scene.read_text().replace(
            "        width: 200px;",
            f"        transform-rotation: {rotation}deg;\n        width: 200px;",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        window = first_window(editor)
        open_linear(window)
        start = center(control(window, "Gradient stop 2"), rotation)

        def destination(distance):
            return shifted(
                start,
                x=distance * math.cos(math.radians(rotation)),
                y=distance * math.sin(math.radians(rotation)),
            )

        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        for dx in (30, 70, 100, 130, 70, -50, -120, 20):
            window.dispatch_event(slint_testing.PointerMoveEvent(destination(dx)))
            assert (
                control(
                    window, "Hex color", slint_testing.AccessibleRole.TextInput
                ).accessible_value
                == "#264052"
            )
            actual = center(control(window, "Gradient stop 2"), rotation)
            assert actual.x == pytest.approx(destination(dx).x, abs=0.001)
            assert actual.y == pytest.approx(destination(dx).y, abs=0.001)
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(destination(20), button)
        )
        (tmp_path / "gradient-stop-marker.png").write_bytes(window.grab_window_as_png())
        press_key(window, keys.Delete)
        assert not elements_with_label(window.root_element, "Gradient stop 3")
        press_key(window, keys.Escape)
        original.assert_unchanged()


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
        (tmp_path / "linear-gradient-editor.png").write_bytes(
            window.grab_window_as_png()
        )
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
        assert center(control(window, "Gradient start")).x == pytest.approx(
            start.x + 40
        )
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
        assert center(control(window, "Gradient start")).x == pytest.approx(
            start.x + 40
        )


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


@pytest.mark.parametrize("rotation", [0, 45, 90])
def test_linear_axis_translation_tracks_rotated_rectangles(
    editor_binary, editor_environment, scene, tmp_path, rotation
):
    scene.write_text(
        scene.read_text().replace(
            "        width: 200px;",
            f"        transform-rotation: {rotation}deg;\n        width: 200px;",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        window = first_window(editor)
        open_linear(window)
        start = center(control(window, "Gradient start"), rotation)
        end = center(control(window, "Gradient end"), rotation)
        midpoint = slint_testing.LogicalPosition(
            x=(start.x + end.x) / 2, y=(start.y + end.y) / 2
        )
        gesture(window, midpoint, shifted(midpoint, x=17, y=23))
        moved_start = center(control(window, "Gradient start"), rotation)
        moved_end = center(control(window, "Gradient end"), rotation)
        assert moved_start.x == pytest.approx(start.x + 17, abs=0.02)
        assert moved_start.y == pytest.approx(start.y + 23, abs=0.02)
        assert moved_end.x == pytest.approx(end.x + 17, abs=0.02)
        assert moved_end.y == pytest.approx(end.y + 23, abs=0.02)
        original.assert_unchanged_now()
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_linear_layout_size_and_keyboard(
    editor_binary, editor_environment, scene, tmp_path
):
    scene.write_text("""export component LinearGradientScene inherits Window {
    width: 400px;
    height: 400px;
    VerticalLayout {
        padding: 100px;
        fill := Rectangle { background: @linear-gradient(90deg, red, blue); }
    }
}
""")
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        window = first_window(editor)
        open_linear(window)
        start = center(control(window, "Gradient start"))
        end = center(control(window, "Gradient end"))
        assert end.x - start.x == pytest.approx(200)
        click(window, "Gradient end")
        press_key(window, keys.LeftArrow)
        press_shortcut(window, keys.Shift, keys.UpArrow)
        end = center(
            control(window, "Gradient end"), math.degrees(math.atan2(-10, 199))
        )
        assert end.x - start.x == pytest.approx(199)
        assert end.y - start.y == pytest.approx(-10, abs=0.001)
        click(window, "Gradient stop 1")
        press_key(window, keys.RightArrow)
        click(window, "Close Custom")
        saved = wait_until(
            lambda: scene.read_bytes()
            if b"red, blue" not in scene.read_bytes()
            else None
        )
        from editor_sync import wait_for_source

        wait_for_source(scene, saved)
        assert b"width: 200px" not in saved


def test_linear_outside_click_accepts_before_selecting_another_rectangle(
    editor_binary, editor_environment, scene, tmp_path
):
    scene.write_text(
        scene.read_text().replace(
            "    fill := Rectangle {",
            "    other := Rectangle { x: 10px; y: 10px; width: 50px; height: 50px; background: yellow; }\n    fill := Rectangle {",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        window = first_window(editor)
        open_linear(window)
        control(
            window, "Hex color", slint_testing.AccessibleRole.TextInput
        ).accessible_value = "#123456"
        other = wait_until(
            lambda: next(
                iter(window.find_elements_by_id("LinearGradientScene::other")), None
            )
        )
        other.single_click(slint_testing.PointerEventButton.Left)
        saved = wait_until(
            lambda: scene.read_bytes() if b"#123456" in scene.read_bytes() else None
        )
        original.wait_for_applied(saved, scene.name)
        assert b"background: yellow" in saved
        assert not elements_with_label(window.root_element, "Gradient start")
        assert not elements_with_label(window.root_element, "Close Custom")


def test_linear_external_edit_cancels_stale_draft(
    editor_binary, editor_environment, scene
):
    from editor_sync import wait_for_source

    original = scene.read_text()
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        window = first_window(editor)
        open_linear(window)
        control(
            window, "Hex color", slint_testing.AccessibleRole.TextInput
        ).accessible_value = "#123456"
        external = original.replace("#568fb8", "#abcdef")
        scene.write_text(external)
        wait_for_source(scene, external.encode())
        assert not elements_with_label(window.root_element, "Gradient start")
        assert not elements_with_label(window.root_element, "Close Custom")
        assert scene.read_text() == external
        open_linear(window)
        assert (
            control(
                window, "Hex color", slint_testing.AccessibleRole.TextInput
            ).accessible_value
            == "#abcdef"
        )


def test_linear_extended_axis_round_trip(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        window = first_window(editor)
        open_linear(window)
        start = center(control(window, "Gradient start"))
        gesture(window, start, shifted(start, x=-50))
        end = center(control(window, "Gradient end"))
        gesture(window, end, shifted(end, x=50))
        click(window, "Close Custom")
        saved = wait_until(
            lambda: scene.read_bytes()
            if scene.read_bytes() != original.sources[Path(scene.name)]
            else None
        )
        original.wait_for_applied(saved, scene.name)
        assert b"0% - 25%" in saved
        assert b"125%" in saved
        open_linear(window)
        assert center(control(window, "Gradient start")).x == pytest.approx(
            start.x - 50
        )
        assert center(control(window, "Gradient end")).x == pytest.approx(end.x + 50)
