# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore getpixel


import math

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
from slint_test import expect
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_driver import (
    file_row,
    first_window,
    launch_editor,
    screenshot,
    select_outline_row,
    wait_until,
)

ZOOM_LEVELS = [25, 50, 75, 100, 125, 150, 200, 300, 400]


def expected_fit_zoom(
    canvas: slint_testing.Element, width: float, height: float
) -> int:
    fit = min((canvas.size.width - 64) / width, (canvas.size.height - 64) / height)
    return max((level for level in ZOOM_LEVELS if level / 100 <= fit), default=25)


@pytest.mark.parametrize("percent", [25, 50, 75, 100, 125, 150, 200, 300, 400])
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
        initial_center = center(
            window.get_by_accessible_name("Selected Rectangle").resolve()
        )
        zoom_canvas(window, percent)
        frame = window.get_by_accessible_name("Selected Rectangle").resolve()
        assert center(frame).x == pytest.approx(initial_center.x)
        assert center(frame).y == pytest.approx(initial_center.y)
        assert frame.size.width == pytest.approx(180 * percent / 100)
        assert frame.size.height == pytest.approx(120 * percent / 100)
        handle = window.get_by_accessible_name("Rectangle resize top-left").resolve()
        assert handle.size.width == pytest.approx(12)
        assert handle.size.height == pytest.approx(12)
        assert center(handle).x == pytest.approx(frame.absolute_position.x)
        assert center(handle).y == pytest.approx(frame.absolute_position.y)
        canvas = window.get_by_accessible_name("Editor canvas").resolve()
        window.pointer.move_to(
            slint_testing.LogicalPosition(
                x=canvas.absolute_position.x + 10,
                y=canvas.absolute_position.y + 10,
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
        window.keyboard.shortcut(keys.Control, "0")
        wait_until(lambda: True if frame.size.width == pytest.approx(180) else None)
        screenshot(window).save(tmp_path / "actual-size.png")
        original.assert_unchanged()


@pytest.mark.parametrize(
    ("file_name", "row", "width", "height", "angle_degrees"),
    [
        ("Main.slint", "root-rectangle", 180, 120, 0),
        ("RotatedCanvasCases.slint", "rotated-free-rectangle", 140, 96, 30),
    ],
)
def test_zoom_to_selection_uses_largest_fitting_level(
    editor_binary,
    editor_environment,
    fixture_project,
    file_name,
    row,
    width,
    height,
    angle_degrees,
):
    source = fixture_project / file_name
    original = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        select_outline_row(window, row)
        canvas = window.get_by_accessible_name("Editor canvas").resolve()
        angle = math.radians(angle_degrees)
        bounds_width = width * abs(math.cos(angle)) + height * abs(math.sin(angle))
        bounds_height = width * abs(math.sin(angle)) + height * abs(math.cos(angle))
        expected = expected_fit_zoom(canvas, bounds_width, bounds_height)
        window.keyboard.shortcut(keys.Shift, "2")
        expect(window.get_by_accessible_name("Editor canvas")).to_have_value(
            f"{expected}%"
        )
        frame = window.get_by_accessible_name("Selected Rectangle").resolve()
        assert center(frame).x == pytest.approx(center(canvas).x)
        assert center(frame).y == pytest.approx(center(canvas).y)
        original.assert_unchanged()


def test_zoom_to_selection_centers_canvas_when_unselected(
    editor_binary, editor_environment, fixture_project
):
    source = fixture_project / "Main.slint"
    original = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        window.get_by_accessible_name("Selected Rectangle").wait_for()
        canvas = window.get_by_accessible_name("Editor canvas").resolve()
        clear_point = slint_testing.LogicalPosition(
            x=canvas.absolute_position.x + 10,
            y=canvas.absolute_position.y + 10,
        )
        window.pointer.press_at(clear_point)
        window.pointer.release_at(clear_point)
        expect(window.get_by_accessible_name("Selected Rectangle")).to_be_hidden()
        target = center(canvas)
        window.pointer.scroll(80, -120, at=target)
        artboard = window.get_by_role("region", name="Artboard").resolve()
        assert center(artboard).x != pytest.approx(target.x)
        assert center(artboard).y != pytest.approx(target.y)
        window.keyboard.shortcut(keys.Shift, "2")
        wait_until(
            lambda: (
                True
                if center(artboard).x == pytest.approx(target.x)
                and center(artboard).y == pytest.approx(target.y)
                else None
            )
        )
        assert canvas.accessible_value == "100%"
        zoom_canvas(window, 125)
        window.keyboard.shortcut(keys.Shift, "2")
        assert canvas.accessible_value == "125%"
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
        handle = window.get_by_accessible_name(label).resolve()
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
        window.keyboard.shortcut(keys.Control, "z")
        original.wait_for_applied(baseline)
        window.keyboard.shortcut(keys.Control, keys.Shift, "z")
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
        frame = window.get_by_accessible_name("Selected Rectangle").resolve()
        canvas = window.get_by_accessible_name("Editor canvas").resolve()
        start = center(canvas)
        before = frame.absolute_position
        window.pointer.scroll(24, 32, at=start)
        wait_until(
            lambda: (
                True
                if frame.absolute_position.x == pytest.approx(before.x + 24)
                else None
            )
        )
        assert frame.absolute_position.y == pytest.approx(before.y + 32)
        # Clicking the canvas frame gives the editor keyboard focus.
        handle = window.get_by_accessible_name("Rectangle move handle").resolve()
        point = center(handle)
        window.pointer.press_at(point)
        window.pointer.release_at(point)
        window.keyboard.down(keys.Space)
        window.pointer.press_at(start)
        end = slint_testing.LogicalPosition(x=start.x + 30, y=start.y + 20)
        window.pointer.move_to(end)
        assert frame.absolute_position.x == pytest.approx(before.x + 54)
        assert frame.absolute_position.y == pytest.approx(before.y + 52)
        window.keyboard.press(keys.Escape)
        window.pointer.release_at(end)
        window.keyboard.up(keys.Space)
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
        window.get_by_accessible_name("Text move handle").dblclick()
        text = window.get_by_accessible_name("Inline text editor").resolve()
        window.keyboard.press_sequentially("Hello ")
        window.keyboard.shortcut(keys.Control, "=")
        wait_until(lambda: True if text.size.width == pytest.approx(225) else None)
        window.keyboard.press_sequentially("world")
        window.keyboard.press(keys.Return)
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
            # Figma measures from the pointer even when the fixed handle inset exceeds the scaled radius.
            radius = 32 if percent == 50 else 20
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
                (
                    f"        border-bottom-left-radius: {radius}px;\n"
                    f"        border-bottom-right-radius: {radius}px;\n"
                    "        border-radius: 12px;\n"
                    f"        border-top-left-radius: {radius}px;\n"
                    f"        border-top-right-radius: {radius}px;"
                ).encode(),
            )
        else:
            handle = window.get_by_accessible_name(
                "Rectangle rotate top-left"
            ).resolve()
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
        window.keyboard.shortcut(keys.Control, "z")
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
        handle = window.get_by_accessible_name(
            "Rectangle resize bottom-right"
        ).resolve()
        start = center(handle)
        window.pointer.press_at(start)
        window.keyboard.shortcut(keys.Control, "+")
        window.keyboard.shortcut(keys.Shift, "2")
        assert (
            window.get_by_accessible_name("Editor canvas").resolve().accessible_value
            == "100%"
        )
        window.pointer.release_at(start)
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
        frame = window.get_by_accessible_name("Selected Rectangle").resolve()
        before = frame.absolute_position
        expected = replace_once(baseline, b'"Fixture text"', b'"Reloaded"')
        source.write_bytes(expected)
        original.wait_for_applied(expected)
        select_outline_row(window, "root-rectangle")
        frame = window.get_by_accessible_name("Selected Rectangle").resolve()
        assert frame.absolute_position.x == pytest.approx(before.x)
        assert frame.absolute_position.y == pytest.approx(before.y)
        assert frame.size.width == pytest.approx(360)
        file_row(window, fixture_project / "Sibling.slint").activate()
        window.get_by_role("list-item", name="sibling-rectangle").wait_for()
        assert (
            window.get_by_accessible_name("Editor canvas").resolve().accessible_value
            == "200%"
        )
        file_row(window, source).activate()
        select_outline_row(window, "root-rectangle")
        frame = window.get_by_accessible_name("Selected Rectangle").resolve()
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
            window.keyboard.shortcut(keys.Control, key)
        assert (
            window.get_by_accessible_name("Editor canvas").resolve().accessible_value
            == f"{percent}%"
        )
        zoom_canvas(window, 400 if percent == 25 else 25)
        window.keyboard.shortcut(keys.Control, "0")
        expect(window.get_by_accessible_name("Editor canvas")).to_have_value("100%")
        original.assert_unchanged()


@pytest.mark.parametrize("percent", [50, 200])
def test_navigation_keeps_gradient_picker_open(
    editor_binary, editor_environment, fixture_project, percent
):
    from gradient_interactions import gesture, shifted

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
        window.get_by_role(
            "button", name="Rectangle background color picker"
        ).activate()
        initial = window.get_by_role("button", name="Gradient start").resolve().size
        zoom_canvas(window, percent)
        start = window.get_by_role("button", name="Gradient start").resolve()
        assert start.size.width == pytest.approx(initial.width)
        assert start.size.height == pytest.approx(initial.height)
        point = center(start)
        gesture(window, point, point)
        window.keyboard.down(keys.Space)
        window.pointer.press_at(point)
        end = shifted(point, x=24, y=32)
        window.pointer.move_to(end)
        window.pointer.release_at(end)
        window.keyboard.up(keys.Space)
        moved = center(window.get_by_role("button", name="Gradient start").resolve())
        assert moved.x == pytest.approx(point.x + 24)
        assert moved.y == pytest.approx(point.y + 32)
        window.pointer.move_to(moved)
        window.pointer.scroll(-12, -16, at=moved)
        moved_again = center(
            window.get_by_role("button", name="Gradient start").resolve()
        )
        assert moved_again.x == pytest.approx(point.x + 12)
        assert moved_again.y == pytest.approx(point.y + 16)
        window.get_by_role("button", name="Close Custom").activate()
        original.assert_unchanged()
