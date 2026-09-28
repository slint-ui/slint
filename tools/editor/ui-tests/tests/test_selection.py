# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import (
    center,
    fixture_element,
    hover_rectangle,
    wait_for_no_rectangle_hover,
)
from editor_sync import wait_for_source
from inspector_interactions import FIELDS
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    first_window,
    launch_editor,
    press_key,
    select_fixture_element,
    wait_until,
)

GOLDENS = Path(__file__).resolve().parents[1] / "goldens"
FOLLOWING_ELEMENT = {
    "Rectangle": b"root-text",
    "Text": b"root-image",
    "Image": b"NestedCard",
}


def deletion_golden(element_type: str) -> bytes:
    expected = (GOLDENS / f"Main.delete-{element_type.lower()}.slint").read_bytes()
    following = FOLLOWING_ELEMENT[element_type]
    return expected.replace(
        b"\n\n    " + following,
        b"\n    \n\n    " + following,
        1,
    )


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
                        iter(window.get_by_id("Main::root-text").all()), None
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
        row = window.get_by_role(
            slint_testing.AccessibleRole.ListItem, name="root-text"
        ).resolve()
        wait_until(lambda: row if row.accessible_item_selected else None)
        window.get_by_role(
            slint_testing.AccessibleRole.Region, name="Selected Text"
        ).resolve()
        assert (
            window.get_by_role(slint_testing.AccessibleRole.TextInput, name=FIELDS["x"])
            .resolve()
            .accessible_value
            == "180"
        )
        snapshot.assert_unchanged()


def test_clear_canvas_selection_does_not_edit_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, source.read_bytes())
        button = slint_testing.PointerEventButton.Left
        for outside in (False, True):
            select_fixture_element(window, "Rectangle")
            for label, value in ((FIELDS["x"], "40"), (FIELDS["width"], "180")):
                assert (
                    window.get_by_role(
                        slint_testing.AccessibleRole.TextInput, name=label
                    )
                    .resolve()
                    .accessible_value
                    == value
                )
            snapshot.assert_unchanged()
            if outside:
                window.get_by_accessible_name("Rectangle background").resolve()
                canvas = window.get_by_accessible_name("Editor canvas").resolve()
                target = slint_testing.LogicalPosition(
                    x=canvas.absolute_position.x + 10, y=canvas.absolute_position.y + 10
                )
                window.dispatch_event(slint_testing.PointerMoveEvent(target))
            else:
                artboard = window.get_by_role(
                    slint_testing.AccessibleRole.Region, name="Artboard"
                ).resolve()
                target = slint_testing.LogicalPosition(
                    x=artboard.absolute_position.x + artboard.size.width - 12,
                    y=artboard.absolute_position.y + artboard.size.height - 12,
                )
            window.dispatch_event(slint_testing.PointerPressEvent(target, button))
            window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))

            def selection_cleared() -> bool | None:
                outline = window.get_by_role(
                    slint_testing.AccessibleRole.List, name="Current file outline"
                ).resolve()
                rows = (
                    outline.query_descendants()
                    .match_accessible_role(slint_testing.AccessibleRole.ListItem)
                    .find_all()
                )
                return (
                    True
                    if rows
                    and not any(row.accessible_item_selected for row in rows)
                    and not any(
                        window.get_by_accessible_name(label).all()
                        for label in (
                            "Selected Rectangle",
                            "Rectangle background",
                            "Root background",
                        )
                    )
                    else None
                )

            wait_until(selection_cleared, timeout=15)
            snapshot.assert_unchanged()
        select_fixture_element(window, "Rectangle")
        outline = window.get_by_accessible_name("Current file outline").resolve()
        root_row = (
            outline.query_descendants()
            .match_accessible_role(slint_testing.AccessibleRole.ListItem)
            .find_all()[0]
        )
        root_row.invoke_accessible_default_action()
        window.get_by_accessible_name("Root background").resolve()
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
        window.get_by_role(
            slint_testing.AccessibleRole.Main, name="Editor canvas"
        ).resolve()
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
        field = window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name=FIELDS["x"]
        ).resolve()
        target = slint_testing.LogicalPosition(
            x=field.absolute_position.x + field.size.width / 2,
            y=field.absolute_position.y + field.size.height / 2,
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        press_key(window, key)
        window.get_by_role(
            slint_testing.AccessibleRole.Region, name="Selected Rectangle"
        ).resolve()
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
        wait_for_source(source_file, source_file.read_bytes())
        select_fixture_element(window, element_type)
        if element_type == "Rectangle":
            hover_rectangle(window)
        press_key(window, key)
        expected = deletion_golden(element_type)
        snapshot.wait_for_applied(expected)
        wait_until(
            lambda: (
                True
                if not window.get_by_role(
                    slint_testing.AccessibleRole.ListItem,
                    name=f"root-{element_type.lower()}",
                ).all()
                else None
            ),
            timeout=15,
        )
        assert not window.get_by_role(
            slint_testing.AccessibleRole.Region, name=f"Selected {element_type}"
        ).all()
        if element_type == "Rectangle":
            assert not window.find_elements_by_id("Main::root-rectangle")
            wait_for_no_rectangle_hover(window)
            if key == keys.Delete:
                window.dispatch_event(
                    slint_testing.PointerMoveEvent(
                        center(fixture_element(window, "Image"))
                    )
                )
                window.get_by_accessible_name("Hovered Image").resolve()
                wait_for_no_rectangle_hover(window)
