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
        for label in ("Shadow angle", "Shadow distance", "Hex color"):
            assert not elements(window, label)
        swatch = element(window, "Shadow color color picker")
        gesture(window, center(swatch), center(swatch))
        element(window, "Hex color", role=slint_testing.AccessibleRole.TextInput)
        element(window, "Close Fill").invoke_accessible_default_action()
        expect(query(window, "Hex color")).to_be_hidden()
        element(window, "Shadow blur", role=slint_testing.AccessibleRole.Slider)
        snapshot.assert_unchanged()
        screenshot(window).save(tmp_path / "shadow-picker.png")


@pytest.mark.parametrize(
    "rotation,percent,radius",
    [(0, 100, 12), (30, 200, 12), (45, 100, 72), (90, 100, 12), (-45, 100, 72)],
)
def test_shadow_handles_follow_the_rotated_guides(
    shadow_family,
    editor_binary,
    editor_environment,
    shadow_scene,
    tmp_path,
    rotation,
    percent,
    radius,
):
    baseline = (
        shadow_scene.read_bytes()
        .replace(
            b"        x: 160px;",
            f"        transform-rotation: {rotation}deg;\n        x: 160px;".encode(),
        )
        .replace(b"border-radius: 12px", f"border-radius: {radius}px".encode())
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
        offset = center(element(window, "Shadow offset handle"))
        angle = math.radians(rotation)
        for handle, expansion in (("blur", 26), ("spread", 10)):
            point = center(element(window, f"Shadow {handle} handle"))
            dx = (point.x - offset.x) / (percent / 100)
            dy = (point.y - offset.y) / (percent / 100)
            if handle == "blur":
                assert dx == pytest.approx(0, abs=0.01)
                assert dy < 0
            else:
                assert dx > 0
                assert dy == pytest.approx(0, abs=0.01)
            expansion *= shadow_family.direction
            half_width, half_height = 120 + expansion, 80 + expansion
            guide_radius = max(0, min(radius + expansion, half_width, half_height))
            x = abs(dx * math.cos(angle) + dy * math.sin(angle))
            y = abs(-dx * math.sin(angle) + dy * math.cos(angle))
            qx, qy = x - half_width + guide_radius, y - half_height + guide_radius
            distance = (
                math.hypot(max(qx, 0), max(qy, 0)) + min(max(qx, qy), 0) - guide_radius
            )
            assert distance == pytest.approx(0, abs=0.01)
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
        screenshot(window).save(tmp_path / "shadow-guide-handles.png")
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
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
            math.radians(rotation) if handle == "offset" else 0,
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
            if change == "effect":
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
        inspector_field(
            window,
            f"Enable {shadow_family.title.lower()}",
            slint_testing.AccessibleRole.Switch,
        ).invoke_accessible_default_action()
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


@pytest.mark.parametrize("other_enabled", [False, True])
def test_shadow_toggle_keeps_unrelated_controls_visually_stable(
    shadow_family,
    editor_binary,
    editor_environment,
    shadow_scene,
    tmp_path,
    other_enabled,
):
    baseline = shadow_scene.read_bytes()
    if other_enabled:
        baseline = baseline.replace(
            b"        background: #e5eaff;",
            f"        background: #e5eaff;\n        {shadow_family.other}-shadow-color: #12345678;".encode(),
        )
        shadow_scene.write_bytes(baseline)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        select_outline_row(window, "shadow-card")
        switch = inspector_field(
            window,
            f"Enable {shadow_family.title.lower()}",
            slint_testing.AccessibleRole.Switch,
        )
        other = inspector_field(
            window,
            f"Enable {shadow_family.other} shadow",
            slint_testing.AccessibleRole.Switch,
        )
        run = element(window, "Start preview", role=slint_testing.AccessibleRole.Button)
        window.dispatch_event(slint_testing.PointerMoveEvent(center(switch)))
        before = screenshot(window)
        scale = before.width / window.root_element.size.width
        selection = element(
            window, "Selected Rectangle", role=slint_testing.AccessibleRole.Region
        )
        rect = selection.absolute_rect
        blue_pixels = []
        for y in range(
            round((rect.y - 5) * scale), round((rect.y + rect.height + 5) * scale)
        ):
            for x in range(
                round((rect.x - 5) * scale), round((rect.x + rect.width + 5) * scale)
            ):
                pixel = before.getpixel((x, y))
                if (
                    isinstance(pixel, tuple)
                    and pixel[0] < 100
                    and pixel[1] > 100
                    and pixel[2] > 230
                ):
                    blue_pixels.append((x, y, pixel))
        assert len(blue_pixels) > 100
        controls = {}
        for name, control in (("other-switch", other), ("run", run)):
            position, size = control.absolute_position, control.size
            bounds = (
                round(position.x * scale),
                round(position.y * scale),
                round((position.x + size.width) * scale),
                round((position.y + size.height) * scale),
            )
            controls[name] = (bounds, before.crop(bounds).tobytes())
        for _ in range(2):
            gesture(window, center(switch), center(switch))
            changed = set()
            deadline = time.monotonic() + 0.8
            while time.monotonic() < deadline:
                frame = screenshot(window, settle=False)
                if any(frame.getpixel((x, y)) != pixel for x, y, pixel in blue_pixels):
                    if "selection-overlay" not in changed:
                        frame.save(tmp_path / "flashing-selection-overlay.png")
                    changed.add("selection-overlay")
                for name, (bounds, pixels) in controls.items():
                    if frame.crop(bounds).tobytes() != pixels and name not in changed:
                        frame.save(tmp_path / f"flashing-{name}.png")
                        changed.add(name)
            baseline = wait_for_source_change(shadow_scene, baseline)
            assert not changed, f"Unrelated controls flashed: {sorted(changed)}"
        screenshot(window).save(tmp_path / "stable-shadow-toggle.png")

        shadow_scene.write_bytes(baseline.replace(b"width: 240px;", b"width: ;"))
        expect(
            query(window, "Editing paused until the preview is available")
        ).to_be_visible()
        expect(switch).to_be_enabled(False)
        expect(run).to_be_enabled(False)
        assert run.computed_opacity < 1


def test_shadow_switch_focus_ring_requires_tab_navigation(
    shadow_family, editor_binary, editor_environment, shadow_scene, tmp_path
):
    baseline = shadow_scene.read_bytes()
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        select_outline_row(window, "shadow-card")
        switch = inspector_field(
            window,
            f"Enable {shadow_family.title.lower()}",
            slint_testing.AccessibleRole.Switch,
        )
        ring = query(switch, id="InspectorSwitch::switch-focus-ring")
        gesture(window, center(switch), center(switch))
        disabled = wait_for_source_change(shadow_scene, baseline)
        expect.poll(lambda: switch.accessible_checked).to_equal(False)
        expect(ring).to_be_hidden()
        press_key(window, keys.Space)
        enabled = wait_for_source_change(shadow_scene, disabled)
        expect.poll(lambda: switch.accessible_checked).to_equal(True)
        expect(ring).to_be_hidden()
        screenshot(window).save(tmp_path / "pointer-focus.png")

        press_key(window, keys.Tab)
        press_shortcut(window, keys.Shift, keys.Tab)
        expect(ring).to_be_visible()
        screenshot(window).save(tmp_path / "keyboard-focus.png")
        press_key(window, keys.Space)
        wait_for_source_change(shadow_scene, enabled)
        expect.poll(lambda: switch.accessible_checked).to_equal(False)
        expect(ring).to_be_visible()

        gesture(window, center(switch), center(switch))
        expect.poll(lambda: switch.accessible_checked).to_equal(True)
        expect(ring).to_be_hidden()


@pytest.mark.parametrize("other_enabled", [False, True])
def test_shadow_switches_enable_independently_and_support_keyboard_undo(
    shadow_family,
    editor_binary,
    editor_environment,
    shadow_scene,
    tmp_path,
    other_enabled,
):
    original = shadow_scene.read_bytes()
    baseline = b"".join(
        line
        for line in original.splitlines(keepends=True)
        if not line.lstrip().startswith(shadow_family.prefix.encode())
    )
    other_prefix = f"{shadow_family.other}-shadow-".encode()
    if other_enabled:
        baseline = baseline.replace(
            b"        background: #e5eaff;",
            b"        background: #e5eaff;\n        "
            + other_prefix
            + b"color: #12345678;\n"
            + b"        "
            + other_prefix
            + b"blur: 23px;",
        )
    shadow_scene.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        select_outline_row(window, "shadow-card")
        control = inspector_field(
            window, shadow_family.title, slint_testing.AccessibleRole.Button
        )
        expect(control).to_be_enabled(False)
        control.invoke_accessible_default_action()
        gesture(window, center(control), center(control))
        expect(query(window, "Shadow offset handle")).to_be_hidden()
        snapshot.assert_unchanged()
        switch = inspector_field(
            window,
            f"Enable {shadow_family.title.lower()}",
            slint_testing.AccessibleRole.Switch,
        )
        other_switch = inspector_field(
            window,
            f"Enable {shadow_family.other} shadow",
            slint_testing.AccessibleRole.Switch,
        )
        expect.poll(lambda: switch.accessible_checked).to_equal(False)
        expect.poll(lambda: other_switch.accessible_checked).to_equal(other_enabled)
        screenshot(window).save(tmp_path / "disabled-shadow-rows.png")
        gesture(window, center(switch), center(switch))
        added = wait_for_source_change(shadow_scene, baseline)
        properties = [
            ("color", "#00000030" if shadow_family.name == "inner" else "#00000040"),
            ("blur", "16px"),
            ("spread", "0px"),
            ("offset-x", "0px"),
            ("offset-y", "8px"),
        ]
        if shadow_family.name == "inner":
            anchor = b"        background: #e5eaff;"
            bindings = b"".join(
                f"        {shadow_family.prefix}{name}: {value};\n".encode()
                for name, value in properties
            )
            expected = baseline.replace(anchor, bindings + anchor)
        else:
            anchor = b"        height: 160px;"
            bindings = b"".join(
                f"        {shadow_family.prefix}{name}: {value};\n".encode()
                for name, value in properties
                if name != "blur"
            )
            expected = baseline.replace(anchor, bindings + anchor).replace(
                b"        border-radius: 12px;",
                b"        border-radius: 12px;\n        drop-shadow-blur: 16px;",
            )
        snapshot.wait_for_applied(expected)
        assert added == expected
        expect(control).to_be_enabled()
        expect.poll(lambda: switch.accessible_checked).to_equal(True)
        expect.poll(lambda: other_switch.accessible_checked).to_equal(other_enabled)
        press_key(window, keys.Space)
        snapshot.wait_for_applied(baseline)
        expect(control).to_be_enabled(False)
        expect.poll(lambda: switch.accessible_checked).to_equal(False)
        expect.poll(lambda: other_switch.accessible_checked).to_equal(other_enabled)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(added)
        expect(control).to_be_enabled()
        expect.poll(lambda: switch.accessible_checked).to_equal(True)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(baseline)
        expect(control).to_be_enabled(False)
        expect.poll(lambda: switch.accessible_checked).to_equal(False)


def test_shadow_switch_rejects_stale_preview(
    shadow_family, editor_binary, editor_environment, shadow_scene, tmp_path
):
    baseline = shadow_scene.read_bytes()
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        select_outline_row(window, "shadow-card")
        switch = inspector_field(
            window,
            f"Enable {shadow_family.title.lower()}",
            slint_testing.AccessibleRole.Switch,
        )
        shadow_scene.write_bytes(baseline + b"\ninvalid Slint\n")
        element(window, "Stale preview", role=slint_testing.AccessibleRole.Region)
        snapshot = SourceSnapshot.capture(tmp_path)
        expect(switch).to_be_enabled(False)
        switch.invoke_accessible_default_action()
        gesture(window, center(switch), center(switch))
        snapshot.assert_unchanged()


def test_both_shadow_pickers_edit_only_their_own_properties(
    shadow_family, editor_binary, editor_environment, shadow_scene, tmp_path
):
    baseline = shadow_scene.read_bytes()
    baseline = baseline.replace(
        b"        background: #e5eaff;",
        b"        background: #e5eaff;\n"
        + f"        {shadow_family.other}-shadow-color: #12345678;\n"
        f"        {shadow_family.other}-shadow-blur: 23px;".encode(),
    )
    shadow_scene.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, shadow_scene) as editor:
        wait_for_source(shadow_scene, baseline)
        window = first_window(editor)
        select_outline_row(window, "shadow-card")
        inspector_field(
            window,
            "Shadow blur value",
            slint_testing.AccessibleRole.TextInput,
            shadow=shadow_family.title,
        )
        screenshot(window).save(tmp_path / "both-shadow-picker.png")
        panel = element(window, id="InspectorShadowPicker::panel")
        expect(query(panel, "Remove shadow")).to_be_hidden()
        expect(
            query(
                panel,
                f"{shadow_family.other.capitalize()} shadow",
                role=slint_testing.AccessibleRole.Button,
            )
        ).to_be_hidden()
        edit_field(
            window, "Shadow blur value", "48", slint_testing.AccessibleRole.TextInput
        )
        edited = baseline.replace(
            f"{shadow_family.prefix}blur: 32px".encode(),
            f"{shadow_family.prefix}blur: 48px".encode(),
        )
        snapshot.wait_for_applied(edited)
        expect(query(window, "Shadow offset handle")).to_be_visible()
        move_shadow_panel(window, shadow_family)
        other_switch = inspector_field(
            window,
            f"Enable {shadow_family.other} shadow",
            slint_testing.AccessibleRole.Switch,
        )
        gesture(window, center(other_switch), center(other_switch))
        without_other = b"".join(
            line
            for line in edited.splitlines(keepends=True)
            if not line.lstrip().startswith(f"{shadow_family.other}-shadow-".encode())
        )
        snapshot.wait_for_applied(without_other)
        expect(query(window, "Shadow offset handle")).to_be_hidden()
        open_shadow(window, shadow_family)
        expect(
            element(
                panel, "Shadow blur value", role=slint_testing.AccessibleRole.TextInput
            )
        ).to_have_value("48")
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(edited)
        switch = inspector_field(
            window,
            f"Enable {shadow_family.title.lower()}",
            slint_testing.AccessibleRole.Switch,
        )
        gesture(window, center(switch), center(switch))
        removed = b"".join(
            line
            for line in edited.splitlines(keepends=True)
            if not line.lstrip().startswith(shadow_family.prefix.encode())
        )
        snapshot.wait_for_applied(removed)
        expect(query(window, "Shadow offset handle")).to_be_hidden()
        expect(query(window, f"Close {shadow_family.title}")).to_be_hidden()
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(edited)
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
            math.radians(rotation) if handle == "offset" else 0,
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
