# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import sys
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import (
    center,
    fixture_element,
    hover_rectangle,
    wait_for_no_rectangle_hover,
)
from editor_sync import wait_for_source
from inspector_interactions import FIELDS, edit_field
from slint_testing import keys
from source_snapshot import wait_for_source_change
from ui_assertions import expect
from ui_driver import (
    element,
    elements,
    first_window,
    launch_editor,
    press_shortcut,
    query,
    select_fixture_element,
    wait_until,
)


def test_source_removal_clears_hover_without_pointer_motion(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, baseline)
        select_fixture_element(window, "Rectangle")
        hover_rectangle(window)

        start = baseline.index(b"    root-rectangle :=")
        end = baseline.index(b"    root-text :=")
        expected = baseline[:start] + baseline[end:]
        source.write_bytes(expected)

        wait_for_source(source, expected)
        assert not elements(window, id="Main::root-rectangle")
        wait_for_no_rectangle_hover(window)


def test_undo_moves_hovered_element_away_from_stationary_pointer(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    baseline = b"""export component Main inherits Window {
    width: 360px;
    height: 240px;
    root-rectangle := Rectangle {
        x: 40px;
        y: 40px;
        width: 100px;
        height: 120px;
        background: blue;
    }
}
"""
    source.write_bytes(baseline)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, baseline)
        select_fixture_element(window, "Rectangle")
        edit_field(window, FIELDS["x"], "220", slint_testing.AccessibleRole.TextInput)
        moved = wait_for_source_change(source, baseline)
        assert b"x: 220px;" in moved
        wait_for_source(source, moved)
        hover_rectangle(window)

        press_shortcut(window, keys.Control, "z")
        wait_for_source(source, baseline)
        wait_for_no_rectangle_hover(window)

        if sys.platform == "win32":
            press_shortcut(window, keys.Control, "y")
        else:
            press_shortcut(window, keys.Control, keys.Shift, "z")
        wait_for_source(source, moved)
        element(window, "Hovered Rectangle")


def test_preview_reload_refreshes_hover_geometry_without_pointer_motion(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, baseline)
        hover_rectangle(window)
        expected = baseline.replace(b"width: 180px;", b"width: 100px;", 1)
        source.write_bytes(expected)
        wait_for_source(source, expected)
        assert fixture_element(window, "Rectangle").size.width == pytest.approx(100)
        expect(query(window, "Hovered Rectangle")).to_have_geometry(
            width=pytest.approx(100)
        )


def test_preview_reload_updates_hover_target_without_pointer_motion(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, baseline)
        hover_rectangle(window)
        expected = baseline.replace(b"x: 180px;", b"x: 40px;").replace(
            b"y: 56px;", b"y: 80px;"
        )
        source.write_bytes(expected)
        wait_for_source(source, expected)
        wait_until(
            lambda: (
                elements(window, "Hovered Text")
                and not elements(window, "Hovered Rectangle")
            )
        )


@pytest.mark.parametrize(
    "state",
    ["outside", "resize top-left", "rotate top-left", "radius top-left", "pressed"],
)
def test_preview_reload_preserves_hover_suppression(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    state: str,
) -> None:
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, baseline)
        select_fixture_element(window, "Rectangle")
        hover_rectangle(window)
        button = slint_testing.PointerEventButton.Left
        if state == "outside":
            artboard = element(window, "Artboard")
            position = slint_testing.LogicalPosition(
                x=artboard.absolute_position.x - 12,
                y=artboard.absolute_position.y - 12,
            )
        elif state == "pressed":
            position = center(fixture_element(window, "Rectangle"))
        else:
            position = center(element(window, "Rectangle " + state))
        window.dispatch_event(slint_testing.PointerMoveEvent(position))
        if state == "pressed":
            window.dispatch_event(slint_testing.PointerPressEvent(position, button))
        wait_for_no_rectangle_hover(window)
        if state != "outside":
            if state == "pressed":
                window.dispatch_event(
                    slint_testing.PointerReleaseEvent(position, button)
                )
            hover_rectangle(window)
            window.dispatch_event(slint_testing.PointerMoveEvent(position))
            if state == "pressed":
                window.dispatch_event(slint_testing.PointerPressEvent(position, button))
            wait_for_no_rectangle_hover(window)

        expected = baseline.replace(b"#2563eb", b"#ef4444")
        source.write_bytes(expected)
        wait_for_source(source, expected)
        wait_for_no_rectangle_hover(window)
        if state == "pressed":
            window.dispatch_event(slint_testing.PointerReleaseEvent(position, button))
        hover_rectangle(window)
        assert source.read_bytes() == expected
