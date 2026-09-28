# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from inspector_interactions import FIELDS
from slint_test import expect
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
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


def test_outline_selection_synchronizes_canvas_and_inspector(
    editor_factory,
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(fixture_project / "Main.slint") as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        assert (
            window.get_by_role("text-input", name=FIELDS["x"])
            .resolve()
            .accessible_value
            == "40"
        )
        assert (
            window.get_by_role("text-input", name=FIELDS["width"])
            .resolve()
            .accessible_value
            == "180"
        )
        snapshot.assert_unchanged()


def test_canvas_selection_synchronizes_outline_and_inspector(
    editor_factory,
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(fixture_project / "Main.slint") as editor:
        window = editor.window
        text = window.get_by_id("Main::root-text").resolve()
        target = slint_testing.LogicalPosition(
            x=text.absolute_position.x + text.size.width / 2,
            y=text.absolute_position.y + text.size.height / 2,
        )
        window.pointer.press_at(target)
        window.pointer.release_at(target)
        row = window.get_by_role("list-item", name="root-text")
        expect(row).to_be_selected()
        window.get_by_role("region", name="Selected Text").wait_for()
        expect(window.get_by_role("text-input", name=FIELDS["x"])).to_have_value("180")
        snapshot.assert_unchanged()


def test_clear_canvas_selection_does_not_edit_source(
    editor_factory,
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(fixture_project / "Main.slint") as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        artboard = window.get_by_role("region", name="Artboard").resolve()
        target = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + artboard.size.width - 12,
            y=artboard.absolute_position.y + artboard.size.height - 12,
        )
        window.pointer.press_at(target)
        window.pointer.release_at(target)
        expect(window.get_by_accessible_name("Selected Rectangle")).to_be_hidden(
            timeout=15_000
        )
        outline = window.get_by_role("list", name="Current file outline").resolve()
        wait_until(
            lambda: (
                rows
                if (
                    rows := outline.query_descendants()
                    .match_accessible_role(slint_testing.AccessibleRole.ListItem)
                    .find_all()
                )
                and not any(row.accessible_item_selected for row in rows)
                else None
            )
        )
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "key", [keys.Delete, keys.Backspace], ids=["delete", "backspace"]
)
def test_delete_without_element_selection_does_not_edit_source(
    editor_factory,
    fixture_project: Path,
    key: str,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(fixture_project / "Main.slint") as editor:
        window = editor.window
        window.get_by_role("main", name="Editor canvas").wait_for()
        window.keyboard.press(key)
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "key", [keys.Backspace, keys.Delete], ids=["backspace", "delete"]
)
def test_focused_inspector_field_consumes_delete_key(
    editor_factory,
    fixture_project: Path,
    key: str,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(fixture_project / "Main.slint") as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        field = window.get_by_role("text-input", name=FIELDS["x"]).resolve()
        target = slint_testing.LogicalPosition(
            x=field.absolute_position.x + field.size.width / 2,
            y=field.absolute_position.y + field.size.height / 2,
        )
        window.pointer.press_at(target)
        window.pointer.release_at(target)
        window.keyboard.press(key)
        window.get_by_role("region", name="Selected Rectangle").wait_for()
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "key", [keys.Delete, keys.Backspace], ids=["delete", "backspace"]
)
@pytest.mark.parametrize("element_type", ["Rectangle", "Text", "Image"])
def test_delete_selected_element_writes_exact_source(
    editor_factory,
    fixture_project: Path,
    key: str,
    element_type: str,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Main.slint"
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select(element_type)
        window.keyboard.press(key)
        expected = deletion_golden(element_type)
        snapshot.wait_for_exact(expected)
        expect(
            window.get_by_role(
                "list-item",
                name=f"root-{element_type.lower()}",
            )
        ).to_be_hidden(timeout=15_000)
        window.get_by_role("region", name=f"Selected {element_type}").wait_for(
            state="hidden"
        )
