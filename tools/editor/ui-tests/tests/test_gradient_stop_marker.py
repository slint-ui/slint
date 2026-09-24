# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path
from typing import cast

import pytest
import slint_testing
from editor_sync import wait_for_source
from gradient_interactions import (
    center,
    control,
    gesture,
    gradient_document,
    open_gradient,
    picker_field,
    shifted,
)
from gradient_interactions import click as click_picker_button
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    press_key,
    screenshot,
    select_outline_row,
)


@pytest.mark.parametrize("kind", ["linear", "radial", "conic"])
@pytest.mark.parametrize(
    "role",
    [slint_testing.AccessibleRole.Slider, slint_testing.AccessibleRole.Button],
    ids=["ramp", "canvas"],
)
def test_stop_marker_click_opens_color_picker_but_drag_does_not(
    editor_binary: Path,
    editor_environment: dict[str, str],
    tmp_path: Path,
    kind: str,
    role: slint_testing.AccessibleRole,
) -> None:
    prefix = {"linear": "90deg", "radial": "circle", "conic": "from 0deg"}[kind]
    positions = (
        ("0deg", "180deg", "360deg") if kind == "conic" else ("0%", "50%", "100%")
    )
    expression = f"@{kind}-gradient({prefix}, red {positions[0]}, #0000ff80 {positions[1]}, white {positions[2]})"
    source_file = gradient_document(tmp_path, expression)
    original = SourceSnapshot.capture(tmp_path)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)

        marker = control(window, "Gradient stop 2", role)
        assert marker.size.width == pytest.approx(40)
        assert marker.size.height == pytest.approx(40)
        start = center(marker)
        gesture(window, start, start)
        assert picker_field(window, "Hex color").accessible_value == "#0000ff80"
        click_picker_button(window, "Close Stop color")

        before = float(picker_field(window, "Stop 2 position").accessible_value)
        start = center(control(window, "Gradient stop 2", role))
        gesture(window, start, shifted(start, x=12))
        after = float(picker_field(window, "Stop 2 position").accessible_value)
        assert after != pytest.approx(before)
        assert not elements_with_label(window.root_element, "Hex color")

        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_ramp_marker_accessible_action_opens_existing_color_picker(
    editor_binary: Path,
    editor_environment: dict[str, str],
    tmp_path: Path,
) -> None:
    source_file = gradient_document(
        tmp_path, "@linear-gradient(90deg, red 0%, #abcdef80 50%, white 100%)"
    )
    original = SourceSnapshot.capture(tmp_path)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)

        control(
            window, "Gradient stop 2", slint_testing.AccessibleRole.Slider
        ).invoke_accessible_default_action()
        assert picker_field(window, "Hex color").accessible_value == "#abcdef80"
        click_picker_button(window, "Close Stop color")
        click_picker_button(window, "Close Custom")
        original.assert_unchanged()


def test_ramp_marker_shows_opaque_and_alpha_halves(
    editor_binary: Path,
    editor_environment: dict[str, str],
    tmp_path: Path,
) -> None:
    source_file = gradient_document(
        tmp_path, "@linear-gradient(90deg, red 0%, #0000ff80 50%, white 100%)"
    )

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)

        marker = control(window, "Gradient stop 2", slint_testing.AccessibleRole.Slider)
        image = screenshot(window)
        x = round(marker.absolute_position.x)
        y = round(marker.absolute_position.y)
        assert marker.size.width == pytest.approx(40)
        assert marker.size.height == pytest.approx(40)
        opaque = cast(tuple[int, int, int], image.getpixel((x + 14, y + 23)))
        translucent = cast(tuple[int, int, int], image.getpixel((x + 26, y + 23)))
        horizontal_tick = cast(tuple[int, int, int], image.getpixel((x + 4, y + 23)))
        bottom_tick = cast(tuple[int, int, int], image.getpixel((x + 20, y + 37)))
        pointer = cast(tuple[int, int, int], image.getpixel((x + 20, y + 4)))

        assert opaque[0] < 16 and opaque[1] < 16 and opaque[2] > 239
        assert translucent[0] > 31 and translucent[1] > 31
        assert translucent[2] > translucent[0] and translucent[2] > translucent[1]
        assert max(horizontal_tick) < 48
        assert max(bottom_tick) < 48
        assert min(pointer) > 224
