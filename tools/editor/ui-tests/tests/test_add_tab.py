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


@pytest.mark.parametrize(
    ("row", "old", "new"),
    [
        (
            "empty-tabs",
            b"        height: 200px;\n    }\n\n    tabs",
            b"        height: 200px;\n"
            b'        Tab {\n            title: "Tab 1";\n        }\n'
            b"    }\n\n    tabs",
        ),
        (
            "tabs",
            b'            title: "Tab 1";\n        }\n',
            b'            title: "Tab 1";\n        }\n'
            b'        Tab {\n            title: "Tab 2";\n        }\n',
        ),
    ],
    ids=("empty", "after-existing-tab"),
)
def test_add_tab_appends_tab(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / TAB_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_outline_row(window, row)
        element(
            window, "Add Tab", role=slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        snapshot.wait_for_applied(expected, relative_path=TAB_SOURCE)

        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=TAB_SOURCE)


def test_add_tab_is_hidden_for_other_elements(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / TAB_SOURCE

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "first-tab")
        expect(
            query(window, "Add Tab", role=slint_testing.AccessibleRole.Button)
        ).to_be_hidden()
