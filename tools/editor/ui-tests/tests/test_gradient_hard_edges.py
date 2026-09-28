# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore tobytes

from io import BytesIO
from pathlib import Path

from editor_sync import wait_for_source
from gradient_interactions import (
    center,
    gesture,
    gradient_document,
    open_gradient,
)
from PIL import Image
from source_snapshot import SourceSnapshot, wait_for_source_change


def test_coincident_canvas_insertion_preserves_rendering(editor_factory, tmp_path):
    file = gradient_document(
        tmp_path, "@linear-gradient(90deg, red 0%, red 50%, blue 50%, blue 100%)"
    )
    original = SourceSnapshot.capture(tmp_path)
    with editor_factory(file) as editor:
        wait_for_source(file, file.read_bytes())
        window = editor.window
        editor.outline.select("fill")
        open_gradient(window)
        start = center(window.get_by_role("button", name="Gradient start").resolve())
        end = center(window.get_by_role("button", name="Gradient end").resolve())
        point = type(start)(x=(start.x + end.x) / 2, y=(start.y + end.y) / 2)

        def pixels():
            image = Image.open(BytesIO(window.screenshot()))
            return image.crop(
                (int(start.x + 2), int(start.y - 70), int(end.x - 2), int(start.y - 60))
            ).tobytes()

        before = pixels()
        for _ in range(2):
            gesture(window, point, point)
        window.get_by_role("button", name="Gradient stop 5").wait_for()
        assert pixels() == before
        window.get_by_role("button", name="Close Custom").activate()
        saved = wait_for_source_change(file, original.sources[Path(file.name)])
        original.wait_for_applied(saved, file.name)
        assert (
            b"#ff0000 0%, #ff0000 50%, #0000ff 50%, #0000ff 50%, #0000ff 100%" in saved
        )
        open_gradient(window)
        assert pixels() == before
