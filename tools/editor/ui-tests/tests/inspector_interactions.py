# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint_testing
from ui_assertions import expect
from ui_driver import element, elements

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
    track = element(slider, id="InspectorSlider::track")
    rect = track.absolute_rect
    return slint_testing.LogicalPosition(
        x=rect.x + rect.width * progress, y=rect.y + rect.height / 2
    )


def inspector_field(
    window: slint_testing.Window,
    label: str,
    role: slint_testing.AccessibleRole | None = None,
) -> slint_testing.Element:
    pane = element(
        window, "Inspector and outline", role=slint_testing.AccessibleRole.Complementary
    )
    pane_rect = pane.absolute_rect
    position = slint_testing.LogicalPosition(
        x=pane_rect.x + pane_rect.width / 2,
        y=pane_rect.y + pane_rect.height / 4,
    )
    divider = element(
        window, "Outline pane resize", role=slint_testing.AccessibleRole.Slider
    )
    for delta in [0, 10000, -180, -180, -180, -180, -180, -180]:
        if delta:
            window.dispatch_event(
                slint_testing.PointerScrolledEvent(position, delta_x=0, delta_y=delta)
            )
        fields = elements(pane, label, role=role)
        if len(fields) == 1 and (
            pane.absolute_position.y
            <= fields[0].absolute_position.y + fields[0].size.height / 2
            <= divider.absolute_position.y
        ):
            return element(pane, label, role=role)
    return element(window, label, role=role)


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
    expect(inspector_field(window, label, role)).to_have_value(value, timeout=timeout)
