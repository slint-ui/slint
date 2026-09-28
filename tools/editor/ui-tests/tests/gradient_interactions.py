# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
from pathlib import Path

import slint_testing
from canvas_interactions import center as element_center
from slint_test import Locator
from ui_driver import select_outline_row


def center(element, rotation=0):
    return element_center(element, math.radians(rotation))


def shifted(point, x=0, y=0):
    return slint_testing.LogicalPosition(x=point.x + x, y=point.y + y)


def around(c, radius, degrees):
    angle = math.radians(degrees - 90)
    return shifted(c, x=radius * math.cos(angle), y=radius * math.sin(angle))


def gesture(window, start, end):
    window.pointer.move_to(start)
    window.pointer.press_at(start)
    window.pointer.move_to(end)
    window.pointer.release_at(end)


def picker_field(
    window, label, role=slint_testing.AccessibleRole.TextInput
) -> Locator:
    return window.get_by_role(role, name=label)


def open_gradient(window):
    window.get_by_role("button", name="Rectangle background color picker").activate()


def open_radial(window):
    select_outline_row(window, "fill")
    open_gradient(window)
    window.get_by_role("button", name="Gradient center handle").resolve()
    assert not window.get_by_accessible_name("Gradient center").all()
    assert not window.get_by_accessible_name("Gradient radius mode").all()
    window.get_by_role("button", name="Add gradient stop").resolve()


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
