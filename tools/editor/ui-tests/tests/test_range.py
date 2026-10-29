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

FIELDS = {
    "value": "Range value",
    "minimum": "Range minimum",
    "maximum": "Range maximum",
    "step": "Range step",
}
WIDGETS = pytest.mark.parametrize("widget", ("Slider", "SpinBox"))
STEP_PROPERTIES = {"Slider": b"step", "SpinBox": b"step-size"}


def source_name(widget: str) -> str:
    return f"{widget}Cases.slint"


def select_widget(window: slint_testing.Window, widget: str, row: str) -> None:
    select_outline_row(window, f"{row}-{widget.lower()}")
    element(window, f"Selected {widget}", role=slint_testing.AccessibleRole.Region)


@WIDGETS
@pytest.mark.parametrize(
    ("row", "values"),
    [
        ("default", {"value": "0", "minimum": "0", "maximum": "100", "step": "1"}),
        (
            "ranged",
            {"value": "20", "minimum": "10", "maximum": "50", "step": "5"},
        ),
    ],
    ids=("default", "ranged"),
)
def test_range_fields_show_source_values(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    widget: str,
    row: str,
    values: dict[str, str],
) -> None:
    source_file = fixture_project / source_name(widget)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_widget(window, widget, row)
        for name, value in values.items():
            wait_for_field(
                window, FIELDS[name], value, slint_testing.AccessibleRole.TextInput
            )


@WIDGETS
@pytest.mark.parametrize(
    ("row", "name", "value", "old", "new"),
    [
        (
            "default",
            "value",
            "42",
            b"        y: 32px;\n        width: 200px;\n",
            b"        y: 32px;\n        value: 42;\n        width: 200px;\n",
        ),
        (
            "ranged",
            "value",
            "30",
            b"        value: 20;",
            b"        value: 30;",
        ),
        (
            "ranged",
            "minimum",
            "5",
            b"        minimum: 10;",
            b"        minimum: 5;",
        ),
        (
            "ranged",
            "maximum",
            "80",
            b"        maximum: 50;",
            b"        maximum: 80;",
        ),
        (
            "ranged",
            "step",
            "2",
            b"        {step}: 5;",
            b"        {step}: 2;",
        ),
    ],
    ids=("add-value", "value", "minimum", "maximum", "step"),
)
def test_range_field_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    widget: str,
    row: str,
    name: str,
    value: str,
    old: bytes,
    new: bytes,
) -> None:
    source = source_name(widget)
    source_file = fixture_project / source
    baseline = source_file.read_bytes()
    step = STEP_PROPERTIES[widget]
    expected = replace_once(
        baseline, old.replace(b"{step}", step), new.replace(b"{step}", step)
    )
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_widget(window, widget, row)
        edit_field(window, FIELDS[name], value, slint_testing.AccessibleRole.TextInput)
        snapshot.wait_for_applied(expected, relative_path=source)
        wait_for_field(
            window, FIELDS[name], value, slint_testing.AccessibleRole.TextInput
        )
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=source)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, relative_path=source)


@WIDGETS
def test_invalid_range_value_does_not_change_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    widget: str,
) -> None:
    source_file = fixture_project / source_name(widget)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_widget(window, widget, "ranged")
        edit_field(
            window,
            FIELDS["value"],
            "unknown_identifier",
            slint_testing.AccessibleRole.TextInput,
        )
        snapshot.assert_unchanged()
        wait_for_field(
            window, FIELDS["value"], "20", slint_testing.AccessibleRole.TextInput
        )
