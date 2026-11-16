# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_assertions import expect
from ui_driver import (
    element,
    first_window,
    launch_editor,
    press_shortcut,
    query,
    select_outline_row,
)

TAB_SOURCE = "TabCases.slint"
RADIO_GROUP_SOURCE = "RadioGroupCases.slint"


@pytest.mark.parametrize(
    ("source", "row", "button", "old", "new"),
    [
        (
            TAB_SOURCE,
            "empty-tabs",
            "Add Tab",
            b"        height: 200px;\n    }\n\n    tabs",
            (
                b"        height: 200px;\n"
                b'        Tab {\n            title: "Tab 1";\n        }\n'
                b"    }\n\n    tabs"
            ),
        ),
        (
            TAB_SOURCE,
            "tabs",
            "Add Tab",
            b'            title: "Tab 1";\n        }\n',
            (
                b'            title: "Tab 1";\n        }\n'
                b'        Tab {\n            title: "Tab 2";\n        }\n'
            ),
        ),
        (
            RADIO_GROUP_SOURCE,
            "empty-group",
            "Add Radio Button",
            b"        width: 280px;\n    }\n\n    group",
            (
                b"        width: 280px;\n"
                b'        RadioButton {\n            text: "Option 1";\n        }\n'
                b"    }\n\n    group"
            ),
        ),
        (
            RADIO_GROUP_SOURCE,
            "group",
            "Add Radio Button",
            b'            text: "Option 1";\n        }\n',
            (
                b'            text: "Option 1";\n        }\n'
                b'        RadioButton {\n            text: "Option 2";\n        }\n'
            ),
        ),
    ],
    ids=(
        "empty-tab-widget",
        "after-existing-tab",
        "empty-radio-group",
        "after-existing-radio-button",
    ),
)
def test_add_child_appends_child(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    source: str,
    row: str,
    button: str,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / source
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_outline_row(window, row)
        element(
            window, button, role=slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        snapshot.wait_for_applied(expected, relative_path=source)

        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=source)


@pytest.mark.parametrize(
    ("source", "row", "button"),
    [
        (TAB_SOURCE, "first-tab", "Add Tab"),
        (RADIO_GROUP_SOURCE, "first-option", "Add Radio Button"),
    ],
    ids=("tab", "radio-button"),
)
def test_add_child_is_hidden_for_other_elements(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    source: str,
    row: str,
    button: str,
) -> None:
    source_file = fixture_project / source

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, row)
        expect(
            query(window, button, role=slint_testing.AccessibleRole.Button)
        ).to_be_hidden()
