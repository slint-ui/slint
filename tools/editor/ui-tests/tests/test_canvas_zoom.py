# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0


import pytest
import slint_testing
from canvas_interactions import (
    center,
    center_canvas_selection,
    manual_drag,
    manual_radius_drag,
    manual_rotation_drag,
    radius_handle,
    rotation_delta,
    zoom_canvas,
)
from editor_sync import wait_for_source
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_driver import (
    file_row,
    first_window,
    launch_editor,
    press_key,
    press_keys,
    press_shortcut,
    screenshot,
    select_outline_row,
    wait_until,
    window_element_with_label,
)


@pytest.mark.parametrize("percent", [25, 50, 100, 125, 200, 400])
def test_zoom_scales_content_and_preserves_controls(
    editor_binary, editor_environment, fixture_project, percent, tmp_path
):
    source = fixture_project / "Main.slint"
    original = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        center_canvas_selection(window)
        initial_center = center(window_element_with_label(window, "Selected Rectangle"))
        zoom_canvas(window, percent)
        frame = window_element_with_label(window, "Selected Rectangle")
        assert center(frame).x == pytest.approx(initial_center.x)
        assert center(frame).y == pytest.approx(initial_center.y)
        assert frame.size.width == pytest.approx(180 * percent / 100)
        assert frame.size.height == pytest.approx(120 * percent / 100)
        handle = window_element_with_label(window, "Rectangle resize top-left")
        assert handle.size.width == pytest.approx(12)
        assert handle.size.height == pytest.approx(12)
        assert center(handle).x == pytest.approx(frame.absolute_position.x)
        assert center(handle).y == pytest.approx(frame.absolute_position.y)
        canvas = window_element_with_label(window, "Editor canvas")
        window.dispatch_event(
            slint_testing.PointerMoveEvent(
                slint_testing.LogicalPosition(
                    x=canvas.absolute_position.x + 10,
                    y=canvas.absolute_position.y + 10,
                )
            )
        )
        rendered = screenshot(window)
        scale = rendered.width / window.root_element.size.width
        row = round((frame.absolute_position.y + frame.size.height * 0.75) * scale)
        left = round(frame.absolute_position.x * scale)
        right = round((frame.absolute_position.x + frame.size.width) * scale)
        blue_pixels = sum(
            rendered.getpixel((x, row)) == (37, 99, 235) for x in range(left, right)
        )
        assert blue_pixels / scale == pytest.approx(180 * percent / 100, abs=2)
        rendered.save(tmp_path / f"zoom-{percent}.png")
        press_shortcut(window, keys.Control, "0")
        wait_until(lambda: True if frame.size.width == pytest.approx(180) else None)
        screenshot(window).save(tmp_path / "actual-size.png")
        original.assert_unchanged()


@pytest.mark.parametrize("percent", [50, 125, 200])
@pytest.mark.parametrize("operation", ["move", "resize"])
def test_zoomed_drag_writes_document_units(
    editor_binary, editor_environment, fixture_project, percent, operation
):
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    original = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        center_canvas_selection(window)
        zoom_canvas(window, percent)
        label = (
            "Rectangle move handle"
            if operation == "move"
            else "Rectangle resize bottom-right"
        )
        handle = window_element_with_label(window, label)
        manual_drag(window, handle, 20 * percent / 100, 16 * percent / 100, original)
        old, new = (
            (b"x: 40px;\n        y: 40px;", b"x: 60px;\n        y: 56px;")
            if operation == "move"
            else (
                b"width: 180px;\n        height: 120px;",
                b"width: 200px;\n        height: 136px;",
            )
        )
        expected = replace_once(baseline, old, new)
        original.wait_for_applied(expected)
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(baseline)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(expected)


@pytest.mark.parametrize("percent", [50, 200])
def test_canvas_scroll_and_space_pan(
    editor_binary, editor_environment, fixture_project, percent
):
    source = fixture_project / "Main.slint"
    original = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        center_canvas_selection(window)
        zoom_canvas(window, percent)
        frame = window_element_with_label(window, "Selected Rectangle")
        canvas = window_element_with_label(window, "Editor canvas")
        start = center(canvas)
        before = frame.absolute_position
        window.dispatch_event(
            slint_testing.PointerScrolledEvent(start, delta_x=24, delta_y=32)
        )
        wait_until(
            lambda: (
                True
                if frame.absolute_position.x == pytest.approx(before.x + 24)
                else None
            )
        )
        assert frame.absolute_position.y == pytest.approx(before.y + 32)
        # Clicking the canvas frame gives the editor keyboard focus.
        handle = window_element_with_label(window, "Rectangle move handle")
        point = center(handle)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(point, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(point, button))
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Space))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        end = slint_testing.LogicalPosition(x=start.x + 30, y=start.y + 20)
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        assert frame.absolute_position.x == pytest.approx(before.x + 54)
        assert frame.absolute_position.y == pytest.approx(before.y + 52)
        press_key(window, keys.Escape)
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Space))
        assert frame.absolute_position.x == pytest.approx(before.x + 24)
        original.assert_unchanged()


def test_zoom_during_inline_edit_preserves_text(
    editor_binary, editor_environment, fixture_project
):
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    original = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_outline_row(window, "root-text")
        window_element_with_label(window, "Text move handle").double_click(
            slint_testing.PointerEventButton.Left
        )
        text = window_element_with_label(window, "Inline text editor")
        press_keys(window, "Hello ")
        press_shortcut(window, keys.Control, "=")
        wait_until(lambda: True if text.size.width == pytest.approx(225) else None)
        press_keys(window, "world")
        press_key(window, keys.Return)
        original.wait_for_applied(
            replace_once(baseline, b'text: "Fixture text";', b'text: "Hello world";')
        )


@pytest.mark.parametrize("percent", [50, 125, 200])
@pytest.mark.parametrize("operation", ["radius", "rotation"])
def test_zoomed_radius_and_rotation(
    editor_binary, editor_environment, fixture_project, percent, operation
):
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    original = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        center_canvas_selection(window)
        zoom_canvas(window, percent)
        if operation == "radius":
            manual_radius_drag(
                window,
                radius_handle(window, "top-left"),
                8 * percent / 100,
                8 * percent / 100,
                original,
            )
            expected = replace_once(
                baseline,
                b"        border-radius: 12px;",
                b"        border-bottom-left-radius: 20px;\n"
                b"        border-bottom-right-radius: 20px;\n"
                b"        border-radius: 12px;\n"
                b"        border-top-left-radius: 20px;\n"
                b"        border-top-right-radius: 20px;",
            )
        else:
            handle = window_element_with_label(window, "Rectangle rotate top-left")
            dx, dy = rotation_delta(window, handle, 15, kind="Rectangle")
            manual_rotation_drag(
                window, handle, dx, dy, original, kind="Rectangle", target_angle=15
            )
            # Rotation adds a property; wait for a complete applied source before undoing it.
            expected = replace_once(
                baseline,
                b"        x: 40px;\n        y: 40px;",
                b"        x: 40px;\n        y: 40px;\n        transform-rotation: 15deg;",
            )
        original.wait_for_applied(expected)
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(baseline)


def test_zoom_is_blocked_during_resize(
    editor_binary, editor_environment, fixture_project
):
    source = fixture_project / "Main.slint"
    original = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        handle = window_element_with_label(window, "Rectangle resize bottom-right")
        start = center(handle)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        press_shortcut(window, keys.Control, "+")
        assert (
            window_element_with_label(window, "Editor canvas").accessible_value
            == "100%"
        )
        window.dispatch_event(slint_testing.PointerReleaseEvent(start, button))
        zoom_canvas(window, 125)
        original.assert_unchanged()


def test_view_survives_reload_and_document_switch(
    editor_binary, editor_environment, fixture_project
):
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    original = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        center_canvas_selection(window)
        zoom_canvas(window, 200)
        frame = window_element_with_label(window, "Selected Rectangle")
        before = frame.absolute_position
        expected = replace_once(baseline, b'"Fixture text"', b'"Reloaded"')
        source.write_bytes(expected)
        original.wait_for_applied(expected)
        select_outline_row(window, "root-rectangle")
        frame = window_element_with_label(window, "Selected Rectangle")
        assert frame.absolute_position.x == pytest.approx(before.x)
        assert frame.absolute_position.y == pytest.approx(before.y)
        assert frame.size.width == pytest.approx(360)
        file_row(
            window, fixture_project / "Sibling.slint"
        ).invoke_accessible_default_action()
        window_element_with_label(
            window, "sibling-rectangle", slint_testing.AccessibleRole.ListItem
        )
        assert (
            window_element_with_label(window, "Editor canvas").accessible_value
            == "200%"
        )
        file_row(window, source).invoke_accessible_default_action()
        select_outline_row(window, "root-rectangle")
        frame = window_element_with_label(window, "Selected Rectangle")
        assert frame.absolute_position.x == pytest.approx(before.x)
        assert frame.absolute_position.y == pytest.approx(before.y)
        assert frame.size.width == pytest.approx(360)


@pytest.mark.parametrize("percent,key", [(25, "-"), (400, "+")])
def test_zoom_limits(editor_binary, editor_environment, fixture_project, percent, key):
    source = fixture_project / "Main.slint"
    original = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        zoom_canvas(window, percent)
        for _ in range(3):
            press_shortcut(window, keys.Control, key)
        assert (
            window_element_with_label(window, "Editor canvas").accessible_value
            == f"{percent}%"
        )
        press_shortcut(window, keys.Control, "0")
        wait_until(
            lambda: (
                True
                if window_element_with_label(window, "Editor canvas").accessible_value
                == "100%"
                else None
            )
        )
        original.assert_unchanged()


@pytest.mark.parametrize("percent", [50, 200])
def test_navigation_keeps_gradient_picker_open(
    editor_binary, editor_environment, fixture_project, percent
):
    from gradient_interactions import click, control, gesture, shifted

    source = fixture_project / "Main.slint"
    source.write_bytes(
        source.read_bytes().replace(
            b"background: #2563eb;", b"background: @linear-gradient(90deg, red, blue);"
        )
    )
    original = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        center_canvas_selection(window)
        click(window, "Rectangle background color picker")
        initial = control(window, "Gradient start").size
        zoom_canvas(window, percent)
        start = control(window, "Gradient start")
        assert start.size.width == pytest.approx(initial.width)
        assert start.size.height == pytest.approx(initial.height)
        point = center(start)
        gesture(window, point, point)
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Space))
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(point, button))
        end = shifted(point, x=24, y=32)
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Space))
        moved = center(control(window, "Gradient start"))
        assert moved.x == pytest.approx(point.x + 24)
        assert moved.y == pytest.approx(point.y + 32)
        window.dispatch_event(slint_testing.PointerMoveEvent(moved))
        window.dispatch_event(
            slint_testing.PointerScrolledEvent(moved, delta_x=-12, delta_y=-16)
        )
        moved_again = center(control(window, "Gradient start"))
        assert moved_again.x == pytest.approx(point.x + 12)
        assert moved_again.y == pytest.approx(point.y + 16)
        click(window, "Close Custom")
        original.assert_unchanged()
