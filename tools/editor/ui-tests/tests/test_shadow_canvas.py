# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore tobytes

import math
import time
from dataclasses import dataclass
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


@dataclass(frozen=True)
class ShadowFamily:
    name: str
    title: str
    direction: int

    @property
    def prefix(self) -> str:
        return f"{self.name}-shadow-"

    @property
    def other(self) -> str:
        return "drop" if self.name == "inner" else "inner"


@pytest.fixture(
    params=[
        ShadowFamily("drop", "Drop shadow", 1),
        ShadowFamily("inner", "Inner shadow", -1),
    ],
    ids=lambda family: family.name,
)
def shadow_family(request):
    return request.param


@pytest.fixture
def shadow_scene(tmp_path, shadow_family):
    source = tmp_path / "Main.slint"
    source.write_bytes(
        (Path(__file__).parents[1] / "fixtures" / "shadow-template.slint")
        .read_bytes()
        .replace(b"@shadow-prefix@", shadow_family.prefix.encode())
    )
    return source


def open_shadow(window, family):
    select_outline_row(window, "shadow-card")
    control = inspector_field(window, family.title, slint_testing.AccessibleRole.Button)
    point = center(control)
    gesture(window, point, point)
    element(window, "Shadow offset handle")


def move_shadow_panel(window, family):
    panel = element(window, id="InspectorShadowPicker::panel")
    files = element(window, "Files")
    start = center(element(window, f"Move {family.title} picker"))
    end = offset_position(
        start,
        files.absolute_position.x - panel.absolute_position.x,
        files.absolute_position.y - panel.absolute_position.y,
        0,
    )
    gesture(window, start, end)


def test_shadow_handles_keep_focus_for_consecutive_edits(
    shadow_family, editor_binary, editor_environment, shadow_scene, tmp_path
):
    baseline = shadow_scene.read_bytes()
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window, shadow_family)
        move_shadow_panel(window, shadow_family)
        expected = baseline
        for name, dx, dy, old, dragged, keyed, key in (
            ("spread", 20, 12, 10, 30, 31, keys.RightArrow),
            ("blur", 20, -8, 32, 48, 49, keys.UpArrow),
        ):
            start = center(element(window, f"Shadow {name} handle"))
            gesture(
                window,
                start,
                offset_position(
                    start,
                    dx * shadow_family.direction,
                    dy * shadow_family.direction,
                    0,
                ),
            )
            expected = expected.replace(
                f"{shadow_family.prefix}{name}: {old}px;".encode(),
                f"{shadow_family.prefix}{name}: {dragged}px;".encode(),
            )
            snapshot.wait_for_applied(expected)
            for handle in ("offset", "blur", "spread"):
                expect(query(window, f"Shadow {handle} handle")).to_be_visible()
            if shadow_family.name == "inner":
                key = keys.LeftArrow if name == "spread" else keys.DownArrow
            press_key(window, key)
            expected = expected.replace(
                f"{shadow_family.prefix}{name}: {dragged}px;".encode(),
                f"{shadow_family.prefix}{name}: {keyed}px;".encode(),
            )
            snapshot.wait_for_applied(expected)


@pytest.mark.parametrize("shift", [keys.Shift, keys.ShiftR])
@pytest.mark.parametrize("dragging", [False, True])
def test_shadow_guides_only_show_while_shift_is_held(
    shadow_family,
    editor_binary,
    editor_environment,
    shadow_scene,
    tmp_path,
    shift,
    dragging,
):
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, shadow_scene.read_bytes())
        window = first_window(editor)
        open_shadow(window, shadow_family)
        move_shadow_panel(window, shadow_family)
        plain = screenshot(window)
        spread = center(element(window, "Shadow spread handle"))
        scale = plain.width / window.root_element.size.width
        color = plain.getpixel((round(spread.x * scale), round((spread.y + 3) * scale)))
        assert isinstance(color, tuple) and len(color) == 3
        assert color[0] > 220 and color[1] > 170 and color[2] < 80

        viewport = element(window, id="EditorUi::canvas-viewport")
        x, y = viewport.absolute_position.x, viewport.absolute_position.y
        bounds = (
            round(x * scale),
            round(y * scale),
            round((x + viewport.size.width) * scale),
            round((y + viewport.size.height) * scale),
        )

        window.dispatch_event(slint_testing.KeyPressedEvent(text=shift))
        guides = screenshot(window)
        assert guides.crop(bounds).tobytes() != plain.crop(bounds).tobytes()
        start = center(element(window, "Shadow offset handle"))
        button = slint_testing.PointerEventButton.Left
        if dragging:
            window.dispatch_event(slint_testing.PointerMoveEvent(start))
            window.dispatch_event(slint_testing.PointerPressEvent(start, button))
            guides = screenshot(window)
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=shift))
        hidden = screenshot(window)
        assert hidden.crop(bounds).tobytes() != guides.crop(bounds).tobytes()
        window.dispatch_event(slint_testing.KeyPressedEvent(text=shift))
        assert (
            screenshot(window).crop(bounds).tobytes() == guides.crop(bounds).tobytes()
        )
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=shift))
        assert (
            screenshot(window).crop(bounds).tobytes() == hidden.crop(bounds).tobytes()
        )
        if dragging:
            window.dispatch_event(slint_testing.PointerReleaseEvent(start, button))
        snapshot.assert_unchanged()


def test_shadow_panel_uses_the_existing_solid_color_picker(
    shadow_family,
    editor_binary,
    editor_environment,
    shadow_scene,
    tmp_path,
):
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, shadow_scene.read_bytes())
        window = first_window(editor)
        open_shadow(window, shadow_family)
        element(window, "Shadow color", role=slint_testing.AccessibleRole.TextInput)
        element(window, "Shadow blur", role=slint_testing.AccessibleRole.Slider)
        element(window, "Shadow spread", role=slint_testing.AccessibleRole.Slider)
        for label in ("Shadow offset X", "Shadow offset Y"):
            element(window, label, role=slint_testing.AccessibleRole.TextInput)
        for label in ("Shadow angle", "Shadow distance", "Hex color"):
            assert not elements(window, label)
        swatch = element(window, "Shadow color color picker")
        gesture(window, center(swatch), center(swatch))
        element(window, "Hex color", role=slint_testing.AccessibleRole.TextInput)
        element(window, "Close Custom").invoke_accessible_default_action()
        expect(query(window, "Hex color")).to_be_hidden()
        element(window, "Shadow blur", role=slint_testing.AccessibleRole.Slider)
        snapshot.assert_unchanged()
        screenshot(window).save(tmp_path / "shadow-picker.png")


@pytest.mark.parametrize(
    "handle,dx,dy,changes",
    [
        ("offset", 20, 12, (("offset-x", 24, 44), ("offset-y", 32, 44))),
        ("blur", 20, -8, (("blur", 32, 48),)),
        ("spread", 20, 12, (("spread", 10, 30),)),
    ],
)
@pytest.mark.parametrize("rotation,percent", [(0, 100), (30, 200)])
@pytest.mark.parametrize("outcome", ["commit", "cancel", "return"])
def test_shadow_canvas_gestures_preview_cancel_and_undo(
    shadow_family,
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
        open_shadow(window, shadow_family)
        move_shadow_panel(window, shadow_family)
        start = center(element(window, f"Shadow {handle} handle"))
        scale = percent / 100 * (shadow_family.direction if handle != "offset" else 1)
        end = offset_position(
            start,
            dx * scale,
            dy * scale,
            math.radians(rotation),
        )
        before = screenshot(window)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        snapshot.assert_unchanged()
        assert screenshot(window).tobytes() != before.tobytes()
        if handle == "offset":
            expect(
                element(
                    window,
                    "Shadow angle and distance",
                    role=slint_testing.AccessibleRole.Text,
                )
            ).to_have_value("45°\nDistance 62 px")
        if outcome == "return":
            window.dispatch_event(slint_testing.PointerMoveEvent(start))
        positions = {
            name: center(element(window, f"Shadow {name} handle"))
            for name in ("offset", "blur", "spread")
        }
        if outcome == "cancel":
            press_key(window, keys.Escape)
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                start if outcome == "return" else end, button
            )
        )
        if outcome != "commit":
            snapshot.assert_unchanged()
        else:
            deadline = time.monotonic() + 0.5
            while time.monotonic() < deadline:
                for name in ("offset", "blur", "spread"):
                    handles = elements(window, f"Shadow {name} handle")
                    assert len(handles) == 1, (
                        f"Shadow {name} handle disappeared after releasing {handle}"
                    )
                    position = center(handles[0])
                    assert (position.x, position.y) == pytest.approx(
                        (positions[name].x, positions[name].y), abs=0.01
                    ), f"Shadow {name} handle jumped after releasing {handle}"
                time.sleep(0.005)
            expected = baseline
            for name, old, new in changes:
                expected = expected.replace(
                    f"{shadow_family.prefix}{name}: {old}px;".encode(),
                    f"{shadow_family.prefix}{name}: {new}px;".encode(),
                )
            snapshot.wait_for_applied(expected)
            for name in ("offset", "blur", "spread"):
                position = center(element(window, f"Shadow {name} handle"))
                assert (position.x, position.y) == pytest.approx(
                    (positions[name].x, positions[name].y), abs=0.01
                )
            element(window, "Shadow blur", role=slint_testing.AccessibleRole.Slider)
            press_shortcut(window, keys.Control, "z")
            snapshot.wait_for_applied(baseline)
            press_shortcut(window, keys.Control, keys.Shift, "z")
            snapshot.wait_for_applied(expected)


@pytest.mark.parametrize(
    "x,y,angle,distance",
    [(3, 4, 53, 5), (-3, 4, 127, 5), (-3, -4, 233, 5), (3, -4, 307, 5), (0, 0, 0, 0)],
)
@pytest.mark.parametrize("rotation,percent", [(0, 100), (30, 200)])
def test_shadow_offset_tooltip_uses_angle_and_distance(
    shadow_family,
    editor_binary,
    editor_environment,
    shadow_scene,
    tmp_path,
    x,
    y,
    angle,
    distance,
    rotation,
    percent,
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
        open_shadow(window, shadow_family)
        move_shadow_panel(window, shadow_family)
        start = center(element(window, "Shadow offset handle"))
        end = offset_position(
            start,
            (x - 24) * percent / 100,
            (y - 32) * percent / 100,
            math.radians(rotation),
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        tooltip = element(
            window, "Shadow angle and distance", role=slint_testing.AccessibleRole.Text
        )
        expect(tooltip).to_have_value(f"{angle}°\nDistance {distance} px")
        handle = center(element(window, "Shadow offset handle"))
        assert tooltip.absolute_position.x + tooltip.size.width < handle.x
        assert tooltip.absolute_position.y + tooltip.size.height < handle.y
        snapshot.assert_unchanged()
        press_key(window, keys.Escape)
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        expect(query(window, "Shadow angle and distance")).to_be_hidden()
        snapshot.assert_unchanged()


@pytest.mark.parametrize("x,y", [(0.01, 0.01), (0.5, 0.01), (0.01, 0.5), (0.99, 0.99)])
def test_shadow_offset_tooltip_stays_inside_the_viewport(
    shadow_family, editor_binary, editor_environment, shadow_scene, tmp_path, x, y
):
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, shadow_scene.read_bytes())
        window = first_window(editor)
        open_shadow(window, shadow_family)
        move_shadow_panel(window, shadow_family)
        viewport = element(window, id="EditorUi::canvas-viewport")
        start = center(element(window, "Shadow offset handle"))
        end = offset_position(
            viewport.absolute_position,
            viewport.size.width * x,
            viewport.size.height * y,
            0,
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        tooltip = element(window, "Shadow angle and distance")
        assert tooltip.absolute_position.x >= viewport.absolute_position.x
        assert tooltip.absolute_position.y >= viewport.absolute_position.y
        assert (
            tooltip.absolute_position.x + tooltip.size.width
            <= viewport.absolute_position.x + viewport.size.width
        )
        assert (
            tooltip.absolute_position.y + tooltip.size.height
            <= viewport.absolute_position.y + viewport.size.height
        )
        press_key(window, keys.Escape)
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        snapshot.assert_unchanged()


@pytest.mark.parametrize("change", ["selection", "source", "effect", "coexist"])
def test_shadow_drag_rejects_a_changed_target(
    shadow_family,
    editor_binary,
    editor_environment,
    shadow_scene,
    tmp_path,
    change,
):
    baseline = shadow_scene.read_bytes()
    external = (
        baseline.replace(
            shadow_family.prefix.encode(),
            b"drop-shadow-" if shadow_family.name == "inner" else b"inner-shadow-",
        )
        if change == "effect"
        else baseline + b"\n// External edit\n"
    )
    if change == "coexist":
        other = shadow_family.other
        external = baseline.replace(
            b"        background: #e5eaff;",
            b"        background: #e5eaff;\n"
            + f"        {other}-shadow-color: #00000040;".encode(),
        )
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window, shadow_family)
        move_shadow_panel(window, shadow_family)
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
            shadow_scene.write_bytes(external)
            snapshot.wait_for_applied(external)
            if change == "effect" or (
                change == "coexist" and shadow_family.name == "inner"
            ):
                expect(query(window, "Shadow offset handle")).to_be_hidden()
            elif change == "coexist":
                expect(query(window, "Shadow offset handle")).to_be_visible()

        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        if change == "selection":
            snapshot.assert_unchanged()
        else:
            snapshot.wait_for_applied(external)


@pytest.mark.parametrize(
    "handle,dx,dy,old,expected",
    [
        ("blur", 0, -100, 32, 128),
        ("blur", 0, 100, 32, 0),
        ("spread", 100, 0, 10, 64),
        ("spread", -100, 0, 10, -64),
    ],
)
def test_shadow_canvas_limits(
    shadow_family,
    editor_binary,
    editor_environment,
    shadow_scene,
    tmp_path,
    handle,
    dx,
    dy,
    old,
    expected,
):
    baseline = shadow_scene.read_bytes()
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window, shadow_family)
        move_shadow_panel(window, shadow_family)
        direction = shadow_family.direction
        start = center(element(window, f"Shadow {handle} handle"))
        gesture(
            window, start, offset_position(start, dx * direction, dy * direction, 0)
        )
        changed = baseline.replace(
            f"{shadow_family.prefix}{handle}: {old}px".encode(),
            f"{shadow_family.prefix}{handle}: {expected}px".encode(),
        )
        snapshot.wait_for_applied(changed)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline)


@pytest.mark.parametrize("dragging", [False, True])
@pytest.mark.parametrize("availability", ["current", "stale"])
def test_removing_shadow_externally_closes_the_session(
    shadow_family,
    editor_binary,
    editor_environment,
    shadow_scene,
    tmp_path,
    dragging,
    availability,
):
    baseline = shadow_scene.read_bytes()
    removed = b"".join(
        line
        for line in baseline.splitlines(keepends=True)
        if not line.lstrip().startswith(shadow_family.prefix.encode())
    )
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window, shadow_family)
        move_shadow_panel(window, shadow_family)
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
        expect(query(window, f"Close {shadow_family.title}")).to_be_hidden()
        if dragging:
            window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        snapshot.wait_for_applied(removed)


def test_picker_undo_closes_a_shadow_created_by_the_previous_edit(
    shadow_family, editor_binary, editor_environment, shadow_scene, tmp_path
):
    baseline = b"".join(
        line
        for line in shadow_scene.read_bytes().splitlines(keepends=True)
        if not line.lstrip().startswith(shadow_family.prefix.encode())
    )
    shadow_scene.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        select_outline_row(window, "shadow-card")
        edit_field(
            window,
            "Rectangle effect",
            shadow_family.name,
            slint_testing.AccessibleRole.Combobox,
        )
        added = wait_for_source_change(shadow_scene, baseline)
        assert f"{shadow_family.prefix}color:".encode() in added
        open_shadow(window, shadow_family)
        move_shadow_panel(window, shadow_family)
        point = center(element(window, f"Move {shadow_family.title} picker"))
        gesture(window, point, point)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline)
        expect(query(window, "Shadow offset handle")).to_be_hidden()
        expect(query(window, f"Close {shadow_family.title}")).to_be_hidden()


@pytest.mark.parametrize("action", ["switch", "Remove shadow"])
@pytest.mark.parametrize("input_method", ["pointer", "keyboard"])
def test_shadow_actions_are_available_without_a_context_menu(
    shadow_family,
    editor_binary,
    editor_environment,
    shadow_scene,
    tmp_path,
    action,
    input_method,
):
    other = shadow_family.other
    if action == "switch":
        action = f"{other.capitalize()} shadow"
    baseline = shadow_scene.read_bytes()
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window, shadow_family)
        if input_method == "pointer":
            point = center(
                element(window, action, role=slint_testing.AccessibleRole.Button)
            )
            gesture(window, point, point)
        else:
            point = center(element(window, f"Move {shadow_family.title} picker"))
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
            for _ in range(len(elements(panel)) + 1):
                press_key(window, keys.Tab)
                if screenshot(window).crop(bounds).tobytes() != unfocused:
                    break
            else:
                raise AssertionError(f"Cannot reach {action} with Tab")
            press_key(window, keys.Return)
        expected = wait_for_source_change(shadow_scene, baseline)
        assert shadow_family.prefix.encode() not in expected
        if action != "Remove shadow":
            assert f"{other}-shadow-color:".encode() in expected
        else:
            assert f"{other}-shadow-".encode() not in expected
        expect(query(window, f"Close {shadow_family.title}")).to_be_hidden()
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline)


@pytest.mark.parametrize("rotation,percent", [(0, 100), (45, 25)])
@pytest.mark.parametrize("handle", ["offset", "blur", "spread"])
def test_small_shadow_handles_remain_individually_draggable(
    shadow_family,
    editor_binary,
    editor_environment,
    shadow_scene,
    handle,
    rotation,
    percent,
):
    baseline = (
        shadow_scene.read_bytes()
        .replace(b"width: 240px", b"width: 10px")
        .replace(b"height: 160px", b"height: 10px")
        .replace(
            f"{shadow_family.prefix}spread: 10px".encode(),
            f"{shadow_family.prefix}spread: -6px".encode(),
        )
        .replace(
            f"{shadow_family.prefix}blur: 32px".encode(),
            f"{shadow_family.prefix}blur: 0px".encode(),
        )
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
        open_shadow(window, shadow_family)
        move_shadow_panel(window, shadow_family)
        start = center(element(window, f"Shadow {handle} handle"))
        end = offset_position(
            start,
            8,
            8 if shadow_family.name == "inner" and handle == "blur" else -8,
            math.radians(rotation),
        )
        gesture(window, start, end)
        changed = wait_for_source_change(shadow_scene, baseline)
        names = ("offset-x", "offset-y") if handle == "offset" else (handle,)
        for name in names:
            before = next(
                line
                for line in baseline.splitlines()
                if f"{shadow_family.prefix}{name}:".encode() in line
            )
            assert before not in changed.splitlines()


def test_shadow_picker_does_not_undo_a_stale_preview(
    shadow_family, editor_binary, editor_environment, shadow_scene, tmp_path
):
    baseline = shadow_scene.read_bytes()
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window, shadow_family)
        move_shadow_panel(window, shadow_family)
        start = center(element(window, "Shadow offset handle"))
        gesture(window, start, offset_position(start, 20, 12, 0))
        changed = wait_for_source_change(shadow_scene, baseline)
        point = center(element(window, f"Move {shadow_family.title} picker"))
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
        gesture(
            window,
            start,
            offset_position(start, 0, 8 if shadow_family.name == "inner" else -8, 0),
        )
        edited = changed.replace(
            f"{shadow_family.prefix}blur: 32px".encode(),
            f"{shadow_family.prefix}blur: 48px".encode(),
        )
        snapshot.wait_for_applied(edited)
        point = center(element(window, f"Move {shadow_family.title} picker"))
        gesture(window, point, point)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(changed)


@pytest.mark.parametrize(
    "expression,expected", [(None, "16"), ("root.shadow-blur", "28")]
)
def test_shadow_blur_uses_inspector_defaults_and_resolved_bindings(
    shadow_family, editor_binary, editor_environment, shadow_scene, expression, expected
):
    baseline = shadow_scene.read_bytes().replace(
        f"        {shadow_family.prefix}blur: 32px;\n".encode(),
        b""
        if expression is None
        else f"        {shadow_family.prefix}blur: {expression};\n".encode(),
    )
    baseline = baseline.replace(
        b"    shadow-card :=",
        b"    in property <length> shadow-blur: 28px;\n    shadow-card :=",
    )
    shadow_scene.write_bytes(baseline)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window, shadow_family)
        expect(
            element(
                window, "Shadow blur value", role=slint_testing.AccessibleRole.TextInput
            )
        ).to_have_value(expected)


def test_shadow_drag_back_to_start_leaves_no_draft_or_history_entry(
    shadow_family, editor_binary, editor_environment, shadow_scene, tmp_path
):
    baseline = shadow_scene.read_bytes()
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        open_shadow(window, shadow_family)
        move_shadow_panel(window, shadow_family)
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
            f"{shadow_family.prefix}offset-x: 24px".encode(),
            f"{shadow_family.prefix}offset-x: 25px".encode(),
        )
        snapshot.wait_for_applied(expected)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline)
