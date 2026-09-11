# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest

from source_snapshot import SourceSnapshot
from test_gradient_geometry import (
    click_picker_button,
    gradient_document,
    open_gradient,
)
from test_linear_gradient_canvas import control
from ui_driver import first_window, launch_editor, select_outline_row


@pytest.mark.parametrize("kind", ["linear", "radial", "conic"])
def test_fixed_picker_controls_keep_their_width(
    editor_binary, editor_environment, tmp_path, kind
):
    prefix = {"linear": "90deg", "radial": "circle", "conic": "from 0deg"}[kind]
    stops = "red 0deg, lime 180deg, blue 360deg" if kind == "conic" else "red, lime, blue"
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
