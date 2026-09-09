# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import current_editor_sync
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    PALETTE_KINDS,
    elements_with_label,
    find_window_element_with_label,
    first_window,
    launch_editor,
    press_key,
    press_keys,
    select_fixture_element,
    wait_until,
    window_element_with_label,
)

GOLDENS = Path(__file__).resolve().parents[1] / "goldens"
TOUCHAREA_PREVIEW_SIZE = (160, 96)


def begin_palette_drag(
    window: slint_testing.Window,
    kind: str,
    target: slint_testing.LogicalPosition,
) -> None:
    row = wait_until(
        lambda: (
            candidate
            if (
                candidate := find_window_element_with_label(
                    window, kind, slint_testing.AccessibleRole.ListItem
                )
            )
            is not None
            and candidate.accessible_enabled
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
        wait_until(
            lambda: next(
                (
                    row
                    for row in outline.query_descendants()
                    .match_accessible_role(slint_testing.AccessibleRole.ListItem)
                    .find_all()
                    if row.accessible_label.strip() == kind
                    and row.accessible_item_selected
                ),
                None,
            )
        )


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
        with current_editor_sync.get().action() as input_action:
            begin_palette_drag(window, kind, outside)
            snapshot.assert_unchanged_now()
            release_palette_drag(window, outside)
        input_action.assert_no_source_writes()
        snapshot.assert_unchanged_now()


@pytest.mark.parametrize("kind", PALETTE_KINDS)
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
        with current_editor_sync.get().action() as input_action:
            begin_palette_drag(window, kind, target)
            window_element_with_label(
                window, f"{kind} drag preview", slint_testing.AccessibleRole.Region
            )
            press_key(window, keys.Escape)
            release_palette_drag(window, target)
        input_action.assert_no_source_writes()
        assert not elements_with_label(window.root_element, f"{kind} drag preview")
        snapshot.assert_unchanged_now()


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


def test_library_search_filters_elements(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Palette.slint"
    ) as editor:
        window = first_window(editor)
        expect_library(window, list(PALETTE_KINDS))
        search = window_element_with_label(window, "Search elements")
        with current_editor_sync.get().action() as input_action:
            for query, expected in [
                ("  aG  ", ["Image"]),
                ("T", ["Rectangle", "Text", "TouchArea"]),
                ("  tOuCh  ", ["TouchArea"]),
                ("AREA", ["TouchArea"]),
                ("missing", []),
            ]:
                search.accessible_value = query
                expect_library(window, expected)
                for label in ("Visual", "Input & interaction"):
                    for header in elements_with_label(
                        window.root_element, label, slint_testing.AccessibleRole.Button
                    ):
                        assert not header.accessible_enabled
            window_element_with_label(window, "No Results")
            for label in ("Visual", "Input & interaction"):
                assert not elements_with_label(
                    window.root_element, label, slint_testing.AccessibleRole.Button
                )
            search.accessible_value = ""
            expect_library(window, list(PALETTE_KINDS))
        input_action.assert_no_source_writes()
        snapshot.assert_unchanged_now()


def test_library_search_restores_independent_collapse_states(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Palette.slint"
    ) as editor:
        window = first_window(editor)
        search = window_element_with_label(window, "Search elements")
        with current_editor_sync.get().action() as input_action:
            for label, collapsed_labels in [
                ("Visual", ["TouchArea"]),
                ("Input & interaction", []),
            ]:
                window_element_with_label(
                    window, label, slint_testing.AccessibleRole.Button
                ).invoke_accessible_default_action()
                expect_library(window, collapsed_labels)
                search.accessible_value = "t"
                expect_library(window, ["Rectangle", "Text", "TouchArea"])
                search.accessible_value = "missing"
                expect_library(window, [])
                search.accessible_value = ""
                expect_library(window, collapsed_labels)
            window_element_with_label(
                window, "Visual", slint_testing.AccessibleRole.Button
            ).invoke_accessible_default_action()
            expect_library(window, ["Image", "Rectangle", "Text"])
            search.accessible_value = "touch"
            expect_library(window, ["TouchArea"])
            assert not elements_with_label(
                window.root_element, "Visual", slint_testing.AccessibleRole.Button
            )
            search.accessible_value = ""
        input_action.assert_no_source_writes()
        expect_library(window, ["Image", "Rectangle", "Text"])
        snapshot.assert_unchanged_now()


def test_library_layout_stays_anchored_during_search_and_collapse(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Palette.slint"
    ) as editor:
        window = first_window(editor)
        expect_library(window, list(PALETTE_KINDS))
        heading = window_element_with_label(window, "ELEMENTS")
        search = window_element_with_label(window, "Search elements")
        heading_y = heading.absolute_position.y
        search_y = search.absolute_position.y
        rows = library_rows(window)
        assert rows[0].absolute_position.y == rows[1].absolute_position.y
        assert rows[0].absolute_position.x < rows[1].absolute_position.x
        assert rows[2].absolute_position.y > rows[0].absolute_position.y
        assert rows[3].absolute_position.y > rows[2].absolute_position.y
        for label, remaining in [
            ("Visual", ["TouchArea"]),
            ("Input & interaction", []),
        ]:
            window_element_with_label(
                window, label, slint_testing.AccessibleRole.Button
            ).invoke_accessible_default_action()
            expect_library(window, remaining)
            assert heading.absolute_position.y == heading_y
            assert search.absolute_position.y == search_y
        for query, expected in [("image", ["Image"]), ("missing", []), ("", [])]:
            search.accessible_value = query
            expect_library(window, expected)
            assert heading.absolute_position.y == heading_y
            assert search.absolute_position.y == search_y


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
        with current_editor_sync.get().action() as input_action:
            select_fixture_element(window, "Rectangle")
            search = window_element_with_label(window, "Search elements")
            search.single_click(slint_testing.PointerEventButton.Left)
            press_keys(window, "Text")
            expect_library(window, ["Text"])
            for _ in range(5):
                press_key(window, keys.Backspace)
            expect_library(window, ["Image", "Rectangle", "Text", "TouchArea"])
        input_action.assert_no_source_writes()
        window_element_with_label(
            window, "Selected Rectangle", slint_testing.AccessibleRole.Region
        )
        snapshot.assert_unchanged_now()


def test_toucharea_drag_preview(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Palette.slint"
    ) as editor:
        window = first_window(editor)
        target = canvas_drop_position(window)
        with current_editor_sync.get().action() as input_action:
            begin_palette_drag(window, "TouchArea", target)
            preview = window_element_with_label(
                window, "TouchArea drag preview", slint_testing.AccessibleRole.Region
            )
            expected_width, expected_height = TOUCHAREA_PREVIEW_SIZE
            assert preview.size.width == expected_width
            assert preview.size.height == expected_height
            assert preview.absolute_position.x == target.x - expected_width / 2
            assert preview.absolute_position.y == target.y - expected_height / 2
            snapshot.assert_unchanged_now()
            outside = slint_testing.LogicalPosition(x=10, y=10)
            release_palette_drag(window, outside)
        input_action.assert_no_source_writes()
        snapshot.assert_unchanged_now()


@pytest.mark.parametrize(
    "activation_key", [keys.Return, keys.Space], ids=["enter", "space"]
)
@pytest.mark.parametrize(
    ("group_label", "tab_count", "remaining"),
    [
        ("Visual", 1, ["TouchArea"]),
        ("Input & interaction", 2, ["Image", "Rectangle", "Text"]),
    ],
)
def test_group_header_keyboard_activation(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    activation_key: str,
    group_label: str,
    tab_count: int,
    remaining: list[str],
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Palette.slint"
    ) as editor:
        window = first_window(editor)
        search = window_element_with_label(window, "Search elements")
        with current_editor_sync.get().action() as input_action:
            search.single_click(slint_testing.PointerEventButton.Left)
            for _ in range(tab_count):
                press_key(window, keys.Tab)
            press_key(window, activation_key)
            expect_library(window, remaining)
            window_element_with_label(
                window, group_label, slint_testing.AccessibleRole.Button
            )
            press_key(window, activation_key)
        input_action.assert_no_source_writes()
        expect_library(window, list(PALETTE_KINDS))
        snapshot.assert_unchanged_now()
