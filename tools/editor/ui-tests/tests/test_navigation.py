# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import slint_testing
from source_snapshot import SourceSnapshot
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    wait_until,
    window_element_with_label,
)


def file_row(window: slint_testing.Window, path: Path) -> slint_testing.Element:
    return window_element_with_label(
        window, str(path), slint_testing.AccessibleRole.ListItem
    )


def test_file_tree_opens_sibling_component(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        file_row(
            window, fixture_project / "Sibling.slint"
        ).invoke_accessible_default_action()
        window_element_with_label(
            window, "sibling-rectangle", slint_testing.AccessibleRole.ListItem
        )
        assert not elements_with_label(window.root_element, "root-text")
        snapshot.assert_unchanged()


def test_file_tree_folder_expand_and_collapse(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    assets = fixture_project / "assets"
    image = assets / "checker.svg"
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        folder = file_row(window, assets)
        assert not elements_with_label(window.root_element, str(image))
        folder.invoke_accessible_default_action()
        file_row(window, image)
        file_row(window, assets).invoke_accessible_default_action()
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, str(image))
                else None
            )
        )
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
        window = first_window(editor)
        window_element_with_label(
            window, "Editor canvas", slint_testing.AccessibleRole.Main
        )
        file_row(window, assets).invoke_accessible_default_action()
        file_row(window, image).invoke_accessible_default_action()
        image_editor = window_element_with_label(
            window, "Image asset editor", slint_testing.AccessibleRole.Main
        )
        assert image_editor.accessible_description == "assets/checker.svg"
        window_element_with_label(
            window, "Preview", slint_testing.AccessibleRole.Button
        )
        file_fields = elements_with_label(
            image_editor, "File", slint_testing.AccessibleRole.Text
        )
        assert file_fields
        assert {
            field.accessible_value for field in file_fields if field.accessible_value
        } == {"assets/checker.svg"}
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "Editor canvas")
                else None
            )
        )
        assert not window_element_with_label(
            window, "Rectangle", slint_testing.AccessibleRole.ListItem
        ).accessible_enabled
        file_row(window, source_file).invoke_accessible_default_action()
        window_element_with_label(
            window, "Editor canvas", slint_testing.AccessibleRole.Main
        )
        window_element_with_label(
            window, "Fixture text", slint_testing.AccessibleRole.Text
        )
        snapshot.assert_unchanged()
