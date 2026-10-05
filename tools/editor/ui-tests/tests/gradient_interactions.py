# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
from pathlib import Path

import slint_testing
from canvas_interactions import center as element_center
from ui_driver import element, elements, select_outline_row


def control(window, label, role=slint_testing.AccessibleRole.Button):
    return element(window, label, role=role)


def click(window, label):
    control(window, label).invoke_accessible_default_action()


def center(element, rotation=0):
    return element_center(element, math.radians(rotation))


def shifted(point, x=0, y=0):
    return slint_testing.LogicalPosition(x=point.x + x, y=point.y + y)


def around(c, radius, degrees):
    angle = math.radians(degrees - 90)
    return shifted(c, x=radius * math.cos(angle), y=radius * math.sin(angle))


def gesture(window, start, end):
    button = slint_testing.PointerEventButton.Left
    window.dispatch_event(slint_testing.PointerMoveEvent(start))
    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    window.dispatch_event(slint_testing.PointerMoveEvent(end))
    window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))


def picker_field(window, label, role=slint_testing.AccessibleRole.TextInput):
    return control(window, label, role)


def open_gradient(window):
    click(window, "Rectangle background color picker")


def open_radial(window):
    select_outline_row(window, "fill")
    open_gradient(window)
    control(window, "Gradient center handle")
    assert not elements(window, "Gradient center")
    assert not elements(window, "Gradient radius mode")
    control(window, "Add gradient stop")


def gradient_document(directory: Path, expression: str) -> Path:
    file = directory / "Gradient.slint"
    file.write_text(f"""export component Gradient inherits Window {{
    width: 400px;
    height: 400px;
    VerticalLayout {{
        fill := Rectangle {{ background: {expression}; }}
    }}
}}
""")
    return file


def picker_panel(window, name):
    return element(window, id=f"InspectorFillPicker::{name}-panel")


def move_picker_to_files(window, name):
    panel = picker_panel(window, name)
    title = "Custom" if name == "main" else "Stop color"
    handle = control(window, f"Move {title} picker")
    files = control(window, "Files", slint_testing.AccessibleRole.Tree)
    point = center(handle)
    gesture(
        window,
        point,
        shifted(
            point,
            x=files.absolute_position.x - panel.absolute_position.x,
            y=files.absolute_position.y - panel.absolute_position.y,
        ),
    )
    canvas = element(window, "Editor canvas")
    assert panel.absolute_position.x + panel.size.width <= canvas.absolute_position.x
