# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import inspector_field
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

INSPECTOR_SOURCE = "InspectorCases.slint"


def checked_switch(window: slint_testing.Window) -> slint_testing.Element:
    return inspector_field(window, "Checked", slint_testing.AccessibleRole.Switch)


def test_checked_switch_writes_exact_source(
    editor_binary: Path, editor_environment: dict[str, str], fixture_project: Path
) -> None:
    source = fixture_project / INSPECTOR_SOURCE
    baseline = source.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_outline_row(window, "inspect-checkbox")
        element(window, "Selected CheckBox", role=slint_testing.AccessibleRole.Region)
        expect(checked_switch(window)).to_be_checked(False)

        checked_switch(window).invoke_accessible_default_action()
        checked = wait_for_source_change(source, baseline)
        assert checked == replace_once(
            baseline,
            b"        x: 224px;\n        y: 272px;",
            b"        x: 224px;\n        y: 272px;\n        checked: true;",
        )
        expect(checked_switch(window)).to_be_checked()

        checked_switch(window).invoke_accessible_default_action()
        unchecked = wait_for_source_change(source, checked)
        assert unchecked == replace_once(
            checked, b"        checked: true;", b"        checked: false;"
        )
        expect(checked_switch(window)).to_be_checked(False)

        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(checked, INSPECTOR_SOURCE)
        expect(checked_switch(window)).to_be_checked()
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(unchecked, INSPECTOR_SOURCE)
        expect(checked_switch(window)).to_be_checked(False)
