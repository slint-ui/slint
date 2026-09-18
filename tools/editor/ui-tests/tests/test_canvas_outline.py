# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import wait_for_source
from source_snapshot import SourceSnapshot
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    select_outline_row,
    wait_until,
    window_element_with_label,
)


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

        def hover_outline_hidden() -> bool | None:
            if elements_with_label(window.root_element, "Hovered Rectangle"):
                return None
            return True

        # Controls suppress hover before a drag starts.
        wait_until(hover_outline_hidden)
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
) -> None:
    source = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
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
        window_element_with_label(window, "Selected Rectangle")
        window_element_with_label(window, "Hovered Rectangle")


def test_resize_handle_touch_area_is_centered(
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
        handle = window_element_with_label(window, "Rectangle resize top-left")
        assert handle.size.width == pytest.approx(12)
        assert handle.size.height == pytest.approx(12)
        assert handle.absolute_position.x + 6 == pytest.approx(
            frame.absolute_position.x
        )
        assert handle.absolute_position.y + 6 == pytest.approx(
            frame.absolute_position.y
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
