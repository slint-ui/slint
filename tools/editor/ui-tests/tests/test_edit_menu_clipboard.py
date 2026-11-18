# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import slint_testing
from editor_sync import wait_for_source
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_driver import (
    element,
    first_window,
    launch_editor,
    press_shortcut,
    select_outline_row,
)

SOURCE = "WrapLayoutCases.slint"
BUTTON = (
    b"    wrapped-button := Button {\n"
    b"        x: 32px;\n"
    b"        y: 32px;\n"
    b'        text: "Wrap me";\n'
    b"    }\n"
)
PASTED_BUTTON = (
    b"    wrapped-button-2 := Button {\n"
    b"        x: 32px;\n"
    b"        y: 32px;\n"
    b'        text: "Wrap me";\n'
    b"    }\n"
)


def click_edit_menu_item(window: slint_testing.Window, title: str) -> None:
    element(window, "Edit").single_click(slint_testing.PointerEventButton.Left)
    element(window, title).single_click(slint_testing.PointerEventButton.Left)


def test_edit_menu_copy_paste_and_cut(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    editor_environment["SLINT_NO_MUDA"] = "1"
    source_file = fixture_project / SOURCE
    baseline = source_file.read_bytes()
    pasted = replace_once(baseline, BUTTON, BUTTON + PASTED_BUTTON)
    cut = replace_once(baseline, BUTTON, b"    \n")
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)

        select_outline_row(window, "wrapped-button")
        click_edit_menu_item(window, "Copy")
        click_edit_menu_item(window, "Paste")
        snapshot.wait_for_applied(pasted, relative_path=SOURCE)

        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=SOURCE)

        select_outline_row(window, "wrapped-button")
        click_edit_menu_item(window, "Cut")
        snapshot.wait_for_applied(cut, relative_path=SOURCE)

        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=SOURCE)
