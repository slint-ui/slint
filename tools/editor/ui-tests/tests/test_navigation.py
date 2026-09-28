# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import sys
from pathlib import Path

import slint_testing
from editor_sync import wait_for_source
from slint_test import expect
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    PALETTE_KINDS,
    elements_with_label,
    file_row,
    first_window,
    launch_editor,
    palette_row,
)


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
        file_row(window, source).activate()
        window.keyboard.press(keys.Return if sys.platform == "darwin" else keys.F2)
        window.get_by_role("text-input", name="Rename Main.slint").wait_for()

        window.keyboard.press(keys.Backspace)
        window.keyboard.press_sequentially("Renamed")
        file_row(window, fixture_project / "Sibling.slint").activate()

        expect.poll(
            lambda: target.is_file() and not source.exists(),
            session=window.session,
            message="renamed file exists and original is gone",
        ).to_equal(True)
        expect(window.get_by_accessible_name("Rename Main.slint")).to_be_hidden()
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
        file_row(window, source).activate()
        window.keyboard.press(keys.Return if sys.platform == "darwin" else keys.F2)
        window.get_by_role("text-input", name="Rename Main.slint").wait_for()

        window.keyboard.press(keys.Backspace)
        window.keyboard.press_sequentially("Sibling")
        window.keyboard.press(keys.Return)

        expect(
            window.get_by_accessible_name("A file with that name already exists")
        ).to_be_visible()


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
        file_row(window, fixture_project / "Sibling.slint").activate()
        window.get_by_role("list-item", name="sibling-rectangle").wait_for()
        window.get_by_accessible_name("root-text").wait_for(state="hidden")
        snapshot.assert_unchanged()


def test_file_tree_folder_expand_and_collapse(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    assets = fixture_project / "assets"
    image = assets / "checker.svg"
    source = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        folder = file_row(window, assets)
        window.get_by_accessible_name(str(image)).wait_for(state="hidden")
        folder.activate()
        file_row(window, image)
        file_row(window, assets).activate()
        expect(window.get_by_accessible_name(str(image))).to_be_hidden()
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
        window.get_by_role("main", name="Editor canvas").wait_for()
        file_row(window, assets).activate()
        file_row(window, image).activate()
        image_editor = window.get_by_role("main", name="Image asset editor").resolve()
        assert image_editor.accessible_description == "assets/checker.svg"
        window.get_by_role("button", name="Preview").wait_for()
        file_fields = elements_with_label(
            image_editor, "File", slint_testing.AccessibleRole.Text
        )
        assert file_fields
        assert {
            field.accessible_value for field in file_fields if field.accessible_value
        } == {"assets/checker.svg"}
        expect(window.get_by_accessible_name("Editor canvas")).to_be_hidden()
        for kind in PALETTE_KINDS:
            assert not palette_row(window, kind).read(
                lambda element: element.accessible_enabled
            )
        file_row(window, source_file).activate()
        window.get_by_role("main", name="Editor canvas").wait_for()
        window.get_by_role("text", name="Fixture text").wait_for()
        snapshot.assert_unchanged()


def test_file_tree_renames_file_inline(editor_factory, fixture_project):
    source = fixture_project / "Main.slint"
    target = fixture_project / "Renamed.slint"
    expected = source.read_text()
    with editor_factory(source) as editor:
        editor.files.rename("Main.slint", "Renamed")
        expect.poll(
            lambda: target.is_file() and not source.exists(),
            session=editor.window.session,
        ).to_equal(True)
        assert target.read_text() == expected
