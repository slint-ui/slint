# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest
import slint_testing
from slint_testing import keys

from source_snapshot import SourceSnapshot
from test_gradient_geometry import (
    click_picker_button,
    gradient_document,
    open_gradient,
    picker_field,
)
from test_linear_gradient_canvas import center, control, gesture, shifted
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    press_key,
    select_outline_row,
)


@pytest.mark.parametrize("kind", ["linear", "radial", "conic"])
def test_fixed_picker_controls_keep_their_width(
    editor_binary, editor_environment, tmp_path, kind
):
    prefix = {"linear": "90deg", "radial": "circle", "conic": "from 0deg"}[kind]
    stops = (
        "red 0deg, lime 180deg, blue 360deg" if kind == "conic" else "red, lime, blue"
    )
    file = gradient_document(tmp_path, f"@{kind}-gradient({prefix}, {stops})")
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        assert control(window, "Add gradient stop").size.width == 24
        for index in range(1, 4):
            assert control(window, f"Remove stop {index}").size.width == 24
        assert control(window, "Close Custom").size.width == 48
        click_picker_button(window, "Edit stop 2 color")
        assert control(window, "Close Stop color").size.width == 48
        (tmp_path / f"picker-{kind}.png").write_bytes(window.grab_window_as_png())
        click_picker_button(window, "Close Stop color")
        click_picker_button(window, "Close Custom")
        original.assert_unchanged()


@pytest.mark.parametrize("kind", ["linear", "radial", "conic"])
@pytest.mark.parametrize("count", [2, 32])
def test_stop_list_sizes_and_scrolls_after_insertion_and_deletion(
    editor_binary, editor_environment, tmp_path, kind, count
):
    prefix = {"linear": "90deg", "radial": "circle", "conic": "from 0deg"}[kind]
    units, suffix = (360, "deg") if kind == "conic" else (100, "%")
    stops = ", ".join(
        f"{'red' if index % 2 else 'blue'} {index * units / (count - 1)}{suffix}"
        for index in range(count)
    )
    file = gradient_document(tmp_path, f"@{kind}-gradient({prefix}, {stops})")
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        first = picker_field(window, "Stop 1 position")
        top = first.absolute_position.y
        scroll_point = center(first)
        window.dispatch_event(
            slint_testing.PointerScrolledEvent(scroll_point, delta_x=0, delta_y=-10000)
        )
        last = picker_field(window, f"Stop {count} position")
        assert last.absolute_position.y >= top
        bottom = control(window, "No recent fills", slint_testing.AccessibleRole.Text)
        assert last.absolute_position.y + last.size.height < bottom.absolute_position.y
        assert bottom.absolute_position.y + bottom.size.height <= window.size.height - 8
        if count == 2:
            assert first.absolute_position.y == top
        (tmp_path / f"picker-{kind}-{count}-stops.png").write_bytes(
            window.grab_window_as_png()
        )
        click_picker_button(window, "Add gradient stop")
        control(
            window, f"Gradient stop {count + 1}", slint_testing.AccessibleRole.Slider
        )
        window.dispatch_event(
            slint_testing.PointerScrolledEvent(scroll_point, delta_x=0, delta_y=-10000)
        )
        remove = control(window, f"Remove stop {count + 1}")
        point = shifted(center(remove), x=-8)
        assert top <= point.y < bottom.absolute_position.y
        gesture(window, point, point)
        assert not elements_with_label(
            window.root_element,
            f"Gradient stop {count + 1}",
            slint_testing.AccessibleRole.Slider,
        )
        press_key(window, keys.Escape)
        original.assert_unchanged()
