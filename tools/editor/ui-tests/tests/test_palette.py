# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    PALETTE_KINDS,
    elements_with_label,
    first_window,
    launch_editor,
    press_key,
    press_keys,
    select_fixture_element,
    wait_until,
    window_element_with_label,
)

GOLDENS = Path(__file__).resolve().parents[1] / "goldens"


def begin_palette_drag(
    window: slint_testing.Window,
    kind: str,
    target: slint_testing.LogicalPosition,
) -> None:
    row = wait_until(
        lambda: (
            candidate
            if (
                candidate := window_element_with_label(
                    window, kind, slint_testing.AccessibleRole.ListItem
                )
            ).accessible_enabled
            else None
        )
    )
    start = center(row)
    button = slint_testing.PointerEventButton.Left
    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    window.dispatch_event(
        slint_testing.PointerMoveEvent(
            slint_testing.LogicalPosition(x=start.x + 16, y=start.y + 16)
        )
    )
    window.dispatch_event(slint_testing.PointerMoveEvent(target))


def release_palette_drag(
    window: slint_testing.Window, target: slint_testing.LogicalPosition
) -> None:
    window.dispatch_event(slint_testing.PointerMoveEvent(target))
    window.dispatch_event(
        slint_testing.PointerReleaseEvent(target, slint_testing.PointerEventButton.Left)
    )


def canvas_drop_position(window: slint_testing.Window) -> slint_testing.LogicalPosition:
    artboard = window_element_with_label(
        window, "Artboard", slint_testing.AccessibleRole.Region
    )
    return slint_testing.LogicalPosition(
        x=artboard.absolute_position.x + 195,
        y=artboard.absolute_position.y + 360,
    )


@pytest.mark.parametrize("kind", PALETTE_KINDS)
def test_insert_palette_element_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        target = canvas_drop_position(window)
        snapshot.assert_unchanged_now()
        begin_palette_drag(window, kind, target)
        release_palette_drag(window, target)
        expected = (GOLDENS / f"Palette.insert-{kind.lower()}.slint").read_bytes()
        snapshot.wait_for_exact(expected, "Palette.slint")
        window_element_with_label(
            window, f"Selected {kind}", slint_testing.AccessibleRole.Region
        )
        outline = window_element_with_label(
            window, "Current file outline", slint_testing.AccessibleRole.List
        )
        inserted = wait_until(
            lambda: next(
                (
                    row
                    for row in outline.query_descendants()
                    .match_accessible_role(slint_testing.AccessibleRole.ListItem)
                    .find_all()
                    if row.accessible_label.strip() == kind
                ),
                None,
            )
        )
        wait_until(lambda: True if inserted.accessible_item_selected else None)


@pytest.mark.parametrize("kind", PALETTE_KINDS)
def test_palette_drop_outside_canvas_does_not_edit_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        outside = center(
            window_element_with_label(
                window,
                "Project and elements",
                slint_testing.AccessibleRole.Navigation,
            )
        )
        begin_palette_drag(window, kind, outside)
        snapshot.assert_unchanged_now()
        release_palette_drag(window, outside)
        snapshot.assert_unchanged()


@pytest.mark.parametrize("kind", PALETTE_KINDS)
@pytest.mark.skip(reason="Requires Rust palette drop-marker support")
def test_escape_cancels_palette_drag_without_source_edit(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        target = canvas_drop_position(window)
        begin_palette_drag(window, kind, target)
        window_element_with_label(
            window, "Canvas drop marker", slint_testing.AccessibleRole.Region
        )
        press_key(window, keys.Escape)
        release_palette_drag(window, target)
        assert not elements_with_label(window.root_element, "Canvas drop marker")
        snapshot.assert_unchanged()


def library_rows(window: slint_testing.Window) -> list[slint_testing.Element]:
    pane = window_element_with_label(
        window, "Project and elements", slint_testing.AccessibleRole.Navigation
    )
    rows = (
        pane.query_descendants()
        .match_accessible_role(slint_testing.AccessibleRole.ListItem)
        .find_all()
    )
    return sorted(
        [row for row in rows if row.accessible_label in PALETTE_KINDS],
        key=lambda row: (row.absolute_position.y, row.absolute_position.x),
    )


def expect_library(window: slint_testing.Window, labels: list[str]) -> None:
    wait_until(
        lambda: (
            True
            if [row.accessible_label for row in library_rows(window)] == labels
            else None
        )
    )


def test_library_search_and_collapse(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Palette.slint"
    ) as editor:
        window = first_window(editor)
        expect_library(window, ["Image", "Rectangle", "Text"])
        heading = window_element_with_label(window, "Elements")
        search = window_element_with_label(window, "Search elements")
        gap = (
            search.absolute_position.y
            - heading.absolute_position.y
            - heading.size.height
        )
        assert 0 <= gap <= 20
        heading_y = heading.absolute_position.y
        search_y = search.absolute_position.y
        rows = library_rows(window)
        assert rows[0].absolute_position.y == rows[1].absolute_position.y
        assert rows[2].absolute_position.y > rows[0].absolute_position.y
        assert not elements_with_label(window.root_element, "Input & interaction")
        group = window_element_with_label(
            window, "Visual", slint_testing.AccessibleRole.Button
        )
        group.invoke_accessible_default_action()
        expect_library(window, [])
        assert heading.absolute_position.y == heading_y
        assert search.absolute_position.y == search_y
        search = window_element_with_label(window, "Search elements")
        for query, expected in [
            ("  aG  ", ["Image"]),
            ("T", ["Rectangle", "Text"]),
            ("TouchArea", []),
        ]:
            search.accessible_value = query
            expect_library(window, expected)
            for header in elements_with_label(
                window.root_element, "Visual", slint_testing.AccessibleRole.Button
            ):
                assert not header.accessible_enabled
            assert heading.absolute_position.y == heading_y
            assert search.absolute_position.y == search_y
        window_element_with_label(window, "No Results")
        assert not elements_with_label(
            window.root_element, "Visual", slint_testing.AccessibleRole.Button
        )
        search.accessible_value = ""
        expect_library(window, [])
        assert heading.absolute_position.y == heading_y
        assert search.absolute_position.y == search_y
        group = window_element_with_label(
            window, "Visual", slint_testing.AccessibleRole.Button
        )
        group.invoke_accessible_default_action()
        expect_library(window, ["Image", "Rectangle", "Text"])
        snapshot.assert_unchanged()


def test_library_search_keyboard_does_not_delete_selection(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        search = window_element_with_label(window, "Search elements")
        search.single_click(slint_testing.PointerEventButton.Left)
        press_keys(window, "Text")
        expect_library(window, ["Text"])
        for _ in range(5):
            press_key(window, keys.Backspace)
        expect_library(window, ["Image", "Rectangle", "Text"])
        snapshot.assert_unchanged()
