# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import time

import slint_testing
from ui_driver import (
    elements_with_label,
    find_window_element_with_label,
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


def inspector_field(
    window: slint_testing.Window,
    label: str,
    role: slint_testing.AccessibleRole | None = None,
    *,
    timeout: float = 5,
) -> slint_testing.Element:
    deadline = time.monotonic() + timeout
    pane = window_element_with_label(
        window,
        "Inspector and outline",
        slint_testing.AccessibleRole.Complementary,
        timeout=max(0, deadline - time.monotonic()),
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
    return window_element_with_label(
        window, label, role, timeout=max(0, deadline - time.monotonic())
    )


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
    deadline = time.monotonic() + timeout
    inspector_field(window, label, role, timeout=timeout)
    wait_until(
        lambda: (
            field
            if (field := find_window_element_with_label(window, label, role))
            is not None
            and field.accessible_value == value
            else None
        ),
        deadline=deadline,
    )
