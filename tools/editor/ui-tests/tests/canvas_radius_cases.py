# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import (
    center,
    frame_rotation,
    manual_drag,
    manual_radius_drag,
    offset_position,
    position_distance,
    radius_handle,
    rotation_start,
    same_state,
    selection_frame,
)
from editor_sync import wait_for_source
from slint_test import expect
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from test_canvas import (
    CORNERS,
    DISABLED_IDS,
    OUTSIDE_ARTBOARD_DISTANCE,
    RADIUS_DELTAS,
    THRESHOLD_LABELS,
    assert_radius_handle_positions,
    radius_handle_positions,
    radius_source,
    wait_for_radius_tooltip,
)
from ui_driver import (
    wait_until,
)


@pytest.mark.parametrize("single", (False, True))
@pytest.mark.parametrize("corner", CORNERS)
def test_each_radius_handle_writes_exact_source(
    editor_factory,
    fixture_project: Path,
    single: bool,
    corner: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    radii = {corner: 16} if single else {name: 16 for name in CORNERS}
    expected = radius_source(baseline, radii)
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_radius_drag(
            window,
            radius_handle(window, corner),
            *RADIUS_DELTAS[corner],
            snapshot,
            shift=single,
        )
        snapshot.wait_for_applied(expected)


@pytest.mark.parametrize("single", (False, True))
@pytest.mark.parametrize("corner", CORNERS)
def test_radius_handles_follow_preview_during_drag(
    editor_factory,
    fixture_project: Path,
    single: bool,
    corner: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        handle = radius_handle(window, corner)
        initial_positions = radius_handle_positions(window)
        start = center(handle)
        end = slint_testing.LogicalPosition(
            x=start.x + RADIUS_DELTAS[corner][0],
            y=start.y + RADIUS_DELTAS[corner][1],
        )
        snapshot = SourceSnapshot.capture(fixture_project)
        if single:
            window.keyboard.down(keys.Shift)
        window.pointer.press_at(start)
        window.pointer.move_to(end)

        wait_for_radius_tooltip(window, 16)
        assert_radius_handle_positions(window, initial_positions, corner, 4, single)
        snapshot.assert_unchanged_now()

        window.pointer.release_at(end)
        if single:
            window.keyboard.up(keys.Shift)


@pytest.mark.parametrize(
    ("angle", "radius", "place_right"),
    [(0, 40, True), (45, 40, True), (180, 12, False)],
)
def test_radius_tooltip_follows_handle_clear_of_corner(
    editor_factory,
    fixture_project: Path,
    angle: int,
    radius: int,
    place_right: bool,
) -> None:
    source_file = fixture_project / "Main.slint"
    source = replace_once(
        source_file.read_bytes(),
        b"        border-radius: 12px;",
        f"        border-radius: {radius}px;\n        transform-rotation: {angle}deg;".encode(),
    )
    source_file.write_bytes(source)
    with editor_factory(source_file) as editor:
        window = editor.window
        wait_for_source(source_file, source)
        editor.canvas.select("Rectangle")
        rotation = math.radians(angle)
        handle = radius_handle(window, "top-left")
        start = center(handle, rotation)
        corner = center(
            window.get_by_accessible_name("Rectangle resize top-left").resolve(),
            rotation,
        )
        end = offset_position(start, 12, 4, rotation)
        window.pointer.press_at(start)
        window.pointer.move_to(end)

        dragged_radius = radius + 8
        wait_for_radius_tooltip(window, dragged_radius)
        tooltip = window.get_by_role("text", name="Radius value").resolve()
        tip = tooltip.absolute_position
        control = center(
            window.get_by_accessible_name("Rectangle radius top-left").resolve(),
            rotation,
        )
        extent = dragged_radius
        if place_right:
            assert tip.x == pytest.approx(
                max(control.x, corner.x + extent) + 12, abs=1.5
            )
        else:
            assert tip.x + tooltip.size.width == pytest.approx(
                min(control.x, corner.x - extent) - 12, abs=1.5
            )
        assert tip.y + tooltip.size.height == pytest.approx(control.y - 12, abs=1.5)

        window.pointer.release_at(end)


@pytest.mark.parametrize("corner", CORNERS)
def test_explicit_corner_radius_makes_every_handle_independent(
    editor_factory,
    fixture_project: Path,
    corner: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    source_file.write_bytes(
        replace_once(
            baseline,
            b"        border-radius: 12px;",
            b"        border-radius: 12px;\n        border-top-left-radius: 12px;",
        )
    )
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        handle = radius_handle(window, corner)
        initial_positions = radius_handle_positions(window)
        start = center(handle)
        end = slint_testing.LogicalPosition(
            x=start.x + RADIUS_DELTAS[corner][0],
            y=start.y + RADIUS_DELTAS[corner][1],
        )
        snapshot = SourceSnapshot.capture(fixture_project)
        window.pointer.press_at(start)
        window.pointer.move_to(end)

        wait_for_radius_tooltip(window, 16)
        assert_radius_handle_positions(window, initial_positions, corner, 4, True)
        snapshot.assert_unchanged_now()

        window.pointer.release_at(end)


@pytest.mark.parametrize(
    ("shift_before_press", "shift_during_drag", "single_corner"),
    [
        pytest.param(False, True, False, id="press-shift-after-pointer-press"),
        pytest.param(True, False, True, id="release-shift-after-pointer-press"),
    ],
)
def test_radius_drag_mode_does_not_change_after_pointer_press(
    editor_factory,
    fixture_project: Path,
    shift_before_press: bool,
    shift_during_drag: bool,
    single_corner: bool,
) -> None:
    source_file = fixture_project / "Main.slint"
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        corner = "top-left"
        handle = radius_handle(window, corner)
        initial_positions = radius_handle_positions(window)
        start = center(handle)
        end = slint_testing.LogicalPosition(
            x=start.x + RADIUS_DELTAS[corner][0],
            y=start.y + RADIUS_DELTAS[corner][1],
        )
        snapshot = SourceSnapshot.capture(fixture_project)
        if shift_before_press:
            window.keyboard.down(keys.Shift)
        window.pointer.press_at(start)
        if shift_during_drag:
            window.keyboard.down(keys.Shift)
        else:
            window.keyboard.up(keys.Shift)
        window.pointer.move_to(end)

        wait_for_radius_tooltip(window, 16)
        assert_radius_handle_positions(
            window, initial_positions, corner, 4, single_corner
        )
        snapshot.assert_unchanged_now()

        window.pointer.release_at(end)
        if shift_during_drag:
            window.keyboard.up(keys.Shift)


def test_clamped_radius_drag_keeps_handles_aligned_with_preview(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        corner = "top-left"
        handle = radius_handle(window, corner)
        initial_positions = radius_handle_positions(window)
        start = center(handle)
        end = slint_testing.LogicalPosition(x=start.x + 100, y=start.y + 100)
        snapshot = SourceSnapshot.capture(fixture_project)
        window.pointer.press_at(start)
        window.pointer.move_to(end)

        wait_for_radius_tooltip(window, 60)
        assert_radius_handle_positions(window, initial_positions, corner, 48, False)
        snapshot.assert_unchanged_now()

        window.pointer.release_at(end)


def test_repeated_rectangles_show_radius_handles_only_on_primary_instance(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    repeated_source = replace_once(
        source_file.read_bytes(),
        b"    root-rectangle := Rectangle {\n        x: 40px;\n        y: 40px;",
        b"    for index in [0, 1, 2]: root-rectangle := Rectangle {\n"
        b"        x: 20px + index * 120px;\n        y: 400px;",
    )
    repeated_source = replace_once(
        repeated_source,
        b"        width: 180px;\n        height: 120px;",
        b"        width: 100px;\n        height: 80px;",
    )
    source_file.write_bytes(repeated_source)
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source_file) as editor:
        window = editor.window
        instances_locator = window.get_by_id("Main::root-rectangle")
        expect(instances_locator).to_have_count(3)
        instances = instances_locator.all()
        target = center(instances[1])
        window.pointer.move_to(target)
        window.pointer.press_at(target)
        window.pointer.release_at(target)
        selected_locator = window.get_by_role("region", name="Selected Rectangle")
        expect(selected_locator).to_have_count(3)

        for corner in CORNERS:
            handles = window.get_by_role(
                "button", name=f"Rectangle radius {corner}"
            ).all()
            assert len(handles) == 1
            assert position_distance(center(handles[0]), target) < 100
        snapshot.assert_unchanged()


def test_radius_is_clamped_to_half_the_shortest_side(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    expected = radius_source(baseline, {name: 60 for name in CORNERS})
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_radius_drag(
            window,
            radius_handle(window, "top-left"),
            100,
            100,
            snapshot,
        )
        snapshot.wait_for_applied(expected)


def test_resize_continues_outside_window_and_commits_on_release(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        handle = window.get_by_accessible_name(
            "Rectangle resize bottom-right"
        ).resolve()
        start = center(handle)
        size = window.root_element.size
        dx = math.ceil(size.width + OUTSIDE_ARTBOARD_DISTANCE - start.x)
        dy = math.ceil(size.height + OUTSIDE_ARTBOARD_DISTANCE - start.y)
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_drag(window, handle, dx, dy, snapshot)
        expected = replace_once(
            baseline,
            b"        width: 180px;\n        height: 120px;",
            f"        width: {180 + dx}px;\n        height: {120 + dy}px;".encode(),
        )
        snapshot.wait_for_applied(expected)


def test_rotation_continues_outside_window_and_commits_on_release(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Text")
        handle = window.get_by_accessible_name("Text rotate top-left").resolve()
        start = rotation_start(handle)
        x, y, width, height = selection_frame(window, "Text")
        cx, cy = x + width / 2, y + height / 2
        vx, vy = start.x - cx, start.y - cy
        size = window.root_element.size
        scale = 4 * max(size.width, size.height) / math.hypot(vx, vy)
        snapshot = SourceSnapshot.capture(fixture_project)
        window.pointer.press_at(start)
        window.keyboard.down(keys.Shift)
        target = start
        for degrees in (15, 30):
            angle = math.radians(degrees)
            target = slint_testing.LogicalPosition(
                x=cx + (vx * math.cos(angle) - vy * math.sin(angle)) * scale,
                y=cy + (vx * math.sin(angle) + vy * math.cos(angle)) * scale,
            )
            assert target.x < 0 or target.y < 0
            window.pointer.move_to(target)
            wait_until(
                lambda degrees=degrees: (
                    True
                    if abs(math.degrees(frame_rotation(window, "Text")) - degrees) < 1
                    else None
                )
            )
            snapshot.assert_unchanged()
        window.pointer.release_at(target)
        window.keyboard.up(keys.Shift)
        original = b'        text: "Fixture text";'
        expected = replace_once(
            baseline, original, original + b"\n        transform-rotation: 30deg;"
        )
        snapshot.wait_for_applied(expected)


def test_radius_drag_continues_outside_window_and_commits_on_release(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        handle = radius_handle(window, "top-left")
        start = center(handle)
        size = window.root_element.size
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_radius_drag(
            window,
            handle,
            size.width + OUTSIDE_ARTBOARD_DISTANCE - start.x,
            size.height + OUTSIDE_ARTBOARD_DISTANCE - start.y,
            snapshot,
        )
        expected = radius_source(baseline, {corner: 60 for corner in CORNERS})
        snapshot.wait_for_applied(expected)


@pytest.mark.parametrize("label", THRESHOLD_LABELS)
def test_handle_click_below_drag_threshold_does_not_edit_source(
    editor_factory,
    fixture_project: Path,
    label: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        handle = (
            radius_handle(window, "top-left")
            if label == "Rectangle radius top-left"
            else window.get_by_accessible_name(label).resolve()
        )
        before = selection_frame(window, "Rectangle")
        start = center(handle)
        radius_position_before = start if label == "Rectangle radius top-left" else None
        end = slint_testing.LogicalPosition(x=start.x + 1, y=start.y + 1)
        window.pointer.press_at(start)
        window.pointer.move_to(end)
        assert same_state(selection_frame(window, "Rectangle"), before)
        if radius_position_before is not None:
            assert (
                position_distance(
                    center(window.get_by_role("button", name=label).resolve()),
                    radius_position_before,
                )
                < 1.5
            )
        window.pointer.release_at(end)
        snapshot.assert_unchanged()


@pytest.mark.parametrize("element_id", DISABLED_IDS)
@pytest.mark.parametrize("corner", CORNERS)
def test_disabled_manipulation_does_not_edit_source(
    editor_factory,
    fixture_project: Path,
    element_id: str,
    corner: str,
) -> None:
    source_file = fixture_project / "CanvasCases.slint"
    with editor_factory(source_file) as editor:
        window = editor.window
        snapshot = SourceSnapshot.capture(fixture_project)
        editor.outline.select(element_id)
        window.get_by_accessible_name("Selected Rectangle").wait_for()
        handle = window.get_by_accessible_name(f"Rectangle resize {corner}").resolve()
        assert not handle.accessible_enabled
        target = center(handle)
        window.drag_and_drop(
            target,
            slint_testing.LogicalPosition(x=target.x + 20, y=target.y + 16),
        )
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "press_shift_during_drag",
    [pytest.param(True, id="press-shift"), pytest.param(False, id="release-shift")],
)
def test_resize_modifier_changes_during_drag(
    editor_factory, fixture_project, press_shift_during_drag
):
    from slint_test import expect

    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source) as editor:
        rectangle = editor.canvas.element("root-rectangle")
        rectangle.select()
        keyboard = editor.window.keyboard
        if not press_shift_during_drag:
            keyboard.down("Shift")
        with rectangle.handle("resize bottom-right").drag() as drag:
            drag.move_by(20, 16)
            before = rectangle.selection.bounds()
            snapshot.assert_unchanged_now()
            if press_shift_during_drag:
                keyboard.down("Shift")
            else:
                keyboard.up("Shift")
            expect.poll(
                rectangle.selection.bounds, session=editor.window.session
            ).not_to_equal(before)
            after = rectangle.selection.bounds()
            assert (after.width == after.height) == press_shift_during_drag
            snapshot.assert_unchanged_now()
            drag.release()
        if press_shift_during_drag:
            keyboard.up("Shift")
        height = 200 if press_shift_during_drag else 136
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                b"        width: 180px;\n        height: 120px;",
                f"        width: 200px;\n        height: {height}px;".encode(),
            )
        )
