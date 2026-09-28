# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore tobytes

from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import (
    edit_field,
    inspector_field,
    slider_position,
    wait_for_field,
)
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from test_inspector import (
    INSPECTOR_SOURCE,
    INVALID_EDITS,
    artboard_pixels,
    select_element,
    shadow_expected,
    shadow_source,
)


@pytest.mark.parametrize("family", ("drop", "inner"))
@pytest.mark.parametrize(
    ("control", "label", "initial", "progress", "value"),
    [
        ("distance", "Shadow distance", 8 / 96, 0.5, "48"),
        ("blur", "Shadow blur", 16 / 128, 0.5, "64"),
        ("spread", "Shadow spread", 0.5, 0.25, "-32"),
    ],
)
@pytest.mark.parametrize("outcome", ("commit", "cancel", "selection", "source"))
def test_shadow_slider_previews_without_source_writes(
    editor_factory,
    fixture_project,
    family,
    control,
    label,
    initial,
    progress,
    value,
    outcome,
):
    source = fixture_project / INSPECTOR_SOURCE
    baseline = shadow_source(source.read_bytes(), family)
    source.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source) as editor:
        wait_for_source(source, baseline)
        window = editor.window
        select_element(window, "Rectangle")
        start = slider_position(window, label, initial)
        end = slider_position(window, label, progress)
        window.pointer.move_to(start)
        before = artboard_pixels(window)
        window.pointer.press_at(start)
        window.pointer.move_to(end)
        wait_for_field(window, label + " value", value)
        assert artboard_pixels(window) != before
        snapshot.assert_unchanged()
        if outcome == "cancel":
            window.keyboard.press_sequentially(keys.Escape)
        elif outcome == "selection":
            select_element(window, "Text")
        elif outcome == "source":
            source.write_bytes(baseline + b"\n// External edit\n")
            snapshot.wait_for_applied(
                baseline + b"\n// External edit\n", INSPECTOR_SOURCE
            )
        window.pointer.release_at(end)
        if outcome != "commit":
            if outcome == "source":
                snapshot.wait_for_applied(
                    baseline + b"\n// External edit\n", INSPECTOR_SOURCE
                )
            else:
                snapshot.assert_unchanged()
            if outcome == "selection":
                select_element(window, "Rectangle")
            assert artboard_pixels(window) == before
        else:
            expected = shadow_expected(baseline, family, control, value)
            snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
            window.keyboard.shortcut(keys.Control, "z")
            snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)
            window.keyboard.shortcut(keys.Control, keys.Shift, "z")
            snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)


@pytest.mark.parametrize("family", ("drop", "inner"))
@pytest.mark.parametrize("outcome", ("commit", "cancel"))
def test_shadow_angle_previews_without_source_writes(
    editor_factory, fixture_project, family, outcome
):
    source = fixture_project / INSPECTOR_SOURCE
    baseline = shadow_source(source.read_bytes(), family)
    source.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source) as editor:
        wait_for_source(source, baseline)
        window = editor.window
        select_element(window, "Rectangle")
        dial = inspector_field(window, "Shadow angle", "slider")
        position, size = dial.absolute_position, dial.size
        center_x = position.x + size.width / 2
        center_y = position.y + size.height / 2
        start = slint_testing.LogicalPosition(center_x, center_y + size.height / 3)
        end = slint_testing.LogicalPosition(center_x + size.width / 3, center_y)
        window.pointer.move_to(start)
        before = artboard_pixels(window)
        window.pointer.press_at(start)
        window.pointer.move_to(end)
        wait_for_field(window, "Shadow angle", "0", "slider")
        assert artboard_pixels(window) != before
        snapshot.assert_unchanged()
        if outcome == "cancel":
            window.keyboard.press_sequentially(keys.Escape)
        window.pointer.release_at(end)
        if outcome == "cancel":
            snapshot.assert_unchanged()
            assert artboard_pixels(window) == before
        else:
            snapshot.wait_for_applied(
                shadow_expected(baseline, family, "angle", "0"), INSPECTOR_SOURCE
            )


@pytest.mark.parametrize("family", ("drop", "inner"))
def test_shadow_distance_keeps_direction_through_zero(
    editor_factory, fixture_project, family
):
    source = fixture_project / INSPECTOR_SOURCE
    baseline = (
        shadow_source(source.read_bytes(), family)
        .replace(
            f"{family}-shadow-offset-x: 0px;".encode(),
            f"{family}-shadow-offset-x: -8px;".encode(),
        )
        .replace(
            f"{family}-shadow-offset-y: 8px;".encode(),
            f"{family}-shadow-offset-y: 0px;".encode(),
        )
    )
    source.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source) as editor:
        wait_for_source(source, baseline)
        window = editor.window
        select_element(window, "Rectangle")
        start = slider_position(window, "Shadow distance", 8 / 96)
        zero = slider_position(window, "Shadow distance", 0)
        end = slider_position(window, "Shadow distance", 0.5)
        window.pointer.move_to(start)
        window.pointer.press_at(start)
        window.pointer.move_to(zero)
        wait_for_field(window, "Shadow distance value", "0")
        window.pointer.move_to(end)
        wait_for_field(window, "Shadow distance value", "48")
        snapshot.assert_unchanged()
        window.pointer.release_at(end)
        expected = baseline.replace(
            f"{family}-shadow-offset-x: -8px;".encode(),
            f"{family}-shadow-offset-x: -48px;".encode(),
        )
        snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
        window.keyboard.shortcut(keys.Control, "z")
        snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)


@pytest.mark.parametrize("effect", ("none", "drop", "inner"))
def test_rectangle_effect_value_writes_exact_source(
    editor_factory,
    fixture_project: Path,
    effect: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    starting_source = shadow_source(baseline, "inner" if effect == "drop" else "drop")
    if effect == "drop":
        source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = b"".join(
        line
        for line in baseline.splitlines(keepends=True)
        if not line.lstrip().startswith(b"drop-shadow-")
    )
    if effect != "none":
        color = "#00000040" if effect == "drop" else "#00000030"
        properties = (
            f"        {effect}-shadow-color: {color};\n"
            f"        {effect}-shadow-blur: 16px;\n"
            f"        {effect}-shadow-spread: 0px;\n"
            f"        {effect}-shadow-offset-x: 0px;\n"
            f"        {effect}-shadow-offset-y: 8px;\n"
        ).encode()
        anchor = (
            b"        height: 96px;\n        background: #2563eb;"
            if effect == "drop"
            else b"        background: #2563eb;"
        )
        expected = replace_once(expected, anchor, properties + anchor)
    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Rectangle")
        edit_field(window, "Rectangle effect", effect, "combobox")
        snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
        wait_for_field(
            window,
            "Rectangle effect",
            {"none": "None", "drop": "Drop Shadow", "inner": "Inner Shadow"}[effect],
            "combobox",
        )
        select_element(window, "Rectangle")
        window.keyboard.shortcut(keys.Control, "z")
        snapshot.wait_for_applied(starting_source, INSPECTOR_SOURCE)
        window.keyboard.shortcut(keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)


@pytest.mark.parametrize(
    ("case", "kind", "label", "value"),
    INVALID_EDITS,
    ids=tuple(case for case, _, _, _ in INVALID_EDITS),
)
def test_invalid_or_empty_inspector_edit_does_not_change_source(
    editor_factory,
    fixture_project: Path,
    case: str,
    kind: str,
    label: str,
    value: str,
) -> None:
    assert case
    source_file = fixture_project / INSPECTOR_SOURCE
    snapshot = SourceSnapshot.capture(fixture_project)

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, kind)
        role = "combobox" if label == "Image fit" else None
        field = inspector_field(window, label, role)
        value_before = field.accessible_value
        edit_field(window, label, value, role)
        snapshot.assert_unchanged()
        wait_for_field(window, label, value_before, role)
        window.get_by_role("region", name=f"Selected {kind}").wait_for()


def test_invalid_rectangle_color_does_not_change_source(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    snapshot = SourceSnapshot.capture(fixture_project)

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Rectangle")
        edit_field(
            window,
            "Rectangle background",
            "not-a-color",
            "text-input",
        )
        snapshot.assert_unchanged()
        wait_for_field(
            window,
            "Rectangle background",
            "#2563eb",
            "text-input",
        )
        window.get_by_role("region", name="Selected Rectangle").wait_for()
