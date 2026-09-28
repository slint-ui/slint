# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint_testing
from ui_driver import wait_until
from ui_locators import Locator, Window

FIELDS = {
    "x": "Position X",
    "y": "Position Y",
    "width": "Width",
    "height": "Height",
    "rotation": "Rotation",
    "radius": "Corner radius",
}


def slider_position(
    window: Window, label: str, progress: float
) -> slint_testing.LogicalPosition:
    slider = inspector_field(window, label, slint_testing.AccessibleRole.Slider)
    return slider_track_position(slider, progress)


def slider_track_position(
    slider: Locator, progress: float
) -> slint_testing.LogicalPosition:
    track = slider.get_by_id("InspectorSlider::track")
    return track.read(
        lambda element: slint_testing.LogicalPosition(
            x=element.absolute_position.x + element.size.width * progress,
            y=element.absolute_position.y + element.size.height / 2,
        )
    )


def inspector_field(
    window: Window,
    label: str,
    role: slint_testing.AccessibleRole | None = None,
) -> Locator:
    pane_locator = window.get_by_role(
        slint_testing.AccessibleRole.Complementary, name="Inspector and outline"
    )
    pane = pane_locator
    position = slint_testing.LogicalPosition(
        x=pane.absolute_position.x + pane.size.width / 2,
        y=pane.absolute_position.y + pane.size.height / 4,
    )
    divider = window.get_by_role(
        slint_testing.AccessibleRole.Slider, name="Outline pane resize"
    )
    for delta in [0, 10000, -180, -180, -180, -180, -180, -180]:
        if delta:
            window.dispatch_event(
                slint_testing.PointerScrolledEvent(position, delta_x=0, delta_y=delta)
            )
        field_locator = (
            pane_locator.get_by_accessible_name(label)
            if role is None
            else pane_locator.get_by_role(role, name=label)
        )
        fields = field_locator.all()
        if len(fields) == 1 and (
            pane.absolute_position.y
            <= fields[0].absolute_position.y + fields[0].size.height / 2
            <= divider.absolute_position.y
        ):
            return field_locator
    return (
        window.get_by_accessible_name(label)
        if role is None
        else window.get_by_role(role, name=label)
    )


def edit_field(
    window: Window,
    label: str,
    value: str,
    role: slint_testing.AccessibleRole | None = None,
) -> None:
    inspector_field(window, label, role).accessible_value = value


def wait_for_field(
    window: Window,
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
