# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import (
    element_frame,
    fixture_element,
    hover_fixture_element,
    selection_frame,
)
from editor_sync import wait_for_source
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_assertions import expect
from ui_driver import (
    elements,
    first_window,
    launch_editor,
    press_key,
    press_shortcut,
    select_fixture_element,
    select_outline_row,
)

POSITIONS = {
    "Rectangle": (40, 40),
    "Text": (180, 56),
    "Image": (230, 132),
}


def moved_source(baseline: bytes, kind: str, dx: int, dy: int) -> bytes:
    x, y = POSITIONS[kind]
    return replace_once(
        baseline,
        f"        x: {x}px;\n        y: {y}px;".encode(),
        f"        x: {x + dx}px;\n        y: {y + dy}px;".encode(),
    )


@pytest.mark.parametrize("kind", POSITIONS)
@pytest.mark.parametrize(
    ("key", "dx", "dy"),
    [
        (keys.LeftArrow, -1, 0),
        (keys.RightArrow, 1, 0),
        (keys.UpArrow, 0, -1),
        (keys.DownArrow, 0, 1),
    ],
    ids=["left", "right", "up", "down"],
)
def test_arrow_key_moves_selected_element(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    key: str,
    dx: int,
    dy: int,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, baseline)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_fixture_element(window, kind)
        press_key(window, key)
        snapshot.wait_for_applied(moved_source(baseline, kind, dx, dy))


def test_shift_arrow_key_moves_selected_element_by_ten(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, baseline)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_fixture_element(window, "Rectangle")
        press_shortcut(window, keys.Shift, keys.RightArrow)
        snapshot.wait_for_applied(moved_source(baseline, "Rectangle", 10, 0))


def test_held_arrow_key_writes_source_once_on_release(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, baseline)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_fixture_element(window, "Rectangle")
        for _ in range(3):
            window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.DownArrow))
        snapshot.assert_unchanged()
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.DownArrow))
        snapshot.wait_for_applied(moved_source(baseline, "Rectangle", 0, 3))
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline)


def test_selection_frame_follows_held_arrow_key(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, source_file.read_bytes())
        select_fixture_element(window, "Rectangle")
        initial_frame = selection_frame(window, "Rectangle")
        initial_element = element_frame(fixture_element(window, "Rectangle"))
        for _ in range(5):
            window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.RightArrow))
        zoom = initial_frame[2] / 180
        expected = round(5 * zoom, 2)

        def moved_by() -> tuple[float, float]:
            frame = selection_frame(window, "Rectangle")
            preview = element_frame(fixture_element(window, "Rectangle"))
            return (
                round(frame[0] - initial_frame[0], 2),
                round(preview[0] - initial_element[0], 2),
            )

        expect.poll(moved_by, message="frame and element offsets").to_equal(
            (expected, expected)
        )
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.RightArrow))


def test_arrow_key_clears_stale_hover_frame(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, baseline)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_fixture_element(window, "Rectangle")
        press_key(window, keys.RightArrow)
        snapshot.wait_for_applied(moved_source(baseline, "Rectangle", 1, 0))
        hover_fixture_element(window, "Rectangle")
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.RightArrow))
        expect.poll(
            lambda: len(elements(window, "Hovered Rectangle")),
            message="hover frames",
        ).to_equal(0)
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.RightArrow))


def test_quick_arrow_key_taps_accumulate(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, baseline)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_fixture_element(window, "Rectangle")
        for _ in range(3):
            press_key(window, keys.RightArrow)
        snapshot.wait_for_applied(moved_source(baseline, "Rectangle", 3, 0))


def test_undo_cancels_unwritten_arrow_key_move(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, source_file.read_bytes())
        snapshot = SourceSnapshot.capture(fixture_project)
        select_fixture_element(window, "Rectangle")
        initial_element = element_frame(fixture_element(window, "Rectangle"))
        press_key(window, keys.RightArrow)
        press_shortcut(window, keys.Control, "z")
        snapshot.assert_unchanged(quiescence=1)
        expect.poll(
            lambda: element_frame(fixture_element(window, "Rectangle")),
            message="element frame",
        ).to_equal(initial_element)


def test_arrow_key_does_not_move_element_in_layout(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "CanvasCases.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, source_file.read_bytes())
        snapshot = SourceSnapshot.capture(fixture_project)
        select_outline_row(window, "layout-rectangle")
        press_key(window, keys.RightArrow)
        snapshot.assert_unchanged()
