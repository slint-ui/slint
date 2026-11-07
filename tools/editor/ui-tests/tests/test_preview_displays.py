# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_assertions import expect
from ui_driver import (
    element,
    first_window,
    launch_editor,
    press_key,
    press_keys,
    press_shortcut,
    query,
)


@pytest.fixture
def display_project(tmp_path: Path) -> Path:
    source = tmp_path / "Main.slint"
    source.write_text(
        "export component Main inherits Window { background: #47a87b; }\n"
    )
    (tmp_path / "slint-project.json").write_text(
        json.dumps(
            {
                "edition": "1.20",
                "main": "Main.slint",
                "metadata": {
                    "visual-editor": {"canvas-presets": [{"width": 320, "height": 240}]}
                },
            }
        )
    )
    return source


def choose_display(window: slint_testing.Window, name: str) -> None:
    element(
        window, "Preview display", role=slint_testing.AccessibleRole.Combobox
    ).invoke_accessible_expand_action()
    element(window, name, role=slint_testing.AccessibleRole.Button).single_click(
        slint_testing.PointerEventButton.Left
    )


def expect_canvas_size(window: slint_testing.Window, width: int, height: int) -> None:
    artboard = element(window, "Artboard", role=slint_testing.AccessibleRole.Region)
    expect.poll(lambda: (artboard.size.width, artboard.size.height)).to_equal(
        (width, height)
    )


def test_desktop_size_and_disabled_orientation(
    editor_binary: Path, editor_environment: dict[str, str], display_project: Path
) -> None:
    with launch_editor(editor_binary, editor_environment, display_project) as editor:
        wait_for_source(display_project, display_project.read_bytes())
        window = first_window(editor)
        choose_display(window, "Desktop")
        expect_canvas_size(window, 1280, 720)
        for label in ["Portrait", "Landscape"]:
            button = element(window, label, role=slint_testing.AccessibleRole.Button)
            expect(button).to_be_enabled(False)
            assert button.accessible_description == "Not supported for Desktop"
            button.single_click(slint_testing.PointerEventButton.Left)
            expect_canvas_size(window, 1280, 720)


def test_mobile_rotation_does_not_change_project_files(
    editor_binary: Path, editor_environment: dict[str, str], display_project: Path
) -> None:
    snapshot = SourceSnapshot.capture(display_project.parent)
    project_file = display_project.with_name("slint-project.json")
    manifest = project_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, display_project) as editor:
        wait_for_source(display_project, display_project.read_bytes())
        window = first_window(editor)
        choose_display(window, "Mobile")
        for label, width, height in [("Landscape", 844, 390), ("Portrait", 390, 844)]:
            button = element(window, label, role=slint_testing.AccessibleRole.Button)
            expect(button).to_be_enabled()
            button.single_click(slint_testing.PointerEventButton.Left)
            expect_canvas_size(window, width, height)
        snapshot.assert_unchanged()
        assert project_file.read_bytes() == manifest


def test_project_size_restores_shared_dimensions(
    editor_binary: Path, editor_environment: dict[str, str], display_project: Path
) -> None:
    with launch_editor(editor_binary, editor_environment, display_project) as editor:
        wait_for_source(display_project, display_project.read_bytes())
        window = first_window(editor)
        expect_canvas_size(window, 320, 240)
        choose_display(window, "Tablet")
        expect_canvas_size(window, 768, 1024)
        choose_display(window, "Project size")
        expect_canvas_size(window, 320, 240)


def test_custom_displays_can_be_added_and_deleted_independently(
    editor_binary: Path, editor_environment: dict[str, str], display_project: Path
) -> None:
    left = slint_testing.PointerEventButton.Left
    with launch_editor(editor_binary, editor_environment, display_project) as editor:
        wait_for_source(display_project, display_project.read_bytes())
        window = first_window(editor)
        element(
            window, "Preview display", role=slint_testing.AccessibleRole.Combobox
        ).invoke_accessible_expand_action()
        for width, height in [(640, 360), (800, 480)]:
            element(
                window, "Add display", role=slint_testing.AccessibleRole.Button
            ).single_click(left)
            picker = element(window, id="DisplayManager::picker")
            for label, value in [("Width", width), ("Height", height)]:
                field = element(
                    picker, label, role=slint_testing.AccessibleRole.TextInput
                )
                field.single_click(left)
                press_shortcut(window, keys.Control, "a")
                press_keys(window, str(value))
                press_key(window, keys.Return)
            element(
                picker, "Add display", role=slint_testing.AccessibleRole.Button
            ).single_click(left)
            expect_canvas_size(window, width, height)
        element(
            window, "Delete 800 × 480", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        expect(
            query(window, "800 × 480", role=slint_testing.AccessibleRole.Button)
        ).to_be_hidden()
        expect(
            query(window, "640 × 360", role=slint_testing.AccessibleRole.Button)
        ).to_be_visible()
        expect_canvas_size(window, 320, 240)
        element(
            window, "Add display", role=slint_testing.AccessibleRole.Button
        ).single_click(left)
        expect(query(window, id="DisplayManager::picker")).to_be_visible()
