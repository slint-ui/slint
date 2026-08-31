# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    select_fixture_element,
    wait_until,
    window_element_with_label,
)

GOLDENS = Path(__file__).resolve().parents[1] / "goldens"
FOLLOWING_ELEMENT = {
    "Rectangle": b"root-text",
    "Text": b"root-image",
    "Image": b"NestedCard",
}


def press_key(window: slint_testing.Window, key: str) -> None:
    window.dispatch_event(slint_testing.KeyPressedEvent(text=key))
    window.dispatch_event(slint_testing.KeyReleasedEvent(text=key))


def deletion_golden(element_type: str) -> bytes:
    expected = (GOLDENS / f"Main.delete-{element_type.lower()}.slint").read_bytes()
    following = FOLLOWING_ELEMENT[element_type]
    return expected.replace(
        b"\n\n    " + following,
        b"\n    \n\n    " + following,
        1,
    )


def test_outline_selection_synchronizes_canvas_and_inspector(
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
        assert (
            window_element_with_label(
                window, "Position X", slint_testing.AccessibleRole.TextInput
            ).accessible_value
            == "40"
        )
        assert (
            window_element_with_label(
                window, "Width", slint_testing.AccessibleRole.TextInput
            ).accessible_value
            == "180"
        )
        snapshot.assert_unchanged()


def test_canvas_selection_synchronizes_outline_and_inspector(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        text = wait_until(
            lambda: (
                element
                if (
                    element := next(
                        iter(window.find_elements_by_id("Main::root-text")), None
                    )
                )
                else None
            )
        )
        target = slint_testing.LogicalPosition(
            x=text.absolute_position.x + text.size.width / 2,
            y=text.absolute_position.y + text.size.height / 2,
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        row = window_element_with_label(
            window, "root-text", slint_testing.AccessibleRole.ListItem
        )
        wait_until(lambda: row if row.accessible_item_selected else None)
        window_element_with_label(
            window, "Selected Text", slint_testing.AccessibleRole.Region
        )
        assert (
            window_element_with_label(
                window, "Position X", slint_testing.AccessibleRole.TextInput
            ).accessible_value
            == "180"
        )
        snapshot.assert_unchanged()


def test_clear_canvas_selection_does_not_edit_source(
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
        artboard = window_element_with_label(
            window, "Artboard", slint_testing.AccessibleRole.Region
        )
        target = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + artboard.size.width - 12,
            y=artboard.absolute_position.y + artboard.size.height - 12,
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "Selected Rectangle")
                else None
            ),
            timeout=15,
        )
        outline = window_element_with_label(
            window, "Current file outline", slint_testing.AccessibleRole.List
        )
        wait_until(
            lambda: (
                rows
                if (
                    rows := outline.query_descendants()
                    .match_accessible_role(slint_testing.AccessibleRole.ListItem)
                    .find_all()
                )
                and rows[0].accessible_item_selected
                and not window_element_with_label(
                    window,
                    "root-rectangle",
                    slint_testing.AccessibleRole.ListItem,
                ).accessible_item_selected
                else None
            )
        )
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "key", [keys.Delete, keys.Backspace], ids=["delete", "backspace"]
)
def test_delete_without_element_selection_does_not_edit_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    key: str,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        window_element_with_label(
            window, "Editor canvas", slint_testing.AccessibleRole.Main
        )
        press_key(window, key)
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "key", [keys.Backspace, keys.Delete], ids=["backspace", "delete"]
)
def test_focused_inspector_field_consumes_delete_key(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    key: str,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        field = window_element_with_label(
            window, "Position X", slint_testing.AccessibleRole.TextInput
        )
        target = slint_testing.LogicalPosition(
            x=field.absolute_position.x + field.size.width / 2,
            y=field.absolute_position.y + field.size.height / 2,
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        press_key(window, key)
        window_element_with_label(
            window, "Selected Rectangle", slint_testing.AccessibleRole.Region
        )
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "key", [keys.Delete, keys.Backspace], ids=["delete", "backspace"]
)
@pytest.mark.parametrize("element_type", ["Rectangle", "Text", "Image"])
def test_delete_selected_element_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    key: str,
    element_type: str,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, element_type)
        press_key(window, key)
        expected = deletion_golden(element_type)
        snapshot.wait_for_exact(expected)
        wait_until(
            lambda: (
                True
                if not elements_with_label(
                    window.root_element,
                    f"root-{element_type.lower()}",
                    slint_testing.AccessibleRole.ListItem,
                )
                else None
            ),
            timeout=15,
        )
        assert not elements_with_label(
            window.root_element,
            f"Selected {element_type}",
            slint_testing.AccessibleRole.Region,
        )
