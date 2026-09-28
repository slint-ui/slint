# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import slint_testing
from slint_test import Locator, Window, expect

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
    slider: slint_testing.Element, progress: float
) -> slint_testing.LogicalPosition:
    track = slider.query_descendants().match_id("InspectorSlider::track").find_all()
    assert len(track) == 1
    position, size = track[0].absolute_position, track[0].size
    return slint_testing.LogicalPosition(
        x=position.x + size.width * progress, y=position.y + size.height / 2
    )


def inspector_field(
    window: Window,
    label: str,
    role: slint_testing.AccessibleRole | None = None,
) -> slint_testing.Element:
    pane = window.get_by_role("complementary", name="Inspector and outline")
    field = (
        pane.get_by_accessible_name(label)
        if role is None
        else pane.get_by_role(role, name=label)
    )
    matches = field.all()
    if len(matches) == 1:
        pane_element = pane.resolve()
        element = matches[0]
        center_y = element.absolute_position.y + element.size.height / 2
        top = pane_element.absolute_position.y
        if top <= center_y <= top + pane_element.size.height:
            return element
    field.scroll_into_view()
    return field.resolve()


def inspector_field_locator(
    window: Window,
    label: str,
    role: slint_testing.AccessibleRole | None = None,
) -> Locator:
    pane = window.get_by_role("complementary", name="Inspector and outline")
    return (
        pane.get_by_accessible_name(label)
        if role is None
        else pane.get_by_role(role, name=label)
    )


def edit_field(
    window: Window,
    label: str,
    value: str,
    role: slint_testing.AccessibleRole | None = None,
) -> None:
    field = inspector_field_locator(window, label, role)
    field.scroll_into_view()
    field.set_accessible_value(value)


def wait_for_field(
    window: Window,
    label: str,
    value: str,
    role: slint_testing.AccessibleRole | None = None,
    timeout: float = 5,
) -> None:
    field = inspector_field_locator(window, label, role)
    field.scroll_into_view()
    expect(field).to_have_value(value, timeout=timeout * 1000)
