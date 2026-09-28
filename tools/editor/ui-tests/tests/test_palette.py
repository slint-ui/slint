# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import begin_palette_drag, center, zoom_canvas
from editor_sync import wait_for_source
from slint_test import Window
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    PALETTE_KINDS,
    first_window,
    launch_editor,
    select_fixture_element,
    wait_until,
)

GOLDENS = Path(__file__).resolve().parents[1] / "goldens"
TOUCHAREA_PREVIEW_SIZE = (160, 96)


def release_palette_drag(window: Window, target: slint_testing.LogicalPosition) -> None:
    window.pointer.move_to(target)
    window.pointer.release_at(target)


def canvas_drop_position(
    window: Window, scale: float = 1
) -> slint_testing.LogicalPosition:
    artboard = window.get_by_role("region", name="Artboard").resolve()
    return slint_testing.LogicalPosition(
        x=artboard.absolute_position.x + 195 * scale,
        y=artboard.absolute_position.y + 360 * scale,
    )


@pytest.mark.parametrize("percent", [50, 100, 200])
@pytest.mark.parametrize("kind", PALETTE_KINDS)
def test_insert_palette_element_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    percent: int,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        zoom_canvas(window, percent)
        target = canvas_drop_position(window, percent / 100)
        snapshot.assert_unchanged_now()
        begin_palette_drag(window, kind, target)
        release_palette_drag(window, target)
        expected = (GOLDENS / f"Palette.insert-{kind.lower()}.slint").read_bytes()
        snapshot.wait_for_exact(expected, "Palette.slint")
        window.get_by_role("region", name=f"Selected {kind}").wait_for()
        outline = window.get_by_role("list", name="Current file outline").resolve()
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
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        outside = center(
            window.get_by_role("navigation", name="Project and elements").resolve()
        )
        begin_palette_drag(window, kind, outside)
        snapshot.assert_unchanged_now()
        release_palette_drag(window, outside)
        snapshot.assert_unchanged()


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
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        target = canvas_drop_position(window)
        begin_palette_drag(window, kind, target)
        window.get_by_role("region", name=f"{kind} drag preview").wait_for()
        window.keyboard.press(keys.Escape)
        release_palette_drag(window, target)
        window.get_by_accessible_name(f"{kind} drag preview").wait_for(state="hidden")
        snapshot.assert_unchanged()


def library_rows(window: Window) -> list[slint_testing.Element]:
    pane = window.get_by_role("navigation", name="Project and elements").resolve()
    rows = (
        pane.query_descendants()
        .match_accessible_role(slint_testing.AccessibleRole.ListItem)
        .find_all()
    )
    return sorted(
        [row for row in rows if row.accessible_label in PALETTE_KINDS],
        key=lambda row: (row.absolute_position.y, row.absolute_position.x),
    )


def expect_library(window: Window, labels: list[str]) -> None:
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
    source_file = fixture_project / "Palette.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        expect_library(window, list(PALETTE_KINDS))
        search = window.get_by_accessible_name("Search elements")
        for query, expected in [
            ("  aG  ", ["Image"]),
            ("T", ["Rectangle", "Text", "TouchArea"]),
            ("  tOuCh  ", ["TouchArea"]),
            ("AREA", ["TouchArea"]),
            ("missing", []),
        ]:
            search.set_accessible_value(query)
            expect_library(window, expected)
            for label in ("Visual", "Input & interaction"):
                for header in window.get_by_role("button", name=label).all():
                    assert not header.accessible_enabled
        window.get_by_accessible_name("No Results").wait_for()
        for label in ("Visual", "Input & interaction"):
            window.get_by_role("button", name=label).wait_for(state="hidden")
        search.set_accessible_value("")
        expect_library(window, list(PALETTE_KINDS))
        snapshot.assert_unchanged()


def test_library_search_restores_independent_collapse_states(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Palette.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        search = window.get_by_accessible_name("Search elements")
        for label, collapsed_labels in [
            ("Visual", ["TouchArea"]),
            ("Input & interaction", []),
        ]:
            window.get_by_role("button", name=label).activate()
            expect_library(window, collapsed_labels)
            search.set_accessible_value("t")
            expect_library(window, ["Rectangle", "Text", "TouchArea"])
            search.set_accessible_value("missing")
            expect_library(window, [])
            search.set_accessible_value("")
            expect_library(window, collapsed_labels)
        window.get_by_role("button", name="Visual").activate()
        expect_library(window, ["Image", "Rectangle", "Text"])
        search.set_accessible_value("touch")
        expect_library(window, ["TouchArea"])
        window.get_by_role("button", name="Visual").wait_for(state="hidden")
        search.set_accessible_value("")
        expect_library(window, ["Image", "Rectangle", "Text"])
        snapshot.assert_unchanged()


def test_library_layout_stays_anchored_during_search_and_collapse(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Palette.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        expect_library(window, list(PALETTE_KINDS))
        heading = window.get_by_accessible_name("ELEMENTS").resolve()
        search = window.get_by_accessible_name("Search elements")
        library = window.get_by_role("list", name="Element library").resolve()
        pane = window.get_by_role("navigation", name="Project and elements").resolve()
        assert library.absolute_position.y + library.size.height == pytest.approx(
            pane.absolute_position.y + pane.size.height, abs=1
        )
        heading_y = heading.absolute_position.y
        search_y = search.bounds().y
        rows = library_rows(window)
        assert rows[0].absolute_position.y == rows[1].absolute_position.y
        assert rows[0].absolute_position.x < rows[1].absolute_position.x
        assert rows[2].absolute_position.y > rows[0].absolute_position.y
        assert rows[3].absolute_position.y > rows[2].absolute_position.y
        for label, remaining in [
            ("Visual", ["TouchArea"]),
            ("Input & interaction", []),
        ]:
            window.get_by_role("button", name=label).activate()
            expect_library(window, remaining)
            assert heading.absolute_position.y == heading_y
            assert search.bounds().y == search_y
        for query, expected in [("image", ["Image"]), ("missing", []), ("", [])]:
            search.set_accessible_value(query)
            expect_library(window, expected)
            assert heading.absolute_position.y == heading_y
            assert search.bounds().y == search_y


def test_library_search_keyboard_does_not_delete_selection(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        search = window.get_by_accessible_name("Search elements")
        search.click()
        window.keyboard.press_sequentially("Text")
        expect_library(window, ["Text"])
        for _ in range(5):
            window.keyboard.press(keys.Backspace)
        expect_library(window, ["Image", "Rectangle", "Text", "TouchArea"])
        window.get_by_role("region", name="Selected Rectangle").wait_for()
        snapshot.assert_unchanged()


def test_toucharea_drag_preview(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Palette.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        target = canvas_drop_position(window)
        begin_palette_drag(window, "TouchArea", target)
        preview = window.get_by_role("region", name="TouchArea drag preview").resolve()
        expected_width, expected_height = TOUCHAREA_PREVIEW_SIZE
        assert preview.size.width == expected_width
        assert preview.size.height == expected_height
        assert preview.absolute_position.x == target.x - expected_width / 2
        assert preview.absolute_position.y == target.y - expected_height / 2
        snapshot.assert_unchanged_now()
        outside = slint_testing.LogicalPosition(x=10, y=10)
        release_palette_drag(window, outside)
        snapshot.assert_unchanged()


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
    source_file = fixture_project / "Palette.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        search = window.get_by_accessible_name("Search elements")
        search.click()
        for _ in range(tab_count):
            window.keyboard.press(keys.Tab)
        window.keyboard.press(activation_key)
        expect_library(window, remaining)
        window.get_by_role("button", name=group_label).wait_for()
        window.keyboard.press(activation_key)
        expect_library(window, list(PALETTE_KINDS))
        snapshot.assert_unchanged()
