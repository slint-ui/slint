# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest
import slint_testing
from editor_sync import wait_for_source
from gradient_interactions import (
    center,
    gesture,
    gradient_document,
    open_gradient,
    picker_field,
    shifted,
)
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    first_window,
    launch_editor,
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
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        assert (
            window.get_by_role("button", name="Add gradient stop").resolve().size.width
            == 24
        )
        for index in range(1, 4):
            assert (
                window.get_by_role("button", name=f"Remove stop {index}")
                .resolve()
                .size.width
                == 24
            )
        assert (
            window.get_by_role("button", name="Close Custom").resolve().size.width == 48
        )
        window.get_by_role("button", name="Edit stop 2 color").activate()
        assert (
            window.get_by_role("button", name="Close Stop color").resolve().size.width
            == 48
        )
        (tmp_path / f"picker-{kind}.png").write_bytes(window.screenshot())
        window.get_by_role("button", name="Close Stop color").activate()
        window.get_by_role("button", name="Close Custom").activate()
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
        wait_for_source(file, file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        first = picker_field(window, "Stop 1 position")
        top = first.bounds().y
        scroll_point = first.center()
        window.pointer.scroll(0, -10000, at=scroll_point)
        last = picker_field(window, f"Stop {count} position")
        last_bounds = last.bounds()
        assert last_bounds.y >= top
        bottom = window.get_by_role(
            slint_testing.AccessibleRole.Text, name="No recent fills"
        ).resolve()
        assert last_bounds.y + last_bounds.height < bottom.absolute_position.y
        assert bottom.absolute_position.y + bottom.size.height <= window.size.height - 8
        if count == 2:
            assert first.bounds().y == top
        (tmp_path / f"picker-{kind}-{count}-stops.png").write_bytes(window.screenshot())
        window.get_by_role("button", name="Add gradient stop").activate()
        window.get_by_role(
            slint_testing.AccessibleRole.Slider, name=f"Gradient stop {count + 1}"
        ).resolve()
        window.pointer.scroll(0, -10000, at=scroll_point)
        remove = window.get_by_role("button", name=f"Remove stop {count + 1}").resolve()
        point = shifted(center(remove), x=-8)
        assert top <= point.y < bottom.absolute_position.y
        gesture(window, point, point)
        assert not window.get_by_role(
            slint_testing.AccessibleRole.Slider, name=f"Gradient stop {count + 1}"
        ).all()
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()
