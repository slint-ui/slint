# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import slint_testing
from canvas_interactions import (
    manual_rotation_drag,
    position_distance,
    radius_handle,
    selection_frame,
)
from source_snapshot import SourceSnapshot, replace_once, wait_for_source_change
from test_canvas import (
    radius_handle_positions,
    wait_for_radius_tooltip,
)


def test_rotation_crosses_zero_with_exact_source(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    original = b'        text: "Fixture text";'
    crossing_baseline = replace_once(
        baseline, original, original + b"\n        transform-rotation: 350deg;"
    )
    source_file.write_bytes(crossing_baseline)
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Text")
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_rotation_drag(
            window,
            window.get_by_accessible_name("Text rotate top-left").resolve(),
            20,
            -20,
            snapshot,
            crosses_zero=True,
        )
        snapshot.wait_for_exact(
            crossing_baseline.replace(
                b"        transform-rotation: 350deg;",
                b"        transform-rotation: 0deg;",
                1,
            ),
        )


def test_zero_radius_handles_stay_inside_and_drag_back_to_zero(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    zero_source = replace_once(
        source_file.read_bytes(),
        b"        border-radius: 12px;",
        b"        border-radius: 0px;",
    )
    source_file.write_bytes(zero_source)
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        radius_handle(window, "top-left")
        x, y, width, height = selection_frame(window, "Rectangle")
        initial_positions = radius_handle_positions(window)
        for corner, position in initial_positions.items():
            expected_x = x + 12 if "left" in corner else x + width - 12
            expected_y = y + 12 if "top" in corner else y + height - 12
            assert abs(position.x - expected_x) < 1.5
            assert abs(position.y - expected_y) < 1.5

        start = initial_positions["top-left"]
        corner_position = slint_testing.LogicalPosition(x=x, y=y)
        window.pointer.press_at(start)
        window.pointer.move_to(corner_position)
        wait_for_radius_tooltip(window, 0)
        for corner, position in radius_handle_positions(window).items():
            expected_x = x if "left" in corner else x + width
            expected_y = y if "top" in corner else y + height
            assert (
                position_distance(
                    position,
                    slint_testing.LogicalPosition(x=expected_x, y=expected_y),
                )
                < 1.5
            )
        window.pointer.release_at(corner_position)
        wait_for_source_change(source_file, zero_source)
        radius_handle(window, "top-left")
        for corner, position in radius_handle_positions(window).items():
            assert position_distance(position, initial_positions[corner]) < 1.5
