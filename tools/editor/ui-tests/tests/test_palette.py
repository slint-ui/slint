# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    wait_until,
    window_element_with_label,
)

GOLDENS = Path(__file__).resolve().parents[1] / "goldens"
PALETTE_KINDS = ("Rectangle", "Text", "Image")


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
@pytest.mark.skip(reason="Requires a Rust palette drop-with-geometry fix")
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
        row = window_element_with_label(
            window, kind, slint_testing.AccessibleRole.ListItem
        )
        window.drag_and_drop(center(row), target)
        expected = (GOLDENS / f"Palette.insert-{kind.lower()}.slint").read_bytes()
        snapshot.wait_for_exact(expected, "Palette.slint")
        window_element_with_label(
            window, f"Selected {kind}", slint_testing.AccessibleRole.Region
        )
        inserted = wait_until(
            lambda: next(
                (
                    row
                    for row in window.root_element.query_descendants()
                    .match_accessible_role(slint_testing.AccessibleRole.ListItem)
                    .find_all()
                    if row.accessible_label == kind
                ),
                None,
            )
        )
        assert inserted.accessible_item_selected


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
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Escape))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Escape))
        release_palette_drag(window, target)
        assert not elements_with_label(window.root_element, "Canvas drop marker")
        snapshot.assert_unchanged()
