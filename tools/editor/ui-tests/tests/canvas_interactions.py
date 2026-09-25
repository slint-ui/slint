# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math

import slint_testing
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    elements_with_label,
    palette_row,
    press_shortcut,
    wait_until,
    window_element_with_label,
)

Frame = tuple[float, float, float, float]


def begin_palette_drag(
    window: slint_testing.Window,
    kind: str,
    target: slint_testing.LogicalPosition,
) -> None:
    row = wait_until(
        lambda: (
            candidate
            if (candidate := palette_row(window, kind)).accessible_enabled
            else None
        )
    )
    start = center(row)
    button = slint_testing.PointerEventButton.Left
    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    window.dispatch_event(
        slint_testing.PointerMoveEvent(
            slint_testing.LogicalPosition(x=start.x + 16, y=start.y + 16)
        )
    )
    window.dispatch_event(slint_testing.PointerMoveEvent(target))


def center(
    element: slint_testing.Element, rotation: float = 0.0
) -> slint_testing.LogicalPosition:
    """The center of `element` in window coordinates, `rotation` in radians.

    `absolute_position` maps the element's origin through the rotation of its ancestors, while
    `size` is measured along the element's own axes.
    So the offset from that origin to the center has to be rotated as well.
    """
    position = element.absolute_position
    size = element.size
    return offset_position(position, size.width / 2, size.height / 2, rotation)


def offset_position(
    position: slint_testing.LogicalPosition, dx: float, dy: float, rotation: float
) -> slint_testing.LogicalPosition:
    cosine = math.cos(rotation)
    sine = math.sin(rotation)
    return slint_testing.LogicalPosition(
        x=position.x + dx * cosine - dy * sine,
        y=position.y + dx * sine + dy * cosine,
    )


def position_distance(
    left: slint_testing.LogicalPosition, right: slint_testing.LogicalPosition
) -> float:
    return math.hypot(left.x - right.x, left.y - right.y)


def element_frame(element: slint_testing.Element) -> Frame:
    assert element.is_valid
    position = element.absolute_position
    size = element.size
    return (position.x, position.y, size.width, size.height)


def selection_frame(window: slint_testing.Window, kind: str) -> Frame:
    return element_frame(
        window_element_with_label(
            window, f"Selected {kind}", slint_testing.AccessibleRole.Region
        )
    )


def frame_rotation(window: slint_testing.Window, kind: str) -> float:
    """The rotation of the selection frame of `kind`, in radians.

    The move handle covers the whole frame, so its reported origin is the frame's top-left
    corner mapped through the rotation, and the angle to it from the frame's center recovers
    the rotation.
    """
    x, y, width, height = selection_frame(window, kind)
    origin = window_element_with_label(window, f"{kind} move handle").absolute_position
    return math.atan2(
        origin.y - (y + height / 2), origin.x - (x + width / 2)
    ) - math.atan2(-height / 2, -width / 2)


def fixture_element(window: slint_testing.Window, kind: str) -> slint_testing.Element:
    return wait_until(
        lambda: next(
            iter(window.find_elements_by_id(f"Main::root-{kind.lower()}")), None
        )
    )


def hover_fixture_element(
    window: slint_testing.Window, kind: str
) -> slint_testing.Element:
    window.dispatch_event(
        slint_testing.PointerMoveEvent(center(fixture_element(window, kind)))
    )
    return window_element_with_label(
        window, f"Hovered {kind}", slint_testing.AccessibleRole.Region
    )


def same_state(left: Frame, right: Frame) -> bool:
    return all(abs(a - b) < 0.01 for a, b in zip(left, right))


def manual_drag(
    window: slint_testing.Window,
    handle: slint_testing.Element,
    dx: float,
    dy: float,
    snapshot: SourceSnapshot,
    shift: bool = False,
    fixed_handle_label: str | None = None,
    follow_pointer: bool = True,
) -> slint_testing.LogicalPosition | None:
    button = slint_testing.PointerEventButton.Left
    kind = handle.accessible_label.split(" ", 1)[0]
    rotation = frame_rotation(window, kind)
    start = center(handle, rotation)
    end = slint_testing.LogicalPosition(x=start.x + dx, y=start.y + dy)
    selected = window_element_with_label(
        window, f"Selected {kind}", slint_testing.AccessibleRole.Region
    )
    initial_frame = element_frame(selected)
    fixed_handle_center = (
        center(window_element_with_label(window, fixed_handle_label), rotation)
        if fixed_handle_label is not None
        else None
    )

    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    if shift:
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))

    transient_states = []
    for step in range(1, 4):
        fraction = step / 3
        position = slint_testing.LogicalPosition(
            x=start.x + dx * fraction,
            y=start.y + dy * fraction,
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(position))
        transient_states.append(element_frame(selected))

    assert transient_states[-1] != initial_frame
    assert len(set(transient_states)) >= 2
    if fixed_handle_center is not None:
        assert fixed_handle_label is not None
        if follow_pointer:
            assert (
                position_distance(
                    center(
                        window_element_with_label(window, handle.accessible_label),
                        rotation,
                    ),
                    end,
                )
                < 1.5
            )
        assert (
            position_distance(
                center(window_element_with_label(window, fixed_handle_label), rotation),
                fixed_handle_center,
            )
            < 1.5
        )

    snapshot.assert_unchanged_now()
    window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
    if shift:
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
    return fixed_handle_center


def _rotation_tooltip_value(window: slint_testing.Window) -> int:
    tooltip = window_element_with_label(
        window, "Rotation angle", slint_testing.AccessibleRole.Text
    )
    return int(tooltip.accessible_value)


def rotation_start(handle: slint_testing.Element) -> slint_testing.LogicalPosition:
    position = center(handle)
    corner = handle.accessible_label.rsplit(" ", 1)[-1]
    return slint_testing.LogicalPosition(
        x=position.x + handle.size.width / 4 * (-1 if "left" in corner else 1),
        y=position.y + handle.size.height / 4 * (-1 if "top" in corner else 1),
    )


def manual_rotation_drag(
    window: slint_testing.Window,
    handle: slint_testing.Element,
    dx: float,
    dy: float,
    snapshot: SourceSnapshot,
    *,
    crosses_zero: bool = False,
    kind: str = "Text",
    target_angle: int = 15,
) -> None:
    start = rotation_start(handle)
    end = slint_testing.LogicalPosition(x=start.x + dx, y=start.y + dy)
    button = slint_testing.PointerEventButton.Left
    # A rotation turns the element around its center, so the position and the size of the
    # selection frame stay where they are and its rotation is what follows the drag.
    initial_rotation = frame_rotation(window, kind)
    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
    angles = [_rotation_tooltip_value(window)]
    for step in range(1, 4):
        fraction = step / 3
        position = slint_testing.LogicalPosition(
            x=start.x + dx * fraction,
            y=start.y + dy * fraction,
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(position))
        angles.append(_rotation_tooltip_value(window))
    final_rotation = frame_rotation(window, kind)
    snapshot.assert_unchanged_now()

    assert all(0 <= angle < 360 for angle in angles)
    assert angles[-1] == target_angle or crosses_zero
    turned = (final_rotation - initial_rotation + math.pi) % (2 * math.pi) - math.pi
    assert abs(turned) > math.radians(1)
    if crosses_zero:
        assert any(angle >= 345 for angle in angles)
        assert any(angle <= 15 for angle in angles)
    window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
    window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))


def rotation_delta(
    window: slint_testing.Window,
    handle: slint_testing.Element,
    degrees: float,
    kind: str = "Text",
) -> tuple[float, float]:
    x, y, width, height = selection_frame(window, kind)
    frame_center = slint_testing.LogicalPosition(
        x=x + width / 2,
        y=y + height / 2,
    )
    start = rotation_start(handle)
    radians = math.radians(degrees)
    cosine = math.cos(radians)
    sine = math.sin(radians)
    relative_x = start.x - frame_center.x
    relative_y = start.y - frame_center.y
    target_x = frame_center.x + relative_x * cosine - relative_y * sine
    target_y = frame_center.y + relative_x * sine + relative_y * cosine
    return target_x - start.x, target_y - start.y


def live_modifier_resize(
    window: slint_testing.Window,
    snapshot: SourceSnapshot,
    *,
    press_shift_during_drag: bool,
) -> None:
    handle = window_element_with_label(window, "Rectangle resize bottom-right")
    start = center(handle)
    end = slint_testing.LogicalPosition(x=start.x + 20, y=start.y + 16)
    button = slint_testing.PointerEventButton.Left
    if not press_shift_during_drag:
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    window.dispatch_event(slint_testing.PointerMoveEvent(end))
    before_modifier = selection_frame(window, "Rectangle")
    snapshot.assert_unchanged_now()

    modifier_event = (
        slint_testing.KeyPressedEvent(text=keys.Shift)
        if press_shift_during_drag
        else slint_testing.KeyReleasedEvent(text=keys.Shift)
    )
    window.dispatch_event(modifier_event)
    after_modifier = wait_until(
        lambda: (
            frame
            if (frame := selection_frame(window, "Rectangle")) != before_modifier
            else None
        )
    )
    if press_shift_during_drag:
        assert after_modifier[2] == after_modifier[3]
    else:
        assert after_modifier[2] != after_modifier[3]
    snapshot.assert_unchanged_now()

    window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
    if press_shift_during_drag:
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))


def manual_radius_drag(
    window: slint_testing.Window,
    handle: slint_testing.Element,
    dx: float,
    dy: float,
    snapshot: SourceSnapshot,
    *,
    shift: bool = False,
) -> None:
    start = center(handle)
    target = slint_testing.LogicalPosition(x=start.x + dx, y=start.y + dy)
    button = slint_testing.PointerEventButton.Left
    if shift:
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    tooltip = window_element_with_label(
        window, "Radius value", slint_testing.AccessibleRole.Text
    )
    initial_value = float(tooltip.accessible_value)
    window.dispatch_event(slint_testing.PointerMoveEvent(target))
    assert tooltip.is_valid
    wait_until(
        lambda: (
            value
            if (value := float(tooltip.accessible_value)) != initial_value
            else None
        )
    )
    snapshot.assert_unchanged_now()
    window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
    if shift:
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))


def radius_handle(window: slint_testing.Window, corner: str) -> slint_testing.Element:
    selection = window_element_with_label(
        window, "Selected Rectangle", slint_testing.AccessibleRole.Region
    )
    # A live reload can replace the frame while the pointer remains at the same logical
    # position. Move away first so the real frame receives a fresh hover transition.
    window.dispatch_event(
        slint_testing.PointerMoveEvent(slint_testing.LogicalPosition(x=1, y=1))
    )
    window.dispatch_event(slint_testing.PointerMoveEvent(center(selection)))
    return window_element_with_label(window, f"Rectangle radius {corner}")


OrientedFrame = tuple[float, float, float, float, float]


def rotated_handle_center(
    element: slint_testing.Element, angle: float = 0
) -> tuple[float, float]:
    midpoint = center(element, math.radians(angle))
    return (midpoint.x, midpoint.y)


def oriented_selection_frame(
    window: slint_testing.Window, kind: str
) -> OrientedFrame | None:
    handles = []
    for corner in ("top-left", "top-right", "bottom-right"):
        matches = elements_with_label(window.root_element, f"{kind} resize {corner}")
        if len(matches) != 1:
            return None
        handles.append(matches[0])
    a, b, c = (rotated_handle_center(handle) for handle in handles)
    angle = math.degrees(math.atan2(b[1] - a[1], b[0] - a[0]))
    corrected_a = rotated_handle_center(handles[0], angle)
    corrected_c = rotated_handle_center(handles[2], angle)
    return (
        (corrected_a[0] + corrected_c[0]) / 2,
        (corrected_a[1] + corrected_c[1]) / 2,
        math.dist(a, b),
        math.dist(b, c),
        angle,
    )


def zoom_canvas(window: slint_testing.Window, percent: int) -> None:
    levels = [25, 50, 75, 100, 125, 150, 200, 300, 400]
    modifier = keys.Control
    canvas = window_element_with_label(window, "Editor canvas")
    current = levels.index(int(canvas.accessible_value.removesuffix("%")))
    target = levels.index(percent)
    direction = 1 if target > current else -1
    for index in range(current + direction, target + direction, direction):
        press_shortcut(window, modifier, "+" if direction > 0 else "-")
        wait_until(
            lambda index=index: (
                True if canvas.accessible_value == f"{levels[index]}%" else None
            )
        )


def center_canvas_selection(
    window: slint_testing.Window, kind: str = "Rectangle"
) -> None:
    canvas = window_element_with_label(window, "Editor canvas")
    frame = window_element_with_label(window, f"Selected {kind}")
    target = center(canvas)
    window.dispatch_event(
        slint_testing.PointerScrolledEvent(
            target,
            delta_x=target.x - center(frame).x,
            delta_y=target.y - center(frame).y,
        )
    )
    wait_until(
        lambda: (
            True
            if abs(center(frame).x - target.x) < 0.01
            and abs(center(frame).y - target.y) < 0.01
            else None
        )
    )
