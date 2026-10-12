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
        element(window, "sibling-rectangle", role=slint_testing.AccessibleRole.ListItem)
        assert not elements(window, "root-text")
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
        element(window, "Editor canvas", role=slint_testing.AccessibleRole.Main)
        assert not elements(window, str(image))
        file_row(window, assets).invoke_accessible_default_action()
        file_row(window, image).invoke_accessible_default_action()
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
        file_row(window, source_file).invoke_accessible_default_action()
        element(window, "Editor canvas", role=slint_testing.AccessibleRole.Main)
        element(window, "Fixture text", role=slint_testing.AccessibleRole.Text)
        file_row(window, assets).invoke_accessible_default_action()
        expect(query(window, str(image))).to_be_hidden()
        snapshot.assert_unchanged()
