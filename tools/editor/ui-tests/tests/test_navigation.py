# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import sys
from pathlib import Path

import slint_testing
from editor_sync import wait_for_source
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    PALETTE_KINDS,
    file_row,
    first_window,
    launch_editor,
    palette_row,
    press_key,
    press_keys,
    wait_until,
)


def test_file_tree_renames_file_inline(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    target = fixture_project / "Renamed.slint"
    expected = source.read_text()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        file_row(window, source).invoke_accessible_default_action()
        press_key(window, keys.Return if sys.platform == "darwin" else keys.F2)
        window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Rename Main.slint"
        ).resolve()

        press_key(window, keys.Backspace)
        press_keys(window, "Renamed")
        press_key(window, keys.Return)

        wait_until(lambda: True if target.is_file() and not source.exists() else None)
        assert target.read_text() == expected


def test_file_tree_saves_rename_when_focus_moves(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    target = fixture_project / "Renamed.slint"
    expected = source.read_text()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        file_row(window, source).invoke_accessible_default_action()
        press_key(window, keys.Return if sys.platform == "darwin" else keys.F2)
        window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Rename Main.slint"
        ).resolve()

        press_key(window, keys.Backspace)
        press_keys(window, "Renamed")
        file_row(
            window, fixture_project / "Sibling.slint"
        ).invoke_accessible_default_action()

        wait_until(lambda: True if target.is_file() and not source.exists() else None)
        wait_until(
            lambda: (
                True
                if not window.get_by_accessible_name("Rename Main.slint").all()
                else None
            )
        )
        assert target.read_text() == expected


def test_file_tree_limits_rename_error_to_edited_row(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        file_row(window, source).invoke_accessible_default_action()
        press_key(window, keys.Return if sys.platform == "darwin" else keys.F2)
        window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Rename Main.slint"
        ).resolve()

        press_key(window, keys.Backspace)
        press_keys(window, "Sibling")
        press_key(window, keys.Return)

        wait_until(
            lambda: (
                True
                if len(
                    window.get_by_accessible_name(
                        "A file with that name already exists"
                    ).all()
                )
                == 1
                else None
            )
        )


def test_file_tree_opens_sibling_component(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        file_row(
            window, fixture_project / "Sibling.slint"
        ).invoke_accessible_default_action()
        window.get_by_role(
            slint_testing.AccessibleRole.ListItem, name="sibling-rectangle"
        ).resolve()
        assert not window.get_by_accessible_name("root-text").all()
        snapshot.assert_unchanged()


def test_file_tree_switches_image_and_component_surfaces(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Main.slint"
    assets = fixture_project / "assets"
    image = assets / "checker.svg"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        window.get_by_role(
            slint_testing.AccessibleRole.Main, name="Editor canvas"
        ).resolve()
        assert not window.get_by_accessible_name(str(image)).all()
        file_row(window, assets).invoke_accessible_default_action()
        file_row(window, image).invoke_accessible_default_action()
        image_editor = window.get_by_role(
            slint_testing.AccessibleRole.Main, name="Image asset editor"
        )
        assert (
            image_editor.read(lambda element: element.accessible_description)
            == "assets/checker.svg"
        )
        window.get_by_role(
            slint_testing.AccessibleRole.Button, name="Preview"
        ).resolve()
        file_fields = image_editor.get_by_role(
            slint_testing.AccessibleRole.Text, name="File"
        ).all()
        assert file_fields
        assert {
            field.accessible_value for field in file_fields if field.accessible_value
        } == {"assets/checker.svg"}
        wait_until(
            lambda: (
                True
                if not window.get_by_accessible_name("Editor canvas").all()
                else None
            )
        )
        for kind in PALETTE_KINDS:
            assert not palette_row(window, kind).accessible_enabled
        file_row(window, source_file).invoke_accessible_default_action()
        window.get_by_role(
            slint_testing.AccessibleRole.Main, name="Editor canvas"
        ).resolve()
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Fixture text"
        ).resolve()
        file_row(window, assets).invoke_accessible_default_action()
        wait_until(
            lambda: (
                True if not window.get_by_accessible_name(str(image)).all() else None
            )
        )
        snapshot.assert_unchanged()
