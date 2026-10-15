# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore tobytes

import math
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import (
    center,
    center_canvas_selection,
    offset_position,
    zoom_canvas,
)
from editor_sync import wait_for_source
from gradient_interactions import gesture
from inspector_interactions import edit_field, inspector_field
from slint_testing import keys
from source_snapshot import SourceSnapshot, wait_for_source_change
from ui_assertions import expect
from ui_driver import (
    element,
    elements,
    first_window,
    launch_editor,
    outline_rows,
    press_key,
    press_shortcut,
    query,
    screenshot,
    select_outline_row,
)


@pytest.fixture
def shadow_scene(tmp_path):
    source = tmp_path / "Main.slint"
    source.write_bytes(
        (Path(__file__).parents[1] / "fixtures" / "drop-shadow.slint").read_bytes()
    )
    return source


def open_shadow(window):
    select_outline_row(window, "shadow-card")
    control = inspector_field(
        window, "Drop shadow", slint_testing.AccessibleRole.Button
    )
    point = center(control)
    gesture(window, point, point)
    element(window, "Shadow offset handle")


def move_shadow_panel(window):
    panel = element(window, id="InspectorShadowPicker::panel")
    files = element(window, "Files")
    start = center(element(window, "Move Drop shadow picker"))
    end = offset_position(
        start,
        files.absolute_position.x - panel.absolute_position.x,
        files.absolute_position.y - panel.absolute_position.y,
        0,
    )
    gesture(window, start, end)


def test_shadow_handles_keep_focus_for_consecutive_edits(
    editor_binary, editor_environment, shadow_scene, tmp_path
):
    baseline = shadow_scene.read_bytes()
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window)
        move_shadow_panel(window)
        expected = baseline
        for name, dx, dy, old, dragged, keyed, key in (
            ("spread", 20, 12, 10, 30, 31, keys.RightArrow),
            ("blur", 20, -8, 32, 48, 49, keys.UpArrow),
        ):
            start = center(element(window, f"Shadow {name} handle"))
            gesture(window, start, offset_position(start, dx, dy, 0))
            expected = expected.replace(
                f"drop-shadow-{name}: {old}px;".encode(),
                f"drop-shadow-{name}: {dragged}px;".encode(),
            )
            snapshot.wait_for_applied(expected)
            for handle in ("offset", "blur", "spread"):
                expect(query(window, f"Shadow {handle} handle")).to_be_visible()
            press_key(window, key)
            expected = expected.replace(
                f"drop-shadow-{name}: {dragged}px;".encode(),
                f"drop-shadow-{name}: {keyed}px;".encode(),
            )
            snapshot.wait_for_applied(expected)


def test_shadow_panel_uses_the_existing_solid_color_picker(
    editor_binary,
    editor_environment,
    shadow_scene,
    tmp_path,
):
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, shadow_scene.read_bytes())
        window = first_window(editor)
        open_shadow(window)
        element(window, "Shadow color", role=slint_testing.AccessibleRole.TextInput)
        element(window, "Shadow blur", role=slint_testing.AccessibleRole.Slider)
        for label in ("Shadow angle", "Shadow distance", "Shadow spread", "Hex color"):
            assert not elements(window, label)
        swatch = element(window, "Shadow color color picker")
        gesture(window, center(swatch), center(swatch))
        element(window, "Hex color", role=slint_testing.AccessibleRole.TextInput)
        element(window, "Close Custom").invoke_accessible_default_action()
        expect(query(window, "Hex color")).to_be_hidden()
        element(window, "Shadow blur", role=slint_testing.AccessibleRole.Slider)
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "handle,dx,dy,changes",
    [
        ("offset", 20, 12, (("offset-x", 24, 44), ("offset-y", 32, 44))),
        ("blur", 20, -8, (("blur", 32, 48),)),
        ("spread", 20, 12, (("spread", 10, 30),)),
    ],
)
@pytest.mark.parametrize("rotation,percent", [(0, 100), (30, 200)])
@pytest.mark.parametrize("outcome", ["commit", "cancel"])
def test_shadow_canvas_gestures_preview_cancel_and_undo(
    editor_binary,
    editor_environment,
    shadow_scene,
    tmp_path,
    handle,
    dx,
    dy,
    changes,
    rotation,
    percent,
    outcome,
):
    baseline = shadow_scene.read_bytes().replace(
        b"        x: 160px;",
        f"        transform-rotation: {rotation}deg;\n        x: 160px;".encode(),
    )
    shadow_scene.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        select_outline_row(window, "shadow-card")
        zoom_canvas(window, percent)
        center_canvas_selection(window)
        open_shadow(window)
        move_shadow_panel(window)
        start = center(element(window, f"Shadow {handle} handle"))
        end = offset_position(
            start, dx * percent / 100, dy * percent / 100, math.radians(rotation)
        )
        before = screenshot(window)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        snapshot.assert_unchanged()
        assert screenshot(window).tobytes() != before.tobytes()
        if handle == "offset":
            element(
                window,
                "offset-x: 44 px · offset-y: 44 px",
                role=slint_testing.AccessibleRole.Text,
            )
        if outcome == "cancel":
            press_key(window, keys.Escape)
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        if outcome == "cancel":
            snapshot.assert_unchanged()
        else:
            expected = baseline
            for name, old, new in changes:
                expected = expected.replace(
                    f"drop-shadow-{name}: {old}px;".encode(),
                    f"drop-shadow-{name}: {new}px;".encode(),
                )
            snapshot.wait_for_applied(expected)
            for name in ("offset", "blur", "spread"):
                expect(query(window, f"Shadow {name} handle")).to_be_visible()
            element(window, "Shadow blur", role=slint_testing.AccessibleRole.Slider)
            press_shortcut(window, keys.Control, "z")
            snapshot.wait_for_applied(baseline)
            press_shortcut(window, keys.Control, keys.Shift, "z")
            snapshot.wait_for_applied(expected)


@pytest.mark.parametrize("change", ["selection", "source"])
def test_shadow_drag_rejects_a_changed_target(
    editor_binary,
    editor_environment,
    shadow_scene,
    tmp_path,
    change,
):
    baseline = shadow_scene.read_bytes()
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window)
        move_shadow_panel(window)
        start = center(element(window, "Shadow offset handle"))
        end = offset_position(start, 20, 12, 0)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        snapshot.assert_unchanged()
        if change == "selection":
            outline_rows(window)[0].invoke_accessible_default_action()
        else:
            shadow_scene.write_bytes(baseline + b"\n// External edit\n")
            snapshot.wait_for_applied(baseline + b"\n// External edit\n")

        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        if change == "selection":
            snapshot.assert_unchanged()
        else:
            snapshot.wait_for_applied(baseline + b"\n// External edit\n")


@pytest.mark.parametrize("dragging", [False, True])
@pytest.mark.parametrize("availability", ["current", "stale"])
def test_removing_shadow_externally_closes_the_session(
    editor_binary, editor_environment, shadow_scene, tmp_path, dragging, availability
):
    baseline = shadow_scene.read_bytes()
    removed = b"".join(
        line
        for line in baseline.splitlines(keepends=True)
        if not line.lstrip().startswith(b"drop-shadow-")
    )
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window)
        move_shadow_panel(window)
        start = center(element(window, "Shadow offset handle"))
        end = offset_position(start, 20, 12, 0)
        button = slint_testing.PointerEventButton.Left
        if dragging:
            window.dispatch_event(slint_testing.PointerMoveEvent(start))
            window.dispatch_event(slint_testing.PointerPressEvent(start, button))
            window.dispatch_event(slint_testing.PointerMoveEvent(end))
        if availability == "stale":
            shadow_scene.write_bytes(baseline + b"\nthis is invalid Slint\n")
            element(window, "Stale preview", role=slint_testing.AccessibleRole.Region)
        shadow_scene.write_bytes(removed)
        snapshot.wait_for_applied(removed)
        expect(query(window, "Shadow offset handle")).to_be_hidden()
        expect(query(window, "Close Drop shadow")).to_be_hidden()
        if dragging:
            window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        snapshot.wait_for_applied(removed)


def test_picker_undo_closes_a_shadow_created_by_the_previous_edit(
    editor_binary, editor_environment, shadow_scene, tmp_path
):
    baseline = b"".join(
        line
        for line in shadow_scene.read_bytes().splitlines(keepends=True)
        if not line.lstrip().startswith(b"drop-shadow-")
    )
    shadow_scene.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        select_outline_row(window, "shadow-card")
        edit_field(
            window, "Rectangle effect", "drop", slint_testing.AccessibleRole.Combobox
        )
        added = wait_for_source_change(shadow_scene, baseline)
        assert b"drop-shadow-color:" in added
        open_shadow(window)
        move_shadow_panel(window)
        point = center(element(window, "Move Drop shadow picker"))
        gesture(window, point, point)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline)
        expect(query(window, "Shadow offset handle")).to_be_hidden()
        expect(query(window, "Close Drop shadow")).to_be_hidden()


@pytest.mark.parametrize("action", ["Inner shadow", "Remove shadow"])
@pytest.mark.parametrize("input_method", ["pointer", "keyboard"])
def test_shadow_actions_are_available_without_a_context_menu(
    editor_binary, editor_environment, shadow_scene, tmp_path, action, input_method
):
    baseline = shadow_scene.read_bytes()
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window)
        if input_method == "pointer":
            point = center(
                element(window, action, role=slint_testing.AccessibleRole.Button)
            )
            gesture(window, point, point)
        else:
            point = center(element(window, "Move Drop shadow picker"))
            gesture(window, point, point)
            panel = element(window, id="InspectorShadowPicker::panel")
            target = element(panel, action, role=slint_testing.AccessibleRole.Button)
            image = screenshot(window)
            scale = image.width / window.root_element.size.width
            rect = target.absolute_rect
            bounds = (
                round(rect.x * scale),
                round(rect.y * scale),
                round((rect.x + rect.width) * scale),
                round((rect.y + rect.height) * scale),
            )
            unfocused = image.crop(bounds).tobytes()
            for _ in range(16):
                press_key(window, keys.Tab)
                if screenshot(window).crop(bounds).tobytes() != unfocused:
                    break
            else:
                raise AssertionError(f"Cannot reach {action} with Tab")
            press_key(window, keys.Return)
        expected = wait_for_source_change(shadow_scene, baseline)
        assert b"drop-shadow-" not in expected
        if action == "Inner shadow":
            assert b"inner-shadow-color:" in expected
        else:
            assert b"inner-shadow-" not in expected
        expect(query(window, "Close Drop shadow")).to_be_hidden()
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline)


@pytest.mark.parametrize("rotation,percent", [(0, 100), (45, 25)])
@pytest.mark.parametrize("handle", ["offset", "blur", "spread"])
def test_small_shadow_handles_remain_individually_draggable(
    editor_binary, editor_environment, shadow_scene, handle, rotation, percent
):
    baseline = (
        shadow_scene.read_bytes()
        .replace(b"width: 240px", b"width: 10px")
        .replace(b"height: 160px", b"height: 10px")
        .replace(b"drop-shadow-spread: 10px", b"drop-shadow-spread: -6px")
        .replace(b"drop-shadow-blur: 32px", b"drop-shadow-blur: 0px")
        .replace(
            b"        x: 160px;",
            f"        transform-rotation: {rotation}deg;\n        x: 160px;".encode(),
        )
    )
    shadow_scene.write_bytes(baseline)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        select_outline_row(window, "shadow-card")
        zoom_canvas(window, percent)
        center_canvas_selection(window)
        open_shadow(window)
        move_shadow_panel(window)
        start = center(element(window, f"Shadow {handle} handle"))
        end = offset_position(start, 8, -8, math.radians(rotation))
        gesture(window, start, end)
        changed = wait_for_source_change(shadow_scene, baseline)
        names = ("offset-x", "offset-y") if handle == "offset" else (handle,)
        for name in names:
            before = next(
                line
                for line in baseline.splitlines()
                if f"drop-shadow-{name}:".encode() in line
            )
            assert before not in changed.splitlines()


def test_shadow_picker_does_not_undo_a_stale_preview(
    editor_binary, editor_environment, shadow_scene, tmp_path
):
    baseline = shadow_scene.read_bytes()
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window)
        move_shadow_panel(window)
        start = center(element(window, "Shadow offset handle"))
        gesture(window, start, offset_position(start, 20, 12, 0))
        changed = wait_for_source_change(shadow_scene, baseline)
        point = center(element(window, "Move Drop shadow picker"))
        gesture(window, point, point)
        broken = changed + b"\nthis is invalid Slint\n"
        shadow_scene.write_bytes(broken)
        element(window, "Stale preview", role=slint_testing.AccessibleRole.Region)
        broken_snapshot = SourceSnapshot.capture(tmp_path)
        press_shortcut(window, keys.Control, "z")
        broken_snapshot.assert_unchanged()
        shadow_scene.write_bytes(changed)
        snapshot.wait_for_applied(changed)
        start = center(element(window, "Shadow blur handle"))
        gesture(window, start, offset_position(start, 0, -8, 0))
        edited = changed.replace(b"drop-shadow-blur: 32px", b"drop-shadow-blur: 48px")
        snapshot.wait_for_applied(edited)
        point = center(element(window, "Move Drop shadow picker"))
        gesture(window, point, point)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(changed)


@pytest.mark.parametrize(
    "expression,expected", [(None, "16"), ("root.shadow-blur", "28")]
)
def test_shadow_blur_uses_inspector_defaults_and_resolved_bindings(
    editor_binary, editor_environment, shadow_scene, expression, expected
):
    baseline = shadow_scene.read_bytes().replace(
        b"        drop-shadow-blur: 32px;\n",
        b""
        if expression is None
        else f"        drop-shadow-blur: {expression};\n".encode(),
    )
    baseline = baseline.replace(
        b"    shadow-card :=",
        b"    in property <length> shadow-blur: 28px;\n    shadow-card :=",
    )
    shadow_scene.write_bytes(baseline)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window)
        expect(
            element(
                window, "Shadow blur value", role=slint_testing.AccessibleRole.TextInput
            )
        ).to_have_value(expected)


def test_shadow_drag_back_to_start_leaves_no_draft_or_history_entry(
    editor_binary, editor_environment, shadow_scene, tmp_path
):
    baseline = shadow_scene.read_bytes()
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window)
        move_shadow_panel(window)
        start = center(element(window, "Shadow offset handle"))
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(
            slint_testing.PointerMoveEvent(offset_position(start, 20, 12, 0))
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerReleaseEvent(start, button))
        snapshot.assert_unchanged()
        actual = center(element(window, "Shadow offset handle"))
        assert (actual.x, actual.y) == pytest.approx((start.x, start.y))
        press_key(window, keys.RightArrow)
        expected = baseline.replace(
            b"drop-shadow-offset-x: 24px", b"drop-shadow-offset-x: 25px"
        )
        snapshot.wait_for_applied(expected)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline)
