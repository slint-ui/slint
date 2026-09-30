# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint_testing
from canvas_interactions import center
from ui_driver import (
    elements_with_label,
    screenshot,
    wait_until,
    window_element_with_label,
)

FIELDS = {
    "x": "Position X",
    "y": "Position Y",
    "width": "Width",
    "height": "Height",
    "rotation": "Rotation",
    "radius": "Corner radius",
}


def slider_position(
    window: slint_testing.Window, label: str, progress: float
) -> slint_testing.LogicalPosition:
    slider = inspector_field(window, label, slint_testing.AccessibleRole.Slider)
    return slider_track_position(slider, progress)


def slider_track_position(
    slider: slint_testing.Element, progress: float
) -> slint_testing.LogicalPosition:
    track = slider.query_descendants().match_id("InspectorSlider::track").find_all()
    assert len(track) == 1
    position, size = track[0].absolute_position, track[0].size
    return slint_testing.LogicalPosition(
        x=position.x + size.width * progress, y=position.y + size.height / 2
    )


def inspector_field(
    window: slint_testing.Window,
    label: str,
    role: slint_testing.AccessibleRole | None = None,
) -> slint_testing.Element:
    pane = window_element_with_label(
        window, "Inspector and outline", slint_testing.AccessibleRole.Complementary
    )
    position = slint_testing.LogicalPosition(
        x=pane.absolute_position.x + pane.size.width / 2,
        y=pane.absolute_position.y + pane.size.height / 4,
    )
    divider = window_element_with_label(
        window, "Outline pane resize", slint_testing.AccessibleRole.Slider
    )
    for delta in [0, 10000, -180, -180, -180, -180, -180, -180]:
        if delta:
            window.dispatch_event(
                slint_testing.PointerScrolledEvent(position, delta_x=0, delta_y=delta)
            )
        fields = elements_with_label(pane, label, role)
        if len(fields) == 1 and (
            pane.absolute_position.y
            <= fields[0].absolute_position.y + fields[0].size.height / 2
            <= divider.absolute_position.y
        ):
            return fields[0]
    return window_element_with_label(window, label, role)


def inspector_text_input(
    window: slint_testing.Window, label: str
) -> slint_testing.Element:
    field = inspector_field(window, label, slint_testing.AccessibleRole.TextInput)
    inputs = (
        field.query_descendants().match_id("InspectorTextFieldBase::input").find_all()
    )
    assert len(inputs) == 1
    return inputs[0]


def click_field(
    window: slint_testing.Window, label: str, *, on_text: bool = True
) -> None:
    field = inspector_field(window, label, slint_testing.AccessibleRole.TextInput)
    input = inspector_text_input(window, label)
    position = slint_testing.LogicalPosition(
        x=input.absolute_position.x + 8
        if on_text
        else field.absolute_position.x + field.size.width - 2,
        y=center(input).y,
    )
    button = slint_testing.PointerEventButton.Left
    window.dispatch_event(slint_testing.PointerMoveEvent(position))
    window.dispatch_event(slint_testing.PointerPressEvent(position, button))
    # A real click allows focus changes to settle before later pointer events.
    screenshot(window)
    window.dispatch_event(slint_testing.PointerMoveEvent(position))
    window.dispatch_event(slint_testing.PointerReleaseEvent(position, button))


def edit_field(
    window: slint_testing.Window,
    label: str,
    value: str,
    role: slint_testing.AccessibleRole | None = None,
) -> None:
    inspector_field(window, label, role).accessible_value = value


def wait_for_field(
    window: slint_testing.Window,
    label: str,
    value: str,
    role: slint_testing.AccessibleRole | None = None,
    timeout: float = 5,
) -> None:
    wait_until(
        lambda: (
            field
            if (field := inspector_field(window, label, role)).accessible_value == value
            else None
        ),
        timeout=timeout,
    )
