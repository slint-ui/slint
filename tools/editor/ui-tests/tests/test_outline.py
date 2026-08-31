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


def outline_row(window: slint_testing.Window, label: str) -> slint_testing.Element:
    return window_element_with_label(
        window, label, slint_testing.AccessibleRole.ListItem
    )


def outline_rows(window: slint_testing.Window) -> list[slint_testing.Element]:
    tree = window_element_with_label(
        window, "Current file outline", slint_testing.AccessibleRole.List
    )
    return (
        tree.query_descendants()
        .match_accessible_role(slint_testing.AccessibleRole.ListItem)
        .find_all()
    )


def known_outline_state(
    window: slint_testing.Window,
) -> list[tuple[str, str, bool]]:
    labels = {"container", "child-a", "child-b", "sibling-a", "sibling-b"}
    return [
        (
            row.accessible_label,
            row.accessible_description,
            row.accessible_item_selected,
        )
        for row in outline_rows(window)
        if row.accessible_label in labels
    ]


def wait_for_outline_state(
    window: slint_testing.Window,
    expected: list[tuple[str, str, bool]],
) -> None:
    wait_until(
        lambda: (
            current if (current := known_outline_state(window)) == expected else None
        ),
        timeout=15,
    )
    assert outline_rows(window)[0].accessible_item_selected


def drop_position(
    window: slint_testing.Window, target: str, location: str
) -> slint_testing.LogicalPosition:
    if target == "<outline-root>":
        return center(
            window_element_with_label(
                window,
                "Outline root drop target",
                slint_testing.AccessibleRole.ListItem,
            )
        )
    row = (
        outline_rows(window)[0]
        if target == "<component-root>"
        else outline_row(window, target)
    )
    fraction = {"before": 1 / 6, "onto": 1 / 2, "after": 5 / 6}[location]
    return slint_testing.LogicalPosition(
        x=row.absolute_position.x + row.size.width / 2,
        y=row.absolute_position.y + row.size.height * fraction,
    )


def drag_row(
    window: slint_testing.Window,
    source: str,
    target: str,
    location: str,
) -> None:
    source_row = (
        outline_rows(window)[0] if source == "<root>" else outline_row(window, source)
    )
    window.drag_and_drop(center(source_row), drop_position(window, target, location))


@pytest.mark.parametrize(
    "source,target,location,golden",
    [
        (
            "sibling-b",
            "sibling-a",
            "before",
            "OutlineCases.reorder.sibling-b-before-a.slint",
        ),
        (
            "sibling-a",
            "sibling-b",
            "after",
            "OutlineCases.reorder.sibling-a-after-b.slint",
        ),
    ],
    ids=["before", "after"],
)
def test_outline_reorders_siblings_with_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    source: str,
    target: str,
    location: str,
    golden: str,
) -> None:
    source_file = fixture_project / "OutlineCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        drag_row(window, source, target, location)
        snapshot.wait_for_exact((GOLDENS / golden).read_bytes(), "OutlineCases.slint")
        wait_for_outline_state(
            window,
            [
                ("container", "Hierarchy level 2", False),
                ("child-a", "Hierarchy level 3", False),
                ("child-b", "Hierarchy level 3", False),
                ("sibling-b", "Hierarchy level 2", False),
                ("sibling-a", "Hierarchy level 2", False),
            ],
        )


@pytest.mark.parametrize(
    "source,target,golden",
    [
        (
            "sibling-a",
            "container",
            "OutlineCases.reparent.sibling-a.slint",
        ),
        (
            "child-a",
            "<outline-root>",
            "OutlineCases.reparent.child-a-root.slint",
        ),
    ],
    ids=["child", "root"],
)
def test_outline_changes_element_parent_with_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    source: str,
    target: str,
    golden: str,
) -> None:
    source_file = fixture_project / "OutlineCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        drag_row(window, source, target, "onto")
        snapshot.wait_for_exact((GOLDENS / golden).read_bytes(), "OutlineCases.slint")
        expected = (
            [
                ("container", "Hierarchy level 2", False),
                ("child-a", "Hierarchy level 3", False),
                ("child-b", "Hierarchy level 3", False),
                ("sibling-a", "Hierarchy level 3", False),
                ("sibling-b", "Hierarchy level 2", False),
            ]
            if source == "sibling-a"
            else [
                ("container", "Hierarchy level 2", False),
                ("child-b", "Hierarchy level 3", False),
                ("sibling-a", "Hierarchy level 2", False),
                ("sibling-b", "Hierarchy level 2", False),
                ("child-a", "Hierarchy level 2", False),
            ]
        )
        wait_for_outline_state(window, expected)


def test_outline_disclosure_collapses_and_expands_without_source_edit(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "OutlineCases.slint"
    ) as editor:
        window = first_window(editor)
        outline_row(window, "container").invoke_accessible_expand_action()
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "child-a")
                else None
            )
        )
        outline_row(window, "container").invoke_accessible_expand_action()
        outline_row(window, "child-a")
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    ("key", "initial", "target", "selection"),
    [
        (keys.Return, "child-a", "child-b", "Selected Text"),
        (keys.Space, "sibling-a", "sibling-b", "Selected Image"),
    ],
    ids=["return", "space"],
)
@pytest.mark.skip(reason="No keyboard-only outline focus navigation is available")
def test_outline_keyboard_selection_synchronizes_editor(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    key: str,
    initial: str,
    target: str,
    selection: str,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "OutlineCases.slint"
    ) as editor:
        window = first_window(editor)
        initial_row = outline_row(window, initial)
        row = outline_row(window, target)
        initial_row.single_click(slint_testing.PointerEventButton.Left)
        assert initial_row.accessible_item_selected
        assert not row.accessible_item_selected
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Tab))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Tab))
        window.dispatch_event(slint_testing.KeyPressedEvent(text=key))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=key))
        wait_until(lambda: row if row.accessible_item_selected else None)
        window_element_with_label(
            window,
            selection,
            slint_testing.AccessibleRole.Region,
        )
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "source,target,location",
    [
        ("sibling-a", "sibling-a", "onto"),
        pytest.param(
            "container",
            "child-a",
            "onto",
            marks=pytest.mark.skip(
                reason="Requires a Rust descendant-cycle rejection fix"
            ),
        ),
        pytest.param(
            "<root>",
            "child-a",
            "onto",
            marks=pytest.mark.skip(
                reason="Requires a Rust component-root rejection fix"
            ),
        ),
    ],
    ids=["self", "cycle", "root"],
)
def test_illegal_outline_drops_do_not_change_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    source: str,
    target: str,
    location: str,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "OutlineCases.slint"
    ) as editor:
        drag_row(first_window(editor), source, target, location)
        snapshot.assert_unchanged()


def test_escape_cancels_outline_drag_without_source_edit(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "OutlineCases.slint"
    ) as editor:
        window = first_window(editor)
        start = center(outline_row(window, "sibling-a"))
        end = drop_position(window, "container", "onto")
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        window_element_with_label(
            window, "Outline drag preview", slint_testing.AccessibleRole.Region
        )
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Escape))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Escape))
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "Outline drag preview")
                else None
            )
        )
        snapshot.assert_unchanged()


def test_prohibited_layout_outline_drop_does_not_change_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    canvas_file = fixture_project / "CanvasCases.slint"
    with launch_editor(editor_binary, editor_environment, canvas_file) as editor:
        window = first_window(editor)
        drag_row(window, "prohibited-layout", "<component-root>", "before")
        snapshot.assert_unchanged()
