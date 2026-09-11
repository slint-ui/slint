# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore getpixel putpixel

import math
from io import BytesIO
from pathlib import Path
from unittest.mock import Mock

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import wait_for_source
from PIL import Image
from source_snapshot import SourceSnapshot
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    select_outline_row,
    wait_until,
    window_element_with_label,
)


def window_pixels(window: slint_testing.Window, png: bytes | None = None):
    rendered = Image.open(BytesIO(png or window.grab_window_as_png())).convert("RGB")
    scale = rendered.width / window.root_element.size.width

    def pixel(x: float, y: float):
        return rendered.getpixel((int(x * scale), int(y * scale)))

    return pixel


@pytest.mark.parametrize("scale", [1, 2])
def test_window_pixels_uses_logical_coordinates(scale: int) -> None:
    rendered = Image.new("RGB", (100 * scale, 80 * scale), "white")
    rendered.putpixel((30 * scale, 20 * scale), (11, 153, 254))
    png = BytesIO()
    rendered.save(png, format="PNG")
    window = Mock(
        spec=slint_testing.Window,
        size=Mock(width=100 * scale, height=80 * scale),
        root_element=Mock(size=Mock(width=100, height=80)),
    )
    # Screenshot pixels are physical; element coordinates are logical.
    assert window_pixels(window, png.getvalue())(30, 20) == (11, 153, 254)


@pytest.mark.parametrize("angle", [0, 30])
@pytest.mark.parametrize("radius", [0, 20])
@pytest.mark.parametrize(
    "selected, hovered", [(False, True), (True, False), (True, True)]
)
def test_canvas_outline_preserves_item_border(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tmp_path: Path,
    angle: int,
    selected: bool,
    hovered: bool,
    radius: int,
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
        border-radius: {radius}px;
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
                    x=artboard.absolute_position.x + (150 if hovered else -20),
                    y=artboard.absolute_position.y + (150 if hovered else -20),
                )
            )
        )
        label = "Selected Rectangle" if selected else "Hovered Rectangle"
        frame = window_element_with_label(window, label)
        assert frame.size.width == pytest.approx(100)
        assert frame.size.height == pytest.approx(100)
        if selected and angle == 0:
            handle = window_element_with_label(window, "Rectangle resize top-left")
            assert handle.size.width == pytest.approx(12)
            assert handle.size.height == pytest.approx(12)
            assert handle.absolute_position.x + 6 == pytest.approx(
                frame.absolute_position.x
            )
            assert handle.absolute_position.y + 6 == pytest.approx(
                frame.absolute_position.y
            )
        if hovered:
            window_element_with_label(window, "Hovered Rectangle")
        else:
            assert not elements_with_label(window.root_element, "Hovered Rectangle")
        png = window.grab_window_as_png()
        (tmp_path / "outline.png").write_bytes(png)
        sample = window_pixels(window, png)
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
            color = sample(absolute_x, absolute_y)
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

            # The blue band stays outside the black item border.
            assert min(edge_pixel(-4)) > 240
            if angle == 0:
                width = 2 if hovered else 1
                assert min(edge_pixel(-width - 0.5)) > 240
                for distance in [-i - 0.5 for i in range(width)]:
                    # Straight edges have the exact light-theme color.
                    assert edge_pixel(distance) == (11, 153, 254)
            red, green, blue = edge_pixel(-1 if hovered else -0.5)
            assert blue > red + 60 and blue > green
            assert max(edge_pixel(1.5)) < 30
            assert min(edge_pixel(5)) > 240
        if hovered and radius and not selected:
            assert min(pixel(-1, -1)) > 240
            arc = radius - (radius + 1) / math.sqrt(2)
            red, green, blue = pixel(arc, arc)
            assert blue > red + 60 and blue > green
        if selected and hovered and angle == 0:
            # The hover line must not cross the white resize handle.
            assert min(pixel(-2, -2)) > 240
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "tool", ["resize top-left", "rotate top-left", "radius top-left", "move handle"]
)
def test_selected_hover_hides_for_manipulation(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tool: str,
) -> None:
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        frame = window_element_with_label(window, "Selected Rectangle")
        inside = slint_testing.LogicalPosition(
            x=frame.absolute_position.x + 40,
            y=frame.absolute_position.y + 60,
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(inside))
        window_element_with_label(window, "Hovered Rectangle")

        handle = window_element_with_label(window, "Rectangle " + tool)
        target = inside if tool == "move handle" else center(handle)
        window.dispatch_event(slint_testing.PointerMoveEvent(target))
        if tool == "move handle":
            window.dispatch_event(
                slint_testing.PointerPressEvent(
                    target, slint_testing.PointerEventButton.Left
                )
            )
        # Controls suppress hover before a drag starts.
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "Hovered Rectangle")
                else None
            )
        )
        if tool == "move handle":
            window.dispatch_event(
                slint_testing.PointerReleaseEvent(
                    target, slint_testing.PointerEventButton.Left
                )
            )
        window.dispatch_event(slint_testing.PointerMoveEvent(inside))
        window_element_with_label(window, "Hovered Rectangle")


def test_click_selection_keeps_visible_hover_outline(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tmp_path: Path,
) -> None:
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        artboard = window_element_with_label(window, "Artboard")
        target = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + 80,
            y=artboard.absolute_position.y + 100,
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(target))
        window_element_with_label(window, "Hovered Rectangle")
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        frame = window_element_with_label(window, "Selected Rectangle")
        window_element_with_label(window, "Hovered Rectangle")
        png = window.grab_window_as_png()
        (tmp_path / "click-hover.png").write_bytes(png)
        # The second blue pixel belongs to hover, not the one-pixel selection.
        assert window_pixels(window, png)(
            frame.absolute_position.x - 1.5, frame.absolute_position.y + 60
        ) == (11, 153, 254)


def test_manipulation_indicators_have_white_fills(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        frame = window_element_with_label(window, "Selected Rectangle")
        window.dispatch_event(
            slint_testing.PointerMoveEvent(
                slint_testing.LogicalPosition(
                    x=frame.absolute_position.x + 40,
                    y=frame.absolute_position.y + 60,
                )
            )
        )
        radius = window_element_with_label(window, "Rectangle radius top-left")
        pixel = window_pixels(window)
        position = center(radius)
        # A transparent center would reveal the blue rectangle.
        assert pixel(position.x, position.y) == (255, 255, 255)
        for corner in ["top-left", "top-right", "bottom-right", "bottom-left"]:
            position = center(
                window_element_with_label(window, "Rectangle resize " + corner)
            )
            # Every pixel of the square 8x8 handle has a one-pixel rim.
            for y in range(8):
                for x in range(8):
                    expected = (
                        (82, 174, 255)
                        if x in (0, 7) or y in (0, 7)
                        else (255, 255, 255)
                    )
                    assert (
                        pixel(position.x - 4 + x + 0.5, position.y - 4 + y + 0.5)
                        == expected
                    )


def test_resize_starts_outside_visible_handle(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        frame = window_element_with_label(window, "Selected Rectangle")
        initial_width, initial_height = frame.size.width, frame.size.height
        handle = window_element_with_label(window, "Rectangle resize bottom-right")
        position = center(handle)
        # Five pixels from the corner is outside the visible four-pixel half-width.
        start = slint_testing.LogicalPosition(x=position.x + 5, y=position.y + 5)
        end = slint_testing.LogicalPosition(x=start.x + 20, y=start.y + 16)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        assert frame.size.width == pytest.approx(initial_width + 20)
        assert frame.size.height == pytest.approx(initial_height + 16)
        snapshot.assert_unchanged_now()
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))


def test_hover_follows_independent_corner_radii(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    radii = [
        ("top-left", 8),
        ("top-right", 16),
        ("bottom-right", 24),
        ("bottom-left", 32),
    ]
    source.write_text(
        source.read_text().replace(
            "border-radius: 12px;",
            "\n".join(
                f"border-{corner}-radius: {radius}px;" for corner, radius in radii
            ),
            1,
        )
    )
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, source.read_bytes())
        artboard = window_element_with_label(window, "Artboard")
        origin = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + 40, y=artboard.absolute_position.y + 40
        )
        window.dispatch_event(
            slint_testing.PointerMoveEvent(
                slint_testing.LogicalPosition(x=origin.x + 40, y=origin.y + 60)
            )
        )
        window_element_with_label(window, "Hovered Rectangle")
        pixel = window_pixels(window)
        # Sample each curved edge where a bounding-box outline would be absent.
        for corner, radius in radii:
            arc = radius - (radius + 1) / math.sqrt(2)
            x = arc if "left" in corner else 180 - arc
            y = arc if "top" in corner else 120 - arc
            red, green, blue = pixel(origin.x + x, origin.y + y)
            assert blue > red + 100 and green > 110
