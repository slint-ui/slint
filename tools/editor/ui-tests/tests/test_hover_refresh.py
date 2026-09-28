# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import sys
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center, fixture_element
from editor_sync import wait_for_source
from inspector_interactions import FIELDS, edit_field
from slint_test import Window, expect
from slint_testing import keys
from source_snapshot import wait_for_source_change
from ui_driver import (
    first_window,
    launch_editor,
    select_fixture_element,
)


def hover_rectangle(window: Window) -> None:
    rectangle = fixture_element(window, "Rectangle")
    position = rectangle.absolute_position
    window.pointer.move_to(
        slint_testing.LogicalPosition(x=position.x + 40, y=position.y + 60)
    )
    window.get_by_accessible_name("Hovered Rectangle").resolve()


def wait_for_no_rectangle_hover(window: Window) -> None:
    expect(window.get_by_accessible_name("Hovered Rectangle")).to_be_hidden()


@pytest.mark.parametrize("operation", ["delete", "backspace", "source-removal"])
def test_removing_hovered_element_without_pointer_motion(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    operation: str,
) -> None:
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, baseline)
        select_fixture_element(window, "Rectangle")
        hover_rectangle(window)

        if operation == "source-removal":
            start = baseline.index(b"    root-rectangle :=")
            end = baseline.index(b"    root-text :=")
            expected = baseline[:start] + baseline[end:]
            source.write_bytes(expected)
        else:
            window.keyboard.press(
                keys.Delete if operation == "delete" else keys.Backspace
            )
            expected = wait_for_source_change(source, baseline)
            assert b"root-rectangle :=" not in expected

        wait_for_source(source, expected)
        assert not window.get_by_id("Main::root-rectangle").all()
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

        window.keyboard.shortcut(keys.Control, "z")
        wait_for_source(source, baseline)
        wait_for_no_rectangle_hover(window)

        if sys.platform == "win32":
            window.keyboard.shortcut(keys.Control, "y")
        else:
            window.keyboard.shortcut(keys.Control, keys.Shift, "z")
        wait_for_source(source, moved)
        window.get_by_accessible_name("Hovered Rectangle").resolve()


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
        expect(window.get_by_accessible_name("Hovered Rectangle")).to_have_geometry(
            width=pytest.approx(100)
        )


def test_pointer_motion_clears_hover_after_removal(
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
        window.keyboard.press(keys.Delete)
        expected = wait_for_source_change(source, baseline)
        assert b"root-rectangle :=" not in expected
        wait_for_source(source, expected)
        window.pointer.move_to(center(fixture_element(window, "Image")))
        window.get_by_accessible_name("Hovered Image").resolve()
        wait_for_no_rectangle_hover(window)


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
        expect(window.get_by_accessible_name("Hovered Text")).to_be_visible()
        expect(window.get_by_accessible_name("Hovered Rectangle")).to_be_hidden()


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
        if state == "outside":
            artboard = window.get_by_accessible_name("Artboard").resolve()
            position = slint_testing.LogicalPosition(
                x=artboard.absolute_position.x - 12,
                y=artboard.absolute_position.y - 12,
            )
        elif state == "pressed":
            rectangle = fixture_element(window, "Rectangle")
            position = slint_testing.LogicalPosition(
                x=rectangle.absolute_position.x + 40,
                y=rectangle.absolute_position.y + 60,
            )
        else:
            position = center(
                window.get_by_accessible_name("Rectangle " + state).resolve()
            )
        window.pointer.move_to(position)
        if state == "pressed":
            window.pointer.press_at(position)
        wait_for_no_rectangle_hover(window)

        expected = baseline.replace(b"#2563eb", b"#ef4444")
        source.write_bytes(expected)
        wait_for_source(source, expected)
        wait_for_no_rectangle_hover(window)
        if state == "pressed":
            window.pointer.release_at(position)
        hover_rectangle(window)
        assert source.read_bytes() == expected
