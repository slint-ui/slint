# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore tobytes

from io import BytesIO
from pathlib import Path

from PIL import Image

from source_snapshot import SourceSnapshot
from test_gradient_geometry import gradient_document, open_gradient
from test_linear_gradient_canvas import center, control, gesture
from ui_driver import first_window, launch_editor, select_outline_row, wait_until


def test_coincident_canvas_insertion_preserves_rendering(
    editor_binary, editor_environment, tmp_path
):
    file = gradient_document(
        tmp_path, "@linear-gradient(90deg, red 0%, red 50%, blue 50%, blue 100%)"
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, file) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        open_gradient(window)
        start = center(control(window, "Gradient start"))
        end = center(control(window, "Gradient end"))
        point = type(start)(x=(start.x + end.x) / 2, y=(start.y + end.y) / 2)

        def pixels():
            image = Image.open(BytesIO(window.grab_window_as_png()))
            return image.crop(
                (int(start.x + 2), int(start.y - 70), int(end.x - 2), int(start.y - 60))
            ).tobytes()

        before = pixels()
        for _ in range(2):
            gesture(window, point, point)
        control(window, "Gradient stop 5")
        assert pixels() == before
        control(window, "Close Custom").invoke_accessible_default_action()
        saved = wait_until(
            lambda: file.read_bytes()
            if file.read_bytes() != original.sources[Path(file.name)]
            else None
        )
        original.wait_for_applied(saved, file.name)
        assert (
            b"#ff0000 0%, #ff0000 50%, #0000ff 50%, #0000ff 50%, #0000ff 100%" in saved
        )
        open_gradient(window)
        assert pixels() == before
