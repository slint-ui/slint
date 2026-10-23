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

SLIDER_SOURCE = "SliderCases.slint"
FIELDS = {
    "value": "Slider value",
    "minimum": "Slider minimum",
    "maximum": "Slider maximum",
}


def select_slider(window: slint_testing.Window, row: str) -> None:
    select_outline_row(window, row)
    element(window, "Selected Slider", role=slint_testing.AccessibleRole.Region)


@pytest.mark.parametrize(
    ("row", "values"),
    [
        ("default-slider", {"value": "0", "minimum": "0", "maximum": "100"}),
        ("ranged-slider", {"value": "20", "minimum": "10", "maximum": "50"}),
    ],
    ids=("default", "ranged"),
)
def test_slider_range_fields_show_source_values(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    values: dict[str, str],
) -> None:
    source_file = fixture_project / SLIDER_SOURCE

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_slider(window, row)
        for name, value in values.items():
            wait_for_field(
                window, FIELDS[name], value, slint_testing.AccessibleRole.TextInput
            )


@pytest.mark.parametrize(
    ("row", "name", "value", "old", "new"),
    [
        (
            "default-slider",
            "value",
            "42",
            b"        y: 32px;\n        width: 200px;\n",
            b"        y: 32px;\n        value: 42;\n        width: 200px;\n",
        ),
        (
            "ranged-slider",
            "value",
            "30",
            b"        value: 20;",
            b"        value: 30;",
        ),
        (
            "ranged-slider",
            "minimum",
            "5",
            b"        minimum: 10;",
            b"        minimum: 5;",
        ),
        (
            "ranged-slider",
            "maximum",
            "80",
            b"        maximum: 50;",
            b"        maximum: 80;",
        ),
    ],
    ids=("add-value", "value", "minimum", "maximum"),
)
def test_slider_range_field_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    name: str,
    value: str,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / SLIDER_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_slider(window, row)
        edit_field(window, FIELDS[name], value, slint_testing.AccessibleRole.TextInput)
        snapshot.wait_for_applied(expected, relative_path=SLIDER_SOURCE)
        wait_for_field(
            window, FIELDS[name], value, slint_testing.AccessibleRole.TextInput
        )
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=SLIDER_SOURCE)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, relative_path=SLIDER_SOURCE)


def test_invalid_slider_value_does_not_change_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / SLIDER_SOURCE
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_slider(window, "ranged-slider")
        edit_field(
            window,
            "Slider value",
            "unknown_identifier",
            slint_testing.AccessibleRole.TextInput,
        )
        snapshot.assert_unchanged()
        wait_for_field(
            window, "Slider value", "20", slint_testing.AccessibleRole.TextInput
        )
