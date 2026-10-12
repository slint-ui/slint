# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from inspector_interactions import edit_field, inspector_field, wait_for_field
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_driver import (
    element,
    elements,
    first_window,
    launch_editor,
    press_keys,
    press_shortcut,
    screenshot,
    select_outline_row,
)

SOURCE = "InspectorCases.slint"


@pytest.fixture
def border_scene(fixture_project: Path) -> Path:
    source = fixture_project / SOURCE
    source.write_bytes(
        replace_once(
            source.read_bytes(),
            b"        background: #2563eb;",
            b"        background: #2563eb;\n"
            b"        border-width: 2px;\n"
            b"        border-color: #64748b;",
        )
    )
    return source


def border_pixel(window: slint_testing.Window) -> tuple[int, ...]:
    rectangle = element(window, id="InspectorCases::inspect-rectangle")
    position, size = rectangle.absolute_position, rectangle.size
    image = screenshot(window)
    scale = image.width / window.size.width
    pixel = image.getpixel(
        (round((position.x + 6) * scale), round((position.y + size.height / 2) * scale))
    )
    assert isinstance(pixel, tuple)
    return pixel


@pytest.mark.parametrize(
    ("label", "value", "old", "new"),
    [
        ("Border width", "12", b"border-width: 2px;", b"border-width: 12px;"),
        ("Border width", "0", b"border-width: 2px;", b"border-width: 0px;"),
        ("Border width", "2.5", b"border-width: 2px;", b"border-width: 2.5px;"),
        (
            "Border color",
            "ABCDEF",
            b"border-color: #64748b;",
            b"border-color: #abcdef;",
        ),
        (
            "Border color opacity",
            "40",
            b"border-color: #64748b;",
            b"border-color: #64748b66;",
        ),
    ],
    ids=("width", "zero", "fractional", "color", "opacity"),
)
def test_border_fields_commit_and_undo(
    editor_binary,
    editor_environment,
    fixture_project,
    border_scene,
    label,
    value,
    old,
    new,
):
    baseline = border_scene.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, border_scene) as editor:
        window = first_window(editor)
        select_outline_row(window, "inspect-rectangle")
        edit_field(window, label, value, slint_testing.AccessibleRole.TextInput)
        snapshot.wait_for_applied(expected, SOURCE)
        wait_for_field(window, label, value, slint_testing.AccessibleRole.TextInput)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, SOURCE)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, SOURCE)


@pytest.mark.parametrize("control", ("width", "opacity"))
@pytest.mark.parametrize("outcome", ("commit", "cancel", "selection", "source"))
def test_border_scrub_previews_and_reverts(
    editor_binary, editor_environment, fixture_project, border_scene, control, outcome
):
    baseline = border_scene.read_bytes()
    if control == "opacity":
        baseline = replace_once(baseline, b"border-width: 2px;", b"border-width: 12px;")
        border_scene.write_bytes(baseline)
    label, delta, displayed, old, new = (
        ("Border width", 12, "14", b"border-width: 2px;", b"border-width: 14px;")
        if control == "width"
        else (
            "Border color opacity",
            -20,
            "80",
            b"border-color: #64748b;",
            b"border-color: #64748bcc;",
        )
    )
    expected = replace_once(baseline, old, new)
    external = baseline + b"\n// External edit\n"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, border_scene) as editor:
        window = first_window(editor)
        select_outline_row(window, "inspect-rectangle")
        scrubber = inspector_field(
            window, label + " scrubber", slint_testing.AccessibleRole.Slider
        )
        start = center(scrubber)
        end = slint_testing.LogicalPosition(x=start.x + delta, y=start.y)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        before = border_pixel(window)
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(window, label, displayed, slint_testing.AccessibleRole.TextInput)
        assert border_pixel(window) != before
        snapshot.assert_unchanged()
        if outcome == "cancel":
            press_keys(window, keys.Escape)
        elif outcome == "selection":
            select_outline_row(window, "inspect-text")
        elif outcome == "source":
            border_scene.write_bytes(external)
            snapshot.wait_for_applied(external, SOURCE)
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        if outcome == "commit":
            snapshot.wait_for_applied(expected, SOURCE)
            press_shortcut(window, keys.Control, "z")
            snapshot.wait_for_applied(baseline, SOURCE)
            assert border_pixel(window) == before
            press_shortcut(window, keys.Control, keys.Shift, "z")
            snapshot.wait_for_applied(expected, SOURCE)
            assert border_pixel(window) != before
        else:
            if outcome == "source":
                snapshot.wait_for_applied(external, SOURCE)
            else:
                snapshot.assert_unchanged()
            if outcome == "selection":
                select_outline_row(window, "inspect-rectangle")
            assert border_pixel(window) == before


@pytest.mark.parametrize(
    ("label", "value", "original"),
    [
        ("Border width", "-1", "2"),
        ("Border width", "NaN", "2"),
        ("Border color", "invalid-color", "64748B"),
    ],
)
def test_invalid_border_edits_preserve_source(
    editor_binary,
    editor_environment,
    fixture_project,
    border_scene,
    label,
    value,
    original,
):
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, border_scene) as editor:
        window = first_window(editor)
        select_outline_row(window, "inspect-rectangle")
        edit_field(window, label, value, slint_testing.AccessibleRole.TextInput)
        snapshot.assert_unchanged()
        wait_for_field(window, label, original, slint_testing.AccessibleRole.TextInput)


def test_border_defaults_and_rectangle_selection(
    editor_binary, editor_environment, fixture_project
):
    source = fixture_project / SOURCE
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        select_outline_row(window, "inspect-rectangle")
        wait_for_field(
            window, "Border width", "0", slint_testing.AccessibleRole.TextInput
        )
        width = inspector_field(
            window, "Border width", slint_testing.AccessibleRole.TextInput
        )
        suffixes = (
            width.query_descendants()
            .match_id("InspectorTextFieldBase::fixed-suffix")
            .find_all()
        )
        assert len(suffixes) == 1
        assert suffixes[0].accessible_label == "px"
        inspector_field(
            window, "Border color opacity", slint_testing.AccessibleRole.TextInput
        )
        picker = inspector_field(
            window, "Border color color picker", slint_testing.AccessibleRole.Button
        )
        picker.invoke_accessible_default_action()
        element(window, "Hex color", role=slint_testing.AccessibleRole.TextInput)
        element(window, "Gradient", role=slint_testing.AccessibleRole.Button)
        element(
            window, "Close Custom", role=slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        for row in ("inspect-text", "inspect-image"):
            select_outline_row(window, row)
            pane = element(window, "Inspector and outline")
            assert not elements(
                pane, "Border width", role=slint_testing.AccessibleRole.TextInput
            )
            assert not elements(
                pane, "Border color", role=slint_testing.AccessibleRole.TextInput
            )
        snapshot.assert_unchanged()
