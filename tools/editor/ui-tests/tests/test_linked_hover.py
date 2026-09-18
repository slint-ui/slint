# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from io import BytesIO

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import wait_for_source
from PIL import Image
from test_outline import outline_row, outline_rows
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    wait_until,
    window_element_with_label,
)


def row_background(window, label):
    row = outline_row(window, label)
    image = Image.open(BytesIO(window.grab_window_as_png()))
    scale = image.width / window.root_element.size.width
    return image.getpixel(
        (
            round((row.absolute_position.x + row.size.width - 10) * scale),
            round((row.absolute_position.y + row.size.height / 2) * scale),
        )
    )


@pytest.mark.parametrize(
    "origin", ["canvas", "tree", "canvas-to-tree", "tree-to-canvas"]
)
def test_hover_links_canvas_and_outline(
    editor_binary, editor_environment, fixture_project, origin
):
    source = fixture_project / "Main.slint"
    original = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, original)
        window = first_window(editor)
        artboard = window_element_with_label(window, "Artboard")
        away = slint_testing.LogicalPosition(x=1, y=1)
        window.dispatch_event(slint_testing.PointerMoveEvent(away))
        labels = ["root-rectangle", "root-text"]
        backgrounds = {label: row_background(window, label) for label in labels}
        selected = [row.accessible_item_selected for row in outline_rows(window)]
        for label, kind, x, y in [
            ("root-rectangle", "Rectangle", 40, 40),
            ("root-text", "Text", 180, 56),
        ]:
            target = (
                center(outline_row(window, label))
                if origin == "tree"
                or (origin == "canvas-to-tree" and kind == "Text")
                or (origin == "tree-to-canvas" and kind == "Rectangle")
                else slint_testing.LogicalPosition(
                    x=artboard.absolute_position.x + x + 30,
                    y=artboard.absolute_position.y + y + 30,
                )
            )
            window.dispatch_event(slint_testing.PointerMoveEvent(target))
            frame = window_element_with_label(window, "Hovered " + kind)
            assert frame.absolute_position.x == pytest.approx(
                artboard.absolute_position.x + x
            )
            assert frame.size.width == pytest.approx(180)
            wait_until(
                lambda label=label: (
                    row_background(window, label) != backgrounds[label] or None
                )
            )
            other = next(value for value in labels if value != label)
            wait_until(
                lambda other=other: (
                    row_background(window, other) == backgrounds[other] or None
                )
            )
            Image.open(BytesIO(window.grab_window_as_png())).save(
                fixture_project.parent
                / ("linked-hover-" + origin + "-" + kind + ".png")
            )
        window.dispatch_event(slint_testing.PointerMoveEvent(away))
        wait_until(
            lambda: not elements_with_label(window.root_element, "Hovered Text") or None
        )
        for label in labels:
            assert row_background(window, label) == backgrounds[label]
        assert [
            row.accessible_item_selected for row in outline_rows(window)
        ] == selected
        assert source.read_bytes() == original


def test_tree_hover_geometry_updates_after_reload(
    editor_binary, editor_environment, fixture_project
):
    source = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        artboard = window_element_with_label(window, "Artboard")
        window.dispatch_event(
            slint_testing.PointerMoveEvent(
                center(outline_row(window, "root-rectangle"))
            )
        )
        window_element_with_label(window, "Hovered Rectangle")
        updated = source.read_bytes().replace(b"x: 40px;", b"x: 70px;", 1)
        source.write_bytes(updated)
        wait_for_source(source, updated)

        def moved():
            frame = window_element_with_label(window, "Hovered Rectangle")
            return (
                frame.absolute_position.x
                == pytest.approx(artboard.absolute_position.x + 70)
                or None
            )

        wait_until(moved)
