# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import wait_for_source
from gradient_interactions import gesture
from inspector_interactions import inspector_field
from PIL import Image
from slint_testing import keys
from ui_assertions import expect
from ui_driver import (
    element,
    first_window,
    launch_editor,
    press_key,
    query,
    screenshot,
    select_outline_row,
)


def color_difference(
    image: Image.Image, position: tuple[int, int], expected: tuple[int, ...]
) -> int:
    pixel = image.getpixel(position)
    assert isinstance(pixel, tuple)
    return max(abs(a - b) for a, b in zip(pixel, expected))


@pytest.mark.parametrize(
    "kind", ["solid", "gradient", "drop", "inner", "drop-color", "inner-color"]
)
def test_picker_anchor_keeps_its_panel_and_focus(
    editor_binary: Path,
    editor_environment: dict[str, str],
    tmp_path: Path,
    kind: str,
) -> None:
    fill = (
        "@linear-gradient(90deg, #124133, #abcdef)" if kind == "gradient" else "#124133"
    )
    family = kind.removesuffix("-color")
    shadow = f"{family}-shadow-color: #124133;" if family in ("drop", "inner") else ""
    shadow_color = kind.endswith("-color")
    color_value = kind == "solid" or shadow_color
    source = tmp_path / "Main.slint"
    source.write_text(f"""export component Main inherits Window {{
    width: 400px;
    height: 400px;
    card := Rectangle {{
        width: 200px;
        height: 200px;
        background: {fill};
        {shadow}
    }}
}}
""")
    baseline = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_outline_row(window, "card")
        reference = inspector_field(
            window, "Width", slint_testing.AccessibleRole.TextInput
        )
        reference.invoke_accessible_default_action()
        reference_point = (
            round(reference.absolute_position.x),
            round(center(reference).y),
        )
        focus_border = screenshot(window).getpixel(reference_point)
        assert isinstance(focus_border, tuple)

        title = {"drop": "Drop shadow", "inner": "Inner shadow"}.get(kind, "Fill")
        label = (
            title if kind in ("drop", "inner") else "Rectangle background color picker"
        )
        if shadow_color:
            control = inspector_field(
                window,
                {"drop": "Drop shadow", "inner": "Inner shadow"}[family],
                slint_testing.AccessibleRole.Button,
            )
            point = center(control)
            gesture(window, point, point)
            anchor = element(
                window,
                "Shadow color color picker",
                role=slint_testing.AccessibleRole.Button,
            )
        else:
            anchor = inspector_field(window, label, slint_testing.AccessibleRole.Button)
        if color_value:
            field = element(
                window,
                "Shadow color" if shadow_color else "Rectangle background",
                role=slint_testing.AccessibleRole.TextInput,
            )
        elif kind == "gradient":
            field = element(window, id="InspectorColorField::gradient-field")
        else:
            field = anchor
        edge = (round(field.absolute_position.x), round(center(field).y))
        panel_label = (
            "Hex color"
            if color_value
            else {"gradient": "Stop 1 position"}.get(kind, "Shadow color")
        )

        for attempt in range(3):
            point = center(anchor)
            button = slint_testing.PointerEventButton.Left
            window.dispatch_event(slint_testing.PointerMoveEvent(point))
            window.dispatch_event(slint_testing.PointerPressEvent(point, button))
            if attempt:
                assert len(query(window, f"Close {title}").find_all()) == 1
                held = screenshot(window)
                assert color_difference(held, edge, focus_border) <= 2
            window.dispatch_event(slint_testing.PointerReleaseEvent(point, button))
            expect(query(window, f"Close {title}")).to_be_visible()
            expect.poll(
                lambda: color_difference(screenshot(window), edge, focus_border) <= 2,
                message="picker anchor has its focus ring",
            ).to_equal(True)
            panel_field = element(
                window, panel_label, role=slint_testing.AccessibleRole.TextInput
            )
            panel_value = panel_field.accessible_value
            press_key(window, "5")
            expect(panel_field).to_have_value(panel_value)
            if color_value:
                expect(field).to_have_value("124133")
            assert source.read_bytes() == baseline

            if attempt == 0:
                panel_field.invoke_accessible_default_action()
                assert color_difference(screenshot(window), edge, focus_border) > 2

        if kind == "gradient":
            press_key(window, keys.Escape)
            expect(query(window, "Close Fill")).to_be_hidden()

        if color_value:
            for activation in (keys.Return, keys.Space):
                point = center(field)
                gesture(window, point, point)
                expect(query(window, "Close Fill")).to_be_hidden()
                element(
                    window,
                    ("Shadow color" if shadow_color else "Rectangle background")
                    + " opacity",
                    role=slint_testing.AccessibleRole.TextInput,
                ).invoke_accessible_default_action()
                press_key(window, keys.Tab)
                press_key(window, activation)
                expect(query(window, "Close Fill")).to_be_visible()
                assert source.read_bytes() == baseline
            point = center(field)
            gesture(window, point, point)
            expect(query(window, "Close Fill")).to_be_hidden()
            press_key(window, "A")
            expect(field).to_have_value("A")
