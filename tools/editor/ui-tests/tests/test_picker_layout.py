# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
    picker_panel,
    shifted,
)
from gradient_interactions import click as click_picker_button
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    elements,
    first_window,
    launch_editor,
    press_key,
    select_outline_row,
)


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
    full_source = file.read_bytes()
    if count == 32:
        compact_stops = (
            "red 0deg, lime 180deg, blue 360deg"
            if kind == "conic"
            else "red, lime, blue"
        )
        gradient_document(tmp_path, f"@{kind}-gradient({prefix}, {compact_stops})")
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        if count == 32:
            assert control(window, "Add gradient stop").size.width == 24
            for index in range(1, 4):
                assert control(window, f"Remove stop {index}").size.width == 24
            click_picker_button(window, "Edit stop 2 color")
            click_picker_button(window, "Close Stop color")
            click_picker_button(window, "Close Custom")
            original.assert_unchanged()
            file.write_bytes(full_source)
            wait_for_source(file, full_source)
            original = SourceSnapshot.capture(tmp_path)
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
        assert not elements(
            window,
            f"Gradient stop {count + 1}",
            role=slint_testing.AccessibleRole.Slider,
        )
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_pickers_drag_independently_and_reopen_beside_current_anchor(
    editor_binary, editor_environment, tmp_path
):
    file = gradient_document(tmp_path, "@linear-gradient(90deg, red, lime, blue)")
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        main = picker_panel(window, "main")
        origin = main.absolute_position
        field = control(window, "Rectangle background color picker")
        assert origin.x + main.size.width < field.absolute_position.x
        handle = control(window, "Move Custom picker")
        gesture(window, center(handle), shifted(center(handle), x=-100, y=40))
        assert main.absolute_position.x == pytest.approx(origin.x - 100)
        assert main.absolute_position.y == pytest.approx(origin.y + 40)
        click_picker_button(window, "Edit stop 2 color")
        stop = picker_panel(window, "stop")
        assert stop.absolute_position.x + stop.size.width < main.absolute_position.x
        assert stop.absolute_position.y == pytest.approx(main.absolute_position.y)
        original.assert_unchanged()
        main_position = main.absolute_position
        stop_origin = stop.absolute_position
        handle = control(window, "Move Stop color picker")
        gesture(window, center(handle), shifted(center(handle), x=-40, y=80))
        assert main.absolute_position == main_position
        assert stop.absolute_position.x == pytest.approx(stop_origin.x - 40)
        assert stop.absolute_position.y == pytest.approx(stop_origin.y + 80)
        stop_position = stop.absolute_position
        click_picker_button(window, "Edit stop 3 color")
        assert stop.absolute_position == stop_position
        click_picker_button(window, "Close Stop color")
        handle = control(window, "Move Custom picker")
        gesture(window, center(handle), shifted(center(handle), x=-50, y=30))
        click_picker_button(window, "Edit stop 1 color")
        assert stop.absolute_position.x + stop.size.width < main.absolute_position.x
        assert stop.absolute_position.y == pytest.approx(main.absolute_position.y)
        press_key(window, keys.Escape)
        assert not elements(window, "Close Custom")
        original.assert_unchanged()
        open_gradient(window)
        assert picker_panel(window, "main").absolute_position == origin
        click_picker_button(window, "Close Custom")
        original.assert_unchanged()


@pytest.mark.parametrize(
    "panel_name,title", [("main", "Custom"), ("stop", "Stop color")]
)
def test_picker_drag_is_bounded_and_keeps_close_clickable(
    editor_binary, editor_environment, tmp_path, panel_name, title
):
    file = gradient_document(tmp_path, "@linear-gradient(90deg, red, lime, blue)")
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        if panel_name == "stop":
            click_picker_button(window, "Edit stop 2 color")
        panel = picker_panel(window, panel_name)
        for x, y in [
            (-window.size.width, -window.size.height),
            (window.size.width * 2, window.size.height * 2),
        ]:
            handle = control(window, f"Move {title} picker")
            gesture(window, center(handle), slint_testing.LogicalPosition(x=x, y=y))
            position, size = panel.absolute_position, panel.size
            assert position.x >= 0 and position.y >= 0
            assert position.x + size.width <= window.size.width
            assert position.y + size.height <= window.size.height
        close = control(window, f"Close {title}")
        assert close.size.width == close.size.height
        gesture(window, center(close), center(close))
        assert not elements(window, f"Close {title}")
        if panel_name == "stop":
            control(window, "Close Custom")
            press_key(window, keys.Escape)
        original.assert_unchanged()


@pytest.mark.parametrize("panel_name", ["main", "stop"])
def test_overlapping_pickers_raise_the_dragged_panel(
    editor_binary, editor_environment, tmp_path, panel_name
):
    file = gradient_document(tmp_path, "@linear-gradient(90deg, red, lime, blue)")
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        click_picker_button(window, "Edit stop 2 color")
        main, stop = picker_panel(window, "main"), picker_panel(window, "stop")
        handle = control(window, "Move Custom picker")
        gesture(
            window,
            center(handle),
            shifted(
                center(handle),
                x=stop.absolute_position.x + 80 - main.absolute_position.x,
                y=stop.absolute_position.y + 60 - main.absolute_position.y,
            ),
        )
        if panel_name == "main":
            point, covered = center(control(window, "Solid")), stop
        else:
            handle = control(window, "Move Stop color picker")
            gesture(window, center(handle), shifted(center(handle), x=20, y=120))
            point, covered = center(control(window, "Close Stop color")), main
        position, size = covered.absolute_position, covered.size
        assert position.x < point.x < position.x + size.width
        assert position.y < point.y < position.y + size.height
        gesture(window, point, point)
        assert not elements(window, "Close Stop color")
        control(window, "Close Custom")
        if panel_name == "main":
            picker_field(window, "Hex color")
        else:
            control(window, "Add gradient stop")
        press_key(window, keys.Escape)
        original.assert_unchanged()
