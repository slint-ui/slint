# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math

import slint_testing
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import elements_with_label, wait_until, window_element_with_label

Frame = tuple[float, float, float, float]


def center(element: slint_testing.Element) -> slint_testing.LogicalPosition:
    position = element.absolute_position
    size = element.size
    return slint_testing.LogicalPosition(
        x=position.x + size.width / 2,
        y=position.y + size.height / 2,
    )


def position_distance(
    left: slint_testing.LogicalPosition, right: slint_testing.LogicalPosition
) -> float:
    return math.hypot(left.x - right.x, left.y - right.y)


def selection_frame(window: slint_testing.Window, kind: str) -> Frame:
    selected = window_element_with_label(
        window, f"Selected {kind}", slint_testing.AccessibleRole.Region
    )
    position = selected.absolute_position
    size = selected.size
    return (position.x, position.y, size.width, size.height)


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
    require_multiple_transient_states: bool = True,
    fixed_handle_label: str | None = None,
) -> slint_testing.LogicalPosition | None:
    start = center(handle)
    end = slint_testing.LogicalPosition(x=start.x + dx, y=start.y + dy)
    button = slint_testing.PointerEventButton.Left
    kind = handle.accessible_label.split(" ", 1)[0]
    initial_frame = selection_frame(window, kind)
    fixed_handle_center = (
        center(window_element_with_label(window, fixed_handle_label))
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
        transient_states.append(selection_frame(window, kind))

    assert transient_states[-1] != initial_frame
    if require_multiple_transient_states:
        assert len(set(transient_states)) >= 2
    if fixed_handle_center is not None:
        assert fixed_handle_label is not None
        assert (
            position_distance(
                center(window_element_with_label(window, handle.accessible_label)), end
            )
            < 1.5
        )
        assert (
            position_distance(
                center(window_element_with_label(window, fixed_handle_label)),
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


def manual_rotation_drag(
    window: slint_testing.Window,
    handle: slint_testing.Element,
    dx: float,
    dy: float,
    snapshot: SourceSnapshot,
    *,
    crosses_zero: bool = False,
) -> None:
    start = center(handle)
    end = slint_testing.LogicalPosition(x=start.x + dx, y=start.y + dy)
    button = slint_testing.PointerEventButton.Left
    initial_frame = selection_frame(window, "Text")
    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
    angles = [_rotation_tooltip_value(window)]
    frames = []
    for step in range(1, 4):
        fraction = step / 3
        position = slint_testing.LogicalPosition(
            x=start.x + dx * fraction,
            y=start.y + dy * fraction,
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(position))
        angles.append(_rotation_tooltip_value(window))
        frames.append(selection_frame(window, "Text"))
    snapshot.assert_unchanged_now()

    assert all(0 <= angle < 360 for angle in angles)
    assert angles[-1] == 15 or crosses_zero
    assert frames[-1] != initial_frame
    if crosses_zero:
        assert any(angle >= 345 for angle in angles)
        assert any(angle <= 15 for angle in angles)
    window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
    window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))


def rotation_delta(
    window: slint_testing.Window,
    handle: slint_testing.Element,
    degrees: float,
) -> tuple[float, float]:
    x, y, width, height = selection_frame(window, "Text")
    frame_center = slint_testing.LogicalPosition(
        x=x + width / 2,
        y=y + height / 2,
    )
    start = center(handle)
    radians = math.radians(degrees)
    cosine = math.cos(radians)
    sine = math.sin(radians)
    relative_x = start.x - frame_center.x
    relative_y = start.y - frame_center.y
    target_x = frame_center.x + relative_x * cosine - relative_y * sine
    target_y = frame_center.y + relative_x * sine + relative_y * cosine
    return target_x - start.x, target_y - start.y


def cancel_pointer_interaction(
    window: slint_testing.Window,
    handle: slint_testing.Element,
    dx: float,
    dy: float,
    snapshot: SourceSnapshot,
    kind: str,
) -> None:
    start = center(handle)
    target = slint_testing.LogicalPosition(x=start.x + dx, y=start.y + dy)
    frame_kind = "Rectangle" if kind == "Rectangle-radius" else kind
    initial_frame = selection_frame(window, frame_kind)
    button = slint_testing.PointerEventButton.Left
    readout_label = {
        "Rectangle-radius": "Radius value",
        "Text": "Rotation angle",
    }.get(kind)

    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    if kind == "Text":
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
    initial_readout = (
        float(
            window_element_with_label(
                window, readout_label, slint_testing.AccessibleRole.Text
            ).accessible_value
        )
        if readout_label is not None
        else None
    )
    window.dispatch_event(slint_testing.PointerMoveEvent(target))

    if readout_label is None:
        wait_until(
            lambda: (
                frame
                if (frame := selection_frame(window, frame_kind)) != initial_frame
                else None
            )
        )
    else:
        wait_until(
            lambda: (
                value
                if (
                    value := float(
                        window_element_with_label(
                            window, readout_label, slint_testing.AccessibleRole.Text
                        ).accessible_value
                    )
                )
                != initial_readout
                else None
            )
        )
    snapshot.assert_unchanged_now()

    window.dispatch_event(slint_testing.PointerExitedEvent())
    window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
    if kind == "Text":
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
    wait_until(
        lambda: (
            True
            if same_state(selection_frame(window, frame_kind), initial_frame)
            else None
        )
    )
    assert not elements_with_label(window.root_element, "Rotation angle")
    assert not elements_with_label(window.root_element, "Radius value")
    snapshot.assert_unchanged()


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
    initial_value = float(
        window_element_with_label(
            window, "Radius value", slint_testing.AccessibleRole.Text
        ).accessible_value
    )
    window.dispatch_event(slint_testing.PointerMoveEvent(target))
    wait_until(
        lambda: (
            value
            if (
                value := float(
                    window_element_with_label(
                        window, "Radius value", slint_testing.AccessibleRole.Text
                    ).accessible_value
                )
            )
            != initial_value
            else None
        )
    )
    snapshot.assert_unchanged_now()
    window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
    if shift:
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
