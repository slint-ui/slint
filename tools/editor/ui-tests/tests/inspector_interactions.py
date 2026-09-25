# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint_testing
from ui_driver import elements_with_label, wait_until, window_element_with_label

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
    for delta in [0, 10000, -180, -180, -180, -180, -180, -180]:
        if delta:
            window.dispatch_event(
                slint_testing.PointerScrolledEvent(position, delta_x=0, delta_y=delta)
            )
        fields = elements_with_label(pane, label, role)
        if len(fields) == 1:
            return fields[0]
    return window_element_with_label(window, label, role)


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
