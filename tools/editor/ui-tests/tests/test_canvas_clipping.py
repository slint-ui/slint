# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore getcolors getpixel tobytes

import math
from io import BytesIO
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import wait_for_source
from PIL import Image
from test_palette import begin_palette_drag
from ui_driver import (
    first_window,
    launch_editor,
    select_outline_row,
    wait_until,
    window_element_with_label,
)


def screenshot(window: slint_testing.Window) -> Image.Image:
    previous = b""
    stable_frames = 0

    def settled() -> Image.Image | None:
        nonlocal previous, stable_frames
        image = Image.open(BytesIO(window.grab_window_as_png())).convert("RGB")
        data = image.tobytes()
        stable_frames = stable_frames + 1 if data == previous else 0
        previous = data
        return image if stable_frames >= 1 else None

    return wait_until(settled)


def protected_regions(window: slint_testing.Window, image: Image.Image):
    canvas = window_element_with_label(window, "Editor canvas")
    scale = image.width / window.root_element.size.width
    left = math.floor(canvas.absolute_position.x * scale)
    top = math.floor(canvas.absolute_position.y * scale)
    right = math.ceil((canvas.absolute_position.x + canvas.size.width) * scale)
    return {
        "title": (0, 0, image.width, top),
        "project panel": (0, top, left, image.height),
        # The inspector gutter contains no selection-dependent controls.
        "inspector panel": (right + 2, top, right + 12, image.height),
    }


@pytest.mark.parametrize("edge", ["left", "right", "top", "bottom"])
def test_selection_overlays_are_clipped_to_canvas(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    edge: str,
) -> None:
    source = fixture_project / "BoundsCases.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        canvas = window_element_with_label(window, "Editor canvas")
        artboard = window_element_with_label(window, "Artboard")
        select_outline_row(window, "bounds-rectangle")
        initial_frame = window_element_with_label(window, "Selected Rectangle")
        before = screenshot(window)
        scale = before.width / window.root_element.size.width
        border_x = round(
            (initial_frame.absolute_position.x + initial_frame.size.width / 2) * scale
        )
        border_y = math.floor(initial_frame.absolute_position.y * scale) - 1
        outline_color = before.getpixel((border_x, border_y))
        assert outline_color != before.getpixel((border_x, border_y - 2))
        x, y = artboard.absolute_position.x + 96, artboard.absolute_position.y + 80
        if edge == "left":
            x = canvas.absolute_position.x - 40
        elif edge == "right":
            x = canvas.absolute_position.x + canvas.size.width - 80
        elif edge == "top":
            y = canvas.absolute_position.y - 20
        else:
            y = canvas.absolute_position.y + canvas.size.height - 60
        expected = (
            source.read_text()
            .replace("x: 96px;", f"x: {x - artboard.absolute_position.x}px;")
            .replace("y: 80px;", f"y: {y - artboard.absolute_position.y}px;")
        )
        source.write_text(expected)
        wait_for_source(source, expected.encode())
        select_outline_row(window, "bounds-rectangle")
        frame = window_element_with_label(window, "Selected Rectangle")
        assert frame.absolute_position.x == pytest.approx(x)
        assert frame.absolute_position.y == pytest.approx(y)
        after = screenshot(window)
        for name, region in protected_regions(window, after).items():
            assert after.crop(region).tobytes() == before.crop(region).tobytes(), name
        scale = after.width / window.root_element.size.width
        region = (
            math.ceil(canvas.absolute_position.x * scale),
            math.ceil(canvas.absolute_position.y * scale),
            math.floor((canvas.absolute_position.x + canvas.size.width) * scale),
            after.height,
        )
        visible = after.crop(region)
        colors = visible.getcolors(visible.width * visible.height) or []
        assert sum(count for count, color in colors if color == outline_color) > 20


@pytest.mark.parametrize("edge", ["left", "top"])
def test_gradient_overlays_are_clipped_to_canvas(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    edge: str,
) -> None:
    source = fixture_project / "BoundsCases.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        canvas = window_element_with_label(window, "Editor canvas")
        artboard = window_element_with_label(window, "Artboard")
        x = (
            canvas.absolute_position.x - 40
            if edge == "left"
            else artboard.absolute_position.x + 96
        )
        y = (
            canvas.absolute_position.y - 60
            if edge == "top"
            else artboard.absolute_position.y + 80
        )
        expected = (
            source.read_text()
            .replace("x: 96px;", f"x: {x - artboard.absolute_position.x}px;")
            .replace("y: 80px;", f"y: {y - artboard.absolute_position.y}px;")
            .replace(
                "background: #2563eb;",
                "background: @linear-gradient(90deg, red, blue);",
            )
        )
        source.write_text(expected)
        wait_for_source(source, expected.encode())
        before = screenshot(window)
        select_outline_row(window, "bounds-rectangle")
        window_element_with_label(window, "Selected Rectangle")
        window_element_with_label(
            window, "Rectangle background color picker"
        ).invoke_accessible_default_action()
        window_element_with_label(window, "Gradient end")
        after = screenshot(window)
        for name, region in protected_regions(window, after).items():
            if name != "inspector panel":
                assert after.crop(region).tobytes() == before.crop(region).tobytes(), (
                    name
                )


@pytest.mark.parametrize("edge", ["left", "right", "top"])
def test_drag_previews_are_clipped_to_canvas(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    edge: str,
) -> None:
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Palette.slint"
    ) as editor:
        window = first_window(editor)
        canvas = window_element_with_label(window, "Editor canvas")
        target = center(canvas)
        begin_palette_drag(window, "Rectangle", target)
        window_element_with_label(window, "Rectangle drag preview")
        before = screenshot(window)
        if edge == "left":
            target = slint_testing.LogicalPosition(
                x=canvas.absolute_position.x + 10, y=target.y
            )
        elif edge == "right":
            target = slint_testing.LogicalPosition(
                x=canvas.absolute_position.x + canvas.size.width - 10, y=target.y
            )
        else:
            target = slint_testing.LogicalPosition(
                x=target.x, y=canvas.absolute_position.y + 10
            )
        window.dispatch_event(slint_testing.PointerMoveEvent(target))
        after = screenshot(window)
        for name, region in protected_regions(window, after).items():
            assert after.crop(region).tobytes() == before.crop(region).tobytes(), name
