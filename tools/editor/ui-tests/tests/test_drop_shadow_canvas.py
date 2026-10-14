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
from inspector_interactions import inspector_field
from slint_testing import keys
from source_snapshot import SourceSnapshot
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
