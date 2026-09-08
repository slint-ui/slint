# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
import time
from pathlib import Path

import pytest
import slint_testing
from slint_testing import keys
from source_snapshot import SourceSnapshot
from test_inspector import edit_field as edit_inspector_field
from test_inspector import wait_for_field as wait_for_inspector_field
from ui_driver import (
    first_window,
    launch_editor,
    select_outline_row,
    window_element_with_label,
)

SOURCE = "InspectorCases.slint"
PROPERTIES = (
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-left-radius",
    "border-bottom-right-radius",
)
LABELS = (
    "Top left corner radius",
    "Top right corner radius",
    "Bottom left corner radius",
    "Bottom right corner radius",
)


def prepare(project: Path, values=(12, 12, 12, 12), rotation="32deg") -> bytes:
    source = project / SOURCE
    original = source.read_text()
    properties = "".join(
        f"        {name}: {value}px;\n" for name, value in zip(PROPERTIES, values)
    )
    original = original.replace(
        "        background: #2563eb;",
        "        background: #2563eb;\n"
        + properties
        + f"        transform-rotation: {rotation};",
    )
    source.write_text(original)
    return source.read_bytes()


def select_element(window, kind):
    select_outline_row(window, "inspect-" + kind.lower())
    window_element_with_label(
        window, "Rotation", slint_testing.AccessibleRole.TextInput
    )


def edit_field(window, label, value):
    edit_inspector_field(window, label, value, slint_testing.AccessibleRole.TextInput)


def wait_for_field(window, label, value):
    wait_for_inspector_field(
        window, label, value, slint_testing.AccessibleRole.TextInput
    )


def action(window, label):
    window_element_with_label(
        window, label, slint_testing.AccessibleRole.Button
    ).invoke_accessible_default_action()


def shortcut(window, redo=False):
    # Source writes precede the replacement preview that re-enables undo/redo.
    time.sleep(0.5)
    window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Control))
    if redo:
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
    window.dispatch_event(slint_testing.KeyPressedEvent(text="z"))
    window.dispatch_event(slint_testing.KeyReleasedEvent(text="z"))
    if redo:
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
    window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Control))


@pytest.mark.parametrize("value", ["-22.5", "0", "382.25"])
def test_rotation_numeric_exact_source_and_undo(
    editor_binary, editor_environment, fixture_project, value
):
    baseline = prepare(fixture_project)
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = baseline.replace(
        b"transform-rotation: 32deg", f"transform-rotation: {value}deg".encode()
    )
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        edit_field(window, "Rotation", value)
        snapshot.wait_for_exact(expected, relative_path=SOURCE)
        wait_for_field(window, "Rotation", value)
        shortcut(window)
        snapshot.wait_for_exact(baseline, relative_path=SOURCE)
        wait_for_field(window, "Rotation", "32")
        shortcut(window, redo=True)
        snapshot.wait_for_exact(expected, relative_path=SOURCE)


@pytest.mark.parametrize("index", range(4))
def test_corner_edit_changes_only_one_property(
    editor_binary, editor_environment, fixture_project, index
):
    baseline = prepare(fixture_project)
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = baseline.replace(
        f"{PROPERTIES[index]}: 12px".encode(),
        f"{PROPERTIES[index]}: 30.5px".encode(),
    )
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        action(window, "Separate corners")
        snapshot.assert_unchanged()
        edit_field(window, LABELS[index], "30.5")
        snapshot.wait_for_exact(expected, relative_path=SOURCE)
        wait_for_field(window, LABELS[index], "30.5")


def test_link_uses_top_left_and_one_undo_restores_expressions(
    editor_binary, editor_environment, fixture_project
):
    baseline = prepare(fixture_project, (8, 16, 24, 32))
    baseline = baseline.replace(
        b"border-top-left-radius: 8px", b"border-top-left-radius: 4px + 4px"
    )
    (fixture_project / SOURCE).write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = baseline
    for name, value in zip(PROPERTIES, ("4px + 4px", "16px", "24px", "32px")):
        expected = expected.replace(
            f"{name}: {value}".encode(), f"{name}: 8px".encode()
        )
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        wait_for_field(window, LABELS[0], "8")
        action(window, "All corners")
        snapshot.wait_for_exact(expected, relative_path=SOURCE)
        wait_for_field(window, "All corner radii", "8")
        shortcut(window)
        snapshot.wait_for_exact(baseline, relative_path=SOURCE)
        wait_for_field(window, LABELS[1], "16")
        shortcut(window, redo=True)
        snapshot.wait_for_exact(expected, relative_path=SOURCE)


@pytest.mark.parametrize("value", ["0", "30.5", "120"])
def test_shared_corner_value_is_atomic_and_not_clamped(
    editor_binary, editor_environment, fixture_project, value
):
    baseline = prepare(fixture_project)
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = baseline
    for name in PROPERTIES:
        expected = expected.replace(
            f"{name}: 12px".encode(), f"{name}: {value}px".encode()
        )
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        edit_field(window, "All corner radii", value)
        snapshot.wait_for_exact(expected, relative_path=SOURCE)
        wait_for_field(window, "All corner radii", value)
        shortcut(window)
        snapshot.wait_for_exact(baseline, relative_path=SOURCE)


@pytest.mark.parametrize(
    "label,value",
    [
        ("Rotation", "invalid"),
        ("Rotation", ""),
        ("All corner radii", "-1"),
        ("All corner radii", "invalid"),
    ],
)
def test_invalid_transform_input_preserves_source(
    editor_binary, editor_environment, fixture_project, label, value
):
    prepare(fixture_project)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        edit_field(window, label, value)
        snapshot.assert_unchanged()


def point(knob, angle):
    position = knob.absolute_position
    return slint_testing.LogicalPosition(
        x=position.x + 16 + 12 * math.sin(math.radians(angle)),
        y=position.y + 16 - 12 * math.cos(math.radians(angle)),
    )


@pytest.mark.parametrize("cancel", ["release", "escape", "pointer"])
def test_knob_crosses_zero_with_transient_preview(
    editor_binary, editor_environment, fixture_project, cancel
):
    baseline = prepare(fixture_project, rotation="350deg")
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        knob = window_element_with_label(window, "Rotation knob")
        assert knob.size.width == 32 and knob.size.height == 32
        start, end = point(knob, 350), point(knob, 10)
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                start, slint_testing.PointerEventButton.Left
            )
        )
        wait_for_field(window, "Rotation", "350")
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(window, "Rotation", "370")
        snapshot.assert_unchanged()
        if cancel == "escape":
            window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Escape))
            window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Escape))
        if cancel == "pointer":
            window.dispatch_event(slint_testing.PointerExitedEvent())
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                end, slint_testing.PointerEventButton.Left
            )
        )
        if cancel != "release":
            snapshot.assert_unchanged()
            wait_for_field(window, "Rotation", "350")
        else:
            snapshot.wait_for_exact(
                baseline.replace(b"350deg", b"370deg"), relative_path=SOURCE
            )
            shortcut(window)
            snapshot.wait_for_exact(baseline, relative_path=SOURCE)


def test_rotation_under_rotated_parent_is_parent_relative(
    editor_binary, editor_environment, fixture_project
):
    baseline = prepare(fixture_project, rotation="20deg")
    baseline = baseline.replace(
        b"    inspect-rectangle := Rectangle {",
        b"    Rectangle {\n        transform-rotation: 45deg;\n        width: 600px; height: 400px;\n    inspect-rectangle := Rectangle {",
    )
    baseline = baseline.replace(
        b"    inspect-text := Text {", b"    }\n\n    inspect-text := Text {"
    )
    (fixture_project / SOURCE).write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        wait_for_field(window, "Rotation", "20")
        edit_field(window, "Rotation", "22.5")
        snapshot.wait_for_exact(
            baseline.replace(b"20deg", b"22.5deg"), relative_path=SOURCE
        )


@pytest.mark.parametrize("shift,expected", [(False, "33"), (True, "47")])
def test_knob_keyboard_step(
    editor_binary, editor_environment, fixture_project, shift, expected
):
    baseline = prepare(fixture_project)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        knob = window_element_with_label(window, "Rotation knob")
        start = point(knob, 32)
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                start, slint_testing.PointerEventButton.Left
            )
        )
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                start, slint_testing.PointerEventButton.Left
            )
        )
        if shift:
            window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.UpArrow))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.UpArrow))
        if shift:
            window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
        snapshot.wait_for_exact(
            baseline.replace(b"32deg", f"{expected}deg".encode()), relative_path=SOURCE
        )


def test_selection_change_cancels_knob_drag(
    editor_binary, editor_environment, fixture_project
):
    prepare(fixture_project)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        knob = window_element_with_label(window, "Rotation knob")
        start, end = point(knob, 32), point(knob, 62)
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                start, slint_testing.PointerEventButton.Left
            )
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(window, "Rotation", "62")
        select_element(window, "Text")
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                end, slint_testing.PointerEventButton.Left
            )
        )
        snapshot.assert_unchanged()
        select_element(window, "Rectangle")
        wait_for_field(window, "Rotation", "32")


@pytest.mark.parametrize("cancel", [False, True])
def test_corner_slider_previews_then_commits_once(
    editor_binary, editor_environment, fixture_project, cancel
):
    baseline = prepare(fixture_project)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        slider = window_element_with_label(window, "All corner radii slider")
        pos, size = slider.absolute_position, slider.size
        start = slint_testing.LogicalPosition(
            x=pos.x + 6 + (size.width - 12) / 4, y=pos.y + 12
        )
        end = slint_testing.LogicalPosition(x=pos.x + size.width - 6, y=pos.y + 12)
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                start, slint_testing.PointerEventButton.Left
            )
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(window, "All corner radii", "48")
        snapshot.assert_unchanged()
        if cancel:
            window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Escape))
            window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Escape))
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                end, slint_testing.PointerEventButton.Left
            )
        )
        if cancel:
            snapshot.assert_unchanged()
            wait_for_field(window, "All corner radii", "12")
        else:
            expected = baseline
            for name in PROPERTIES:
                expected = expected.replace(
                    f"{name}: 12px".encode(), f"{name}: 48px".encode()
                )
            snapshot.wait_for_exact(expected, relative_path=SOURCE)
            wait_for_field(window, "All corner radii", "48")
            shortcut(window)
            snapshot.wait_for_exact(baseline, relative_path=SOURCE)


@pytest.mark.parametrize("radius", ["0", "30.5"])
def test_shared_radius_reads_effective_shorthand(
    editor_binary, editor_environment, fixture_project, radius
):
    baseline = prepare(fixture_project)
    for name in PROPERTIES:
        baseline = baseline.replace(f"        {name}: 12px;\n".encode(), b"")
    baseline = baseline.replace(
        b"        background: #2563eb;",
        f"        background: #2563eb;\n        border-radius: {radius}px;".encode(),
    )
    (fixture_project / SOURCE).write_bytes(baseline)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        wait_for_field(window, "All corner radii", radius)


def test_knob_shift_drag_snaps_and_retains_keyboard_focus(
    editor_binary, editor_environment, fixture_project
):
    baseline = prepare(fixture_project)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        knob = window_element_with_label(window, "Rotation knob")
        start, end = point(knob, 90), point(knob, 108)
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                start, slint_testing.PointerEventButton.Left
            )
        )
        wait_for_field(window, "Rotation", "32")
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(window, "Rotation", "45")
        snapshot.assert_unchanged()
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                end, slint_testing.PointerEventButton.Left
            )
        )
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
        snapshot.wait_for_exact(
            baseline.replace(b"32deg", b"45deg"), relative_path=SOURCE
        )
        wait_for_field(window, "Rotation", "45")
        time.sleep(0.5)
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.UpArrow))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.UpArrow))
        snapshot.wait_for_exact(
            baseline.replace(b"32deg", b"46deg"), relative_path=SOURCE
        )
        shortcut(window)
        snapshot.wait_for_exact(
            baseline.replace(b"32deg", b"45deg"), relative_path=SOURCE
        )
        shortcut(window)
        snapshot.wait_for_exact(baseline, relative_path=SOURCE)


def test_source_reload_cancels_knob_gesture(
    editor_binary, editor_environment, fixture_project
):
    baseline = prepare(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / SOURCE
    ) as app:
        window = first_window(app)
        select_element(window, "Rectangle")
        knob = window_element_with_label(window, "Rotation knob")
        start, end = point(knob, 32), point(knob, 62)
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                start, slint_testing.PointerEventButton.Left
            )
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(window, "Rotation", "62")
        updated = baseline.replace(b"32deg", b"17.5deg")
        (fixture_project / SOURCE).write_bytes(updated)
        wait_for_field(window, "Rotation", "17.5")
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                end, slint_testing.PointerEventButton.Left
            )
        )
        time.sleep(0.2)
        assert (fixture_project / SOURCE).read_bytes() == updated
