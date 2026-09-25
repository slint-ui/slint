# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import slint_testing
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    wait_until,
    window_element_with_label,
)


def test_editor_starts_with_valid_fixture(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        assert window.size.width > 0
        assert window.size.height > 0
        window_element_with_label(
            window, "Editor canvas", slint_testing.AccessibleRole.Main
        )
        window_element_with_label(
            window, "Project and elements", slint_testing.AccessibleRole.Navigation
        )
        window_element_with_label(
            window,
            "Inspector and outline",
            slint_testing.AccessibleRole.Complementary,
        )
        window_element_with_label(
            window, "Fixture text", slint_testing.AccessibleRole.Text
        )
        window_element_with_label(
            window, "root-text", slint_testing.AccessibleRole.ListItem
        )
        assert not elements_with_label(window.root_element, "Startup wizard")


def test_startup_page_shows_project_actions_without_editor_panes(
    editor_binary: Path,
    editor_environment: dict[str, str],
) -> None:
    with launch_editor(editor_binary, editor_environment) as editor:
        window = first_window(editor)
        window_element_with_label(
            window, "Startup wizard", slint_testing.AccessibleRole.Region
        )
        assert not elements_with_label(window.root_element, "Editor canvas")
        assert not elements_with_label(window.root_element, "Project and elements")
        assert not elements_with_label(window.root_element, "Inspector and outline")

        create = window_element_with_label(
            window, "Create New Project...", slint_testing.AccessibleRole.Button
        )
        assert create.accessible_enabled
        open_existing = window_element_with_label(
            window, "Open Existing Project...", slint_testing.AccessibleRole.Button
        )
        assert open_existing.accessible_enabled


def test_file_menu_opens_the_same_recent_project_as_the_startup_page(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tmp_path: Path,
) -> None:
    editor_environment["HOME"] = str(tmp_path / "home")
    editor_environment["XDG_CONFIG_HOME"] = str(tmp_path / "config")
    editor_environment["SLINT_NO_MUDA"] = "1"

    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        window_element_with_label(
            window, "Fixture text", slint_testing.AccessibleRole.Text
        )

        def recent_project_was_saved() -> Path | None:
            settings_files = list(tmp_path.rglob("visual-editor-user-settings.json"))
            if len(settings_files) != 1:
                return None
            contents = settings_files[0].read_text()
            return settings_files[0] if str(fixture_project) in contents else None

        wait_until(recent_project_was_saved)

    with launch_editor(editor_binary, editor_environment) as editor:
        window = first_window(editor)
        recent_row = window_element_with_label(
            window,
            fixture_project.name,
            slint_testing.AccessibleRole.ListItem,
        )
        assert recent_row.accessible_description == str(fixture_project)

        window_element_with_label(window, "File").single_click(
            slint_testing.PointerEventButton.Left
        )
        open_recent = window_element_with_label(window, "Open Recent")
        open_recent.single_click(slint_testing.PointerEventButton.Left)

        def recent_menu_item() -> slint_testing.Element | None:
            return next(
                (
                    element
                    for element in elements_with_label(
                        window.root_element, fixture_project.name
                    )
                    if element.accessible_role != slint_testing.AccessibleRole.ListItem
                ),
                None,
            )

        recent_item = wait_until(recent_menu_item)
        recent_item.single_click(slint_testing.PointerEventButton.Left)
        window_element_with_label(
            window, "Fixture text", slint_testing.AccessibleRole.Text
        )
