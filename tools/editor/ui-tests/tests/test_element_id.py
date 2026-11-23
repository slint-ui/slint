# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import wait_for_source
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_assertions import expect
from ui_driver import (
    double_click_at,
    element,
    first_window,
    launch_editor,
    outline_row,
    press_key,
    press_keys,
    press_shortcut,
    query,
    select_outline_row,
)

OUTLINE_SOURCE = "OutlineCases.slint"


def id_editor(window: slint_testing.Window) -> slint_testing.Element:
    return element(window, "Element id", role=slint_testing.AccessibleRole.TextInput)


def begin_id_edit(window: slint_testing.Window, row: str) -> None:
    select_outline_row(window, row)
    press_key(window, keys.F2)
    expect(id_editor(window)).to_have_value(row)
    assert id_editor(window).size.width > outline_row(window, row).size.width / 2


@pytest.mark.parametrize(
    ("typed", "new_id"),
    [("renamed", "renamed"), ("", ""), ("my side.bar", "my-sidebar")],
    ids=("rename", "remove", "normalize"),
)
def test_edit_element_id(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    typed: str,
    new_id: str,
) -> None:
    source_file = fixture_project / OUTLINE_SOURCE
    baseline = source_file.read_bytes()
    new_line = f"    {new_id} := Rectangle {{" if new_id else "    Rectangle {"
    expected = replace_once(
        baseline, b"    sibling-a := Rectangle {", new_line.encode()
    )
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        begin_id_edit(window, "sibling-a")
        press_key(window, keys.Backspace)
        press_keys(window, typed)
        press_key(window, keys.Return)
        snapshot.wait_for_applied(expected, relative_path=OUTLINE_SOURCE)
        expect(query(window, "Element id")).to_be_hidden()

        edited = SourceSnapshot.capture(fixture_project)
        outline = element(
            window, "Current file outline", role=slint_testing.AccessibleRole.List
        )
        row = element(
            outline, new_id or "Rectangle", role=slint_testing.AccessibleRole.ListItem
        )
        double_click_at(window, center(row))
        expect(id_editor(window)).to_have_value(new_id)
        press_key(window, keys.Return)
        expect(query(window, "Element id")).to_be_hidden()
        edited.assert_unchanged()

        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=OUTLINE_SOURCE)


def test_rejected_element_id_edits(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / OUTLINE_SOURCE
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        for typed, commit_key in (
            ("renamed", keys.Escape),
            ("container", keys.Return),
            ("root", keys.Return),
        ):
            begin_id_edit(window, "sibling-a")
            press_keys(window, typed)
            press_key(window, commit_key)
            expect(query(window, "Element id")).to_be_hidden()
            snapshot.assert_unchanged()
            outline_row(window, "sibling-a")
