# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import edit_field, inspector_field, wait_for_field
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once, wait_for_source_change
from ui_assertions import expect
from ui_driver import (
    element,
    first_window,
    launch_editor,
    press_shortcut,
    select_outline_row,
)

PROGRESS_SOURCE = "ProgressCases.slint"


def select_element(window: slint_testing.Window, row: str, type_name: str) -> None:
    select_outline_row(window, row)
    element(window, f"Selected {type_name}", role=slint_testing.AccessibleRole.Region)


def indeterminate_switch(window: slint_testing.Window) -> slint_testing.Element:
    return inspector_field(window, "Indeterminate", slint_testing.AccessibleRole.Switch)


@pytest.mark.parametrize(
    ("row", "type_name", "progress", "indeterminate"),
    [
        ("default-progress", "ProgressIndicator", "0", False),
        ("set-progress", "ProgressIndicator", "0.5", True),
        ("default-spinner", "Spinner", "0", False),
    ],
    ids=("default", "set", "spinner"),
)
def test_progress_fields_show_source_values(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    type_name: str,
    progress: str,
    indeterminate: bool,
) -> None:
    source_file = fixture_project / PROGRESS_SOURCE

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, row, type_name)
        wait_for_field(
            window, "Progress value", progress, slint_testing.AccessibleRole.TextInput
        )
        expect(indeterminate_switch(window)).to_be_checked(indeterminate)


@pytest.mark.parametrize(
    ("row", "type_name", "value", "old", "new"),
    [
        (
            "default-progress",
            "ProgressIndicator",
            "0.25",
            b"        y: 32px;\n        width: 200px;\n",
            b"        y: 32px;\n        progress: 0.25;\n        width: 200px;\n",
        ),
        (
            "set-progress",
            "ProgressIndicator",
            "0.75",
            b"        progress: 0.5;",
            b"        progress: 0.75;",
        ),
        (
            "default-spinner",
            "Spinner",
            "0.3",
            b"        y: 160px;\n        width: 48px;\n",
            b"        y: 160px;\n        progress: 0.3;\n        width: 48px;\n",
        ),
    ],
    ids=("add-progress", "progress", "spinner"),
)
def test_progress_field_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    type_name: str,
    value: str,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / PROGRESS_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_element(window, row, type_name)
        edit_field(
            window, "Progress value", value, slint_testing.AccessibleRole.TextInput
        )
        snapshot.wait_for_applied(expected, relative_path=PROGRESS_SOURCE)
        wait_for_field(
            window, "Progress value", value, slint_testing.AccessibleRole.TextInput
        )
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=PROGRESS_SOURCE)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, relative_path=PROGRESS_SOURCE)


def test_indeterminate_switch_writes_exact_source(
    editor_binary: Path, editor_environment: dict[str, str], fixture_project: Path
) -> None:
    source_file = fixture_project / PROGRESS_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_element(window, "default-spinner", "Spinner")
        expect(indeterminate_switch(window)).to_be_checked(False)

        indeterminate_switch(window).invoke_accessible_default_action()
        indeterminate = wait_for_source_change(source_file, baseline)
        assert indeterminate == replace_once(
            baseline,
            b"        height: 48px;\n",
            b"        height: 48px;\n        indeterminate: true;\n",
        )
        expect(indeterminate_switch(window)).to_be_checked()

        indeterminate_switch(window).invoke_accessible_default_action()
        determinate = wait_for_source_change(source_file, indeterminate)
        assert determinate == replace_once(
            indeterminate,
            b"        height: 48px;\n        indeterminate: true;",
            b"        height: 48px;\n        indeterminate: false;",
        )
        expect(indeterminate_switch(window)).to_be_checked(False)

        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(indeterminate, PROGRESS_SOURCE)
        expect(indeterminate_switch(window)).to_be_checked()
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(determinate, PROGRESS_SOURCE)
        expect(indeterminate_switch(window)).to_be_checked(False)
