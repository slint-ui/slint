# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import inspector_field
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

CHECKABLE_SOURCE = "CheckableCases.slint"


def checkable_switch(window: slint_testing.Window) -> slint_testing.Element:
    return inspector_field(window, "Checkable", slint_testing.AccessibleRole.Switch)


def checked_switch(window: slint_testing.Window) -> slint_testing.Element:
    return inspector_field(window, "Checked", slint_testing.AccessibleRole.Switch)


def checked_query(window: slint_testing.Window) -> slint_testing.ElementQuery:
    return query(window, "Checked", role=slint_testing.AccessibleRole.Switch)


def select_button(window: slint_testing.Window, row: str) -> None:
    select_outline_row(window, row)
    element(window, "Selected Button", role=slint_testing.AccessibleRole.Region)


def test_checkable_switch_writes_exact_source_and_shows_checked(
    editor_binary: Path, editor_environment: dict[str, str], fixture_project: Path
) -> None:
    source = fixture_project / CHECKABLE_SOURCE
    baseline = source.read_bytes()
    expected = replace_once(
        baseline,
        b'        y: 32px;\n        text: "Plain";\n',
        b'        y: 32px;\n        checkable: true;\n        text: "Plain";\n',
    )
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_button(window, "plain-button")
        expect(checkable_switch(window)).to_be_checked(False)
        expect(checked_query(window)).to_be_hidden()

        checkable_switch(window).invoke_accessible_default_action()
        snapshot.wait_for_applied(expected, CHECKABLE_SOURCE)
        expect(checkable_switch(window)).to_be_checked()
        expect(checked_switch(window)).to_be_checked(False)

        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, CHECKABLE_SOURCE)
        expect(checkable_switch(window)).to_be_checked(False)
        expect(checked_query(window)).to_be_hidden()

        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, CHECKABLE_SOURCE)
        expect(checkable_switch(window)).to_be_checked()
        expect(checked_query(window)).to_be_visible()


def test_checked_switch_on_checkable_button_writes_exact_source(
    editor_binary: Path, editor_environment: dict[str, str], fixture_project: Path
) -> None:
    source = fixture_project / CHECKABLE_SOURCE
    baseline = source.read_bytes()
    expected = replace_once(
        baseline,
        b"        checkable: true;\n",
        b"        checkable: true;\n        checked: true;\n",
    )
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_button(window, "checkable-button")
        expect(checkable_switch(window)).to_be_checked()
        expect(checked_switch(window)).to_be_checked(False)

        checked_switch(window).invoke_accessible_default_action()
        snapshot.wait_for_applied(expected, CHECKABLE_SOURCE)
        expect(checked_switch(window)).to_be_checked()

        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, CHECKABLE_SOURCE)
        expect(checked_switch(window)).to_be_checked(False)
