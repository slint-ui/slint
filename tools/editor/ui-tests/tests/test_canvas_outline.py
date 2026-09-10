# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
from io import BytesIO
from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from PIL import Image
from source_snapshot import SourceSnapshot
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    select_outline_row,
    window_element_with_label,
)


@pytest.mark.parametrize("angle", [0, 30])
@pytest.mark.parametrize("selected", [False, True])
def test_canvas_outline_preserves_item_border(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tmp_path: Path,
    angle: int,
    selected: bool,
) -> None:
    source = fixture_project / "Main.slint"
    source.write_text(
        f"""export component Main inherits Window {{
    width: 400px;
    height: 400px;
    background: white;
    outlined := Rectangle {{
        x: 100px;
        y: 100px;
        width: 100px;
        height: 100px;
        background: white;
        border-width: 3px;
        border-color: black;
        transform-rotation: {angle}deg;
    }}
}}
"""
    )
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, source.read_bytes())
        artboard = window_element_with_label(window, "Artboard")
        if selected:
            select_outline_row(window, "outlined")
        window.dispatch_event(
            slint_testing.PointerMoveEvent(
                slint_testing.LogicalPosition(
                    x=artboard.absolute_position.x + 150,
                    y=artboard.absolute_position.y + 150,
                )
            )
        )
        label = "Selected Rectangle" if selected else "Hovered Rectangle"
        frame = window_element_with_label(window, label)
        assert frame.size.width == pytest.approx(100)
        assert frame.size.height == pytest.approx(100)
        if selected:
            assert not elements_with_label(window.root_element, "Hovered Rectangle")
        png = window.grab_window_as_png()
        (tmp_path / "outline.png").write_bytes(png)
        rendered = Image.open(BytesIO(png)).convert("RGB")
        scale = rendered.width / window.size.width
        radians = math.radians(angle)

        def pixel(x: float, y: float) -> tuple[int, int, int]:
            dx, dy = x - 50, y - 50
            absolute_x = (
                artboard.absolute_position.x
                + 150
                + dx * math.cos(radians)
                - dy * math.sin(radians)
            )
            absolute_y = (
                artboard.absolute_position.y
                + 150
                + dx * math.sin(radians)
                + dy * math.cos(radians)
            )
            color = rendered.getpixel(
                (int(absolute_x * scale), int(absolute_y * scale))
            )
            assert isinstance(color, tuple)
            return color[0], color[1], color[2]

        for side in range(4):

            def edge_pixel(distance: float, side: int = side) -> tuple[int, int, int]:
                return pixel(
                    *[
                        (distance, 50),
                        (100 - distance, 50),
                        (50, distance),
                        (50, 100 - distance),
                    ][side]
                )

            assert min(edge_pixel(-4)) > 240
            if angle == 0:
                width = 1 if selected else 2
                assert min(edge_pixel(-width - 0.5)) > 240
                for distance in [-i - 0.5 for i in range(width)]:
                    red, green, blue = edge_pixel(distance)
                    assert red < 130 and green > 100 and blue > 220
            red, green, blue = edge_pixel(-0.5 if selected else -1)
            assert blue > red + 60 and blue > green
            assert max(edge_pixel(1.5)) < 30
            assert min(edge_pixel(5)) > 240
        snapshot.assert_unchanged()
