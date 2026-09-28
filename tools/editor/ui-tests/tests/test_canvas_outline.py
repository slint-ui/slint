# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import wait_for_source
from source_snapshot import SourceSnapshot
from ui_driver import (
    wait_until,
)


@pytest.mark.parametrize(
    "tool", ["resize top-left", "rotate top-left", "radius top-left", "move handle"]
)
def test_selected_hover_hides_for_manipulation(
    editor_factory,
    fixture_project: Path,
    tool: str,
) -> None:
    with editor_factory(fixture_project / "Main.slint") as editor:
        window = editor.window
        editor.outline.select("root-rectangle")
        frame = window.get_by_accessible_name("Selected Rectangle").resolve()
        inside = slint_testing.LogicalPosition(
            x=frame.absolute_position.x + 40,
            y=frame.absolute_position.y + 60,
        )
        window.pointer.move_to(inside)
        window.get_by_accessible_name("Hovered Rectangle").wait_for()

        handle = window.get_by_accessible_name("Rectangle " + tool).resolve()
        target = inside if tool == "move handle" else center(handle)
        window.pointer.move_to(target)
        if tool == "move handle":
            window.pointer.press_at(target)

        def hover_outline_hidden() -> bool | None:
            if window.get_by_accessible_name("Hovered Rectangle").all():
                return None
            return True

        # Controls suppress hover before a drag starts.
        wait_until(hover_outline_hidden)
        if tool == "move handle":
            window.pointer.release_at(target)
        window.pointer.move_to(inside)
        window.get_by_accessible_name("Hovered Rectangle").wait_for()


def test_click_selection_keeps_visible_hover_outline(
    editor_factory,
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    with editor_factory(source) as editor:
        wait_for_source(source, source.read_bytes())
        window = editor.window
        artboard = window.get_by_accessible_name("Artboard").resolve()
        target = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + 80,
            y=artboard.absolute_position.y + 100,
        )
        window.pointer.move_to(target)
        window.get_by_accessible_name("Hovered Rectangle").wait_for()
        window.pointer.press_at(target)
        window.pointer.release_at(target)
        window.get_by_accessible_name("Selected Rectangle").wait_for()
        window.get_by_accessible_name("Hovered Rectangle").wait_for()


def test_click_outside_artboard_clears_selection(
    editor_factory,
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    with editor_factory(source) as editor:
        wait_for_source(source, source.read_bytes())
        window = editor.window
        editor.outline.select("root-rectangle")
        window.get_by_accessible_name("Selected Rectangle").wait_for()
        window.get_by_accessible_name("Rectangle background").wait_for()
        canvas = window.get_by_accessible_name("Editor canvas").resolve()
        target = slint_testing.LogicalPosition(
            x=canvas.absolute_position.x + 10,
            y=canvas.absolute_position.y + 10,
        )
        window.pointer.move_to(target)
        window.pointer.press_at(target)
        window.pointer.release_at(target)

        def selection_cleared() -> bool | None:
            tree = window.get_by_accessible_name("Current file outline").resolve()
            rows = (
                tree.query_descendants()
                .match_accessible_role(slint_testing.AccessibleRole.ListItem)
                .find_all()
            )
            return (
                True
                if rows
                and not any(row.accessible_item_selected for row in rows)
                and not window.get_by_accessible_name("Selected Rectangle").all()
                and not window.get_by_accessible_name("Rectangle background").all()
                and not window.get_by_accessible_name("Root background").all()
                else None
            )

        wait_until(selection_cleared)
        editor.outline.select("root-rectangle")
        window.get_by_accessible_name("Selected Rectangle").wait_for()
        window.get_by_role("list", name="Current file outline").get_by_role(
            "list-item"
        ).nth(0).activate()
        window.get_by_accessible_name("Root background").wait_for()


def test_resize_handle_touch_area_is_centered(
    editor_factory,
    fixture_project: Path,
) -> None:
    with editor_factory(fixture_project / "Main.slint") as editor:
        window = editor.window
        editor.outline.select("root-rectangle")
        frame = window.get_by_accessible_name("Selected Rectangle").resolve()
        handle = window.get_by_accessible_name("Rectangle resize top-left").resolve()
        assert handle.size.width == pytest.approx(12)
        assert handle.size.height == pytest.approx(12)
        assert handle.absolute_position.x + 6 == pytest.approx(
            frame.absolute_position.x
        )
        assert handle.absolute_position.y + 6 == pytest.approx(
            frame.absolute_position.y
        )


def test_resize_starts_outside_visible_handle(
    editor_factory,
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source) as editor:
        window = editor.window
        editor.outline.select("root-rectangle")
        frame = window.get_by_accessible_name("Selected Rectangle").resolve()
        initial_width, initial_height = frame.size.width, frame.size.height
        handle = window.get_by_accessible_name(
            "Rectangle resize bottom-right"
        ).resolve()
        position = center(handle)
        # Five pixels from the corner is outside the visible four-pixel half-width.
        start = slint_testing.LogicalPosition(x=position.x + 5, y=position.y + 5)
        end = slint_testing.LogicalPosition(x=start.x + 20, y=start.y + 16)
        window.pointer.move_to(start)
        window.pointer.press_at(start)
        window.pointer.move_to(end)
        assert frame.size.width == pytest.approx(initial_width + 20)
        assert frame.size.height == pytest.approx(initial_height + 16)
        snapshot.assert_unchanged_now()
        window.pointer.release_at(end)
