# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import sys
from pathlib import Path

import slint_testing
from editor_sync import wait_for_source
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_assertions import expect
from ui_driver import (
    PALETTE_KINDS,
    element,
    elements,
    file_row,
    first_window,
    launch_editor,
    palette_row,
    press_key,
    press_keys,
    press_shortcut,
    query,
    select_outline_row,
)


def test_recompilation_preserves_inspector_focus_after_file_tree_navigation(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        element(window, "Fixture text", role=slint_testing.AccessibleRole.Text)
        file_row(window, source).invoke_accessible_default_action()
        select_outline_row(window, "root-rectangle")
        field = element(
            window, "Position X", role=slint_testing.AccessibleRole.TextInput
        )
        field.invoke_accessible_default_action()
        for _ in field.accessible_value:
            press_key(window, keys.Delete)
        press_keys(window, "99")
        snapshot = SourceSnapshot.capture(fixture_project)
        press_key(window, keys.Return)
        expected = snapshot.sources[Path("Main.slint")].replace(
            b"x: 40px;", b"x: 99px;", 1
        )
        snapshot.wait_for_applied(expected)
        press_key(window, "7")
        expect(field).to_have_value("997")
        press_key(window, keys.Return)
        snapshot.wait_for_applied(expected.replace(b"x: 99px;", b"x: 997px;", 1))
        expect(query(window, "Rename Main.slint")).to_be_hidden()


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
        element(
            window, "Rename Main.slint", role=slint_testing.AccessibleRole.TextInput
        )

        press_key(window, keys.Backspace)
        press_keys(window, "Renamed")
        press_key(window, keys.Return)

        expect.poll(
            lambda: target.is_file() and not source.exists(),
            message="renamed file exists and original is gone",
        ).to_equal(True)
        assert target.read_text() == expected


def test_file_tree_renames_file_from_context_menu(
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
        file_row(window, source).single_click(slint_testing.PointerEventButton.Right)
        element(window, "Rename").single_click(slint_testing.PointerEventButton.Left)
        element(
            window, "Rename Main.slint", role=slint_testing.AccessibleRole.TextInput
        )

        press_key(window, keys.Backspace)
        press_keys(window, "Renamed")
        press_key(window, keys.Return)

        expect.poll(
            lambda: target.is_file() and not source.exists(),
            message="renamed file exists and original is gone",
        ).to_equal(True)
        assert target.read_text() == expected


def test_file_tree_copies_path_from_context_menu(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        file_row(window, source).single_click(slint_testing.PointerEventButton.Right)
        element(window, "Copy Path").single_click(slint_testing.PointerEventButton.Left)

        file_row(window, source).single_click(slint_testing.PointerEventButton.Right)
        element(window, "Rename").single_click(slint_testing.PointerEventButton.Left)
        field = element(
            window, "Rename Main.slint", role=slint_testing.AccessibleRole.TextInput
        )
        press_shortcut(window, keys.Control, "a")
        press_shortcut(window, keys.Control, "v")
        expect(field).to_have_value(str(source))

        press_key(window, keys.Escape)
        expect(query(window, "Rename Main.slint")).to_be_hidden()
        assert source.is_file()


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
        element(
            window, "Rename Main.slint", role=slint_testing.AccessibleRole.TextInput
        )

        press_key(window, keys.Backspace)
        press_keys(window, "Renamed")
        file_row(
            window, fixture_project / "Sibling.slint"
        ).invoke_accessible_default_action()

        expect.poll(
            lambda: target.is_file() and not source.exists(),
            message="renamed file exists and original is gone",
        ).to_equal(True)
        expect(query(window, "Rename Main.slint")).to_be_hidden()
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
        element(
            window, "Rename Main.slint", role=slint_testing.AccessibleRole.TextInput
        )

        press_key(window, keys.Backspace)
        press_keys(window, "Sibling")
        press_key(window, keys.Return)

        expect(query(window, "A file with that name already exists")).to_be_visible()


def test_project_files_switch_assets_and_component_surfaces(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Main.slint"
    image = fixture_project / "assets" / "checker.svg"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        element(window, "Editor canvas", role=slint_testing.AccessibleRole.Main)
        assert not elements(window, str(image))
        element(window, "Assets", role=slint_testing.AccessibleRole.Tab).single_click(
            slint_testing.PointerEventButton.Left
        )
        file_row(window, image).single_click(slint_testing.PointerEventButton.Left)
        image_editor = element(
            window, "Image asset editor", role=slint_testing.AccessibleRole.Main
        )
        assert image_editor.accessible_description == "assets/checker.svg"
        element(window, "Preview", role=slint_testing.AccessibleRole.Button)
        file_fields = elements(
            image_editor, "File", role=slint_testing.AccessibleRole.Text
        )
        assert file_fields
        assert {
            field.accessible_value for field in file_fields if field.accessible_value
        } == {"assets/checker.svg"}
        expect(query(window, "Editor canvas")).to_be_hidden()
        for kind in PALETTE_KINDS:
            assert not palette_row(window, kind).accessible_enabled
        element(
            window, "Slint files", role=slint_testing.AccessibleRole.Tab
        ).single_click(slint_testing.PointerEventButton.Left)
        file_row(window, fixture_project / "Sibling.slint").single_click(
            slint_testing.PointerEventButton.Left
        )
        element(window, "sibling-rectangle", role=slint_testing.AccessibleRole.ListItem)
        expect(query(window, "root-text")).to_be_hidden()
        file_row(window, source_file).single_click(
            slint_testing.PointerEventButton.Left
        )
        element(window, "Editor canvas", role=slint_testing.AccessibleRole.Main)
        element(window, "Fixture text", role=slint_testing.AccessibleRole.Text)
        expect(query(window, str(image))).to_be_hidden()
        snapshot.assert_unchanged()


def test_project_files_filter_tabs_and_search(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    image = fixture_project / "assets" / "checker.svg"
    fonts = fixture_project / "assets" / "fonts"
    fonts.mkdir()
    font = fonts / "Example.OTF"
    font.write_bytes(b"")
    unrelated = fonts / "README.md"
    unrelated.write_text("Font notes")
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        element(window, "Fixture text", role=slint_testing.AccessibleRole.Text)
        file_row(window, source)
        expect(query(window, str(image))).to_be_hidden()
        expect(query(window, str(font))).to_be_hidden()
        expect(query(window, str(unrelated))).to_be_hidden()
        expect(query(window, str(fonts))).to_be_hidden()

        slint_files = element(
            window, "Slint files", role=slint_testing.AccessibleRole.Tab
        )
        assets = element(window, "Assets", role=slint_testing.AccessibleRole.Tab)
        slint_files.single_click(slint_testing.PointerEventButton.Left)
        press_key(window, keys.RightArrow)
        expect(assets).to_be_selected()
        press_key(window, keys.RightArrow)
        expect(slint_files).to_be_selected()
        press_key(window, keys.LeftArrow)
        expect(assets).to_be_selected()
        expect(query(window, str(source))).to_be_hidden()
        file_row(window, image)
        file_row(window, font).single_click(slint_testing.PointerEventButton.Left)
        expect.poll(lambda: file_row(window, font).accessible_item_selected).to_equal(
            True
        )
        element(window, "Editor canvas", role=slint_testing.AccessibleRole.Main)
        expect(query(window, str(unrelated))).to_be_hidden()

        search = element(
            window, "Search project files", role=slint_testing.AccessibleRole.TextInput
        )
        search.accessible_value = "FoNtS"
        file_row(window, font)
        expect(query(window, str(image))).to_be_hidden()
        search.accessible_value = "no-such-file"
        files = element(window, "Files", role=slint_testing.AccessibleRole.Tree)
        expect(query(files, "No Results")).to_be_visible()
        search.accessible_value = ""
        file_row(window, image)
        snapshot.assert_unchanged()


def test_project_file_groups_collapse_independently_and_search_hidden_files(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    group = fixture_project / "ui" / "components"
    group.mkdir(parents=True)
    child = group / "Nested.slint"
    child.write_text("export component Nested inherits Rectangle {}")
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        file_row(window, child)
        file_row(window, group).single_click(slint_testing.PointerEventButton.Left)
        expect(query(window, str(child))).to_be_hidden()
        file_row(window, fixture_project).single_click(
            slint_testing.PointerEventButton.Left
        )
        expect(query(window, str(source))).to_be_hidden()

        search = element(
            window, "Search project files", role=slint_testing.AccessibleRole.TextInput
        )
        search.single_click(slint_testing.PointerEventButton.Left)
        press_keys(window, "nEsTeD")
        file_row(window, child)
        expect(query(window, str(source))).to_be_hidden()
        for _ in "nEsTeD":
            press_key(window, keys.Backspace)
        expect(search).to_have_value("")
        press_keys(window, "missing")
        expect(search).to_have_value("missing")
        search.accessible_value = ""
        expect(query(window, str(child))).to_be_hidden()
        file_row(window, group).single_click(slint_testing.PointerEventButton.Left)
        file_row(window, child)
        expect(query(window, str(source))).to_be_hidden()
        snapshot.assert_unchanged()
