# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import edit_field, wait_for_field
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_driver import (
    element,
    first_window,
    launch_editor,
    press_shortcut,
    select_outline_row,
)

STANDARD_BUTTON_SOURCE = "StandardButtonCases.slint"


@pytest.mark.parametrize(
    ("row", "label", "value", "new_label", "old", "new"),
    [
        (
            "default-standard-button",
            "OK",
            "help",
            "Help",
            b"        x: 32px;\n        y: 32px;\n",
            b"        kind: help;\n        x: 32px;\n        y: 32px;\n",
        ),
        (
            "cancel-standard-button",
            "Cancel",
            "retry",
            "Retry",
            b"        kind: cancel;\n",
            b"        kind: retry;\n",
        ),
    ],
    ids=("default", "cancel"),
)
def test_standard_button_kind_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    label: str,
    value: str,
    new_label: str,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / STANDARD_BUTTON_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_outline_row(window, row)
        element(
            window, "Selected StandardButton", role=slint_testing.AccessibleRole.Region
        )
        role = slint_testing.AccessibleRole.Combobox
        wait_for_field(window, "Button Kind", label, role)
        edit_field(window, "Button Kind", value, role)
        snapshot.wait_for_applied(expected, relative_path=STANDARD_BUTTON_SOURCE)
        wait_for_field(window, "Button Kind", new_label, role)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=STANDARD_BUTTON_SOURCE)
        wait_for_field(window, "Button Kind", label, role)
