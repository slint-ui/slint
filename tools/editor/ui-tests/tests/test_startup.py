# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import slint_testing
from editor_sync import wait_for_source
from ui_assertions import expect
from ui_driver import (
    element,
    elements,
    first_window,
    launch_editor,
    wait_until,
)


def test_visual_editor_loads_its_own_ui(
    editor_binary: Path,
    editor_environment: dict[str, str],
) -> None:
    source_file = Path(__file__).resolve().parents[2] / "ui" / "main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, source_file.read_bytes())
        expect.poll(
            lambda: any(
                row.accessible_label.strip() == "EditorUi inherits Window"
                for row in elements(window, role=slint_testing.AccessibleRole.ListItem)
            ),
            message="The outline contains the editor's root component",
        ).to_equal(True)
        element(window, "No files available", role=slint_testing.AccessibleRole.Text)


def test_startup_page_shows_project_actions_without_editor_panes(
    editor_binary: Path,
    editor_environment: dict[str, str],
) -> None:
    with launch_editor(editor_binary, editor_environment) as editor:
        window = first_window(editor)
        element(window, "Startup wizard", role=slint_testing.AccessibleRole.Region)
        assert not elements(window, "Editor canvas")
        assert not elements(window, "Project and elements")
        assert not elements(window, "Inspector and outline")

        create = element(
            window, "Create New Project...", role=slint_testing.AccessibleRole.Button
        )
        assert create.accessible_enabled
        open_existing = element(
            window, "Open Existing Project...", role=slint_testing.AccessibleRole.Button
        )
        assert open_existing.accessible_enabled


def test_file_menu_opens_the_same_recent_project_as_the_startup_page(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    editor_environment["SLINT_NO_MUDA"] = "1"

    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        assert window.size.width > 0
        assert window.size.height > 0
        element(window, "Editor canvas", role=slint_testing.AccessibleRole.Main)
        element(
            window, "Project and elements", role=slint_testing.AccessibleRole.Navigation
        )
        element(
            window,
            "Inspector and outline",
            role=slint_testing.AccessibleRole.Complementary,
        )
        element(window, "Fixture text", role=slint_testing.AccessibleRole.Text)
        element(window, "root-text", role=slint_testing.AccessibleRole.ListItem)
        assert not elements(window, "Startup wizard")

        def recent_project_was_saved() -> Path | None:
            settings_directory = Path(editor_environment["HOME"]).parent
            settings_files = list(
                settings_directory.rglob("visual-editor-user-settings.json")
            )
            if len(settings_files) != 1:
                return None
            contents = settings_files[0].read_text()
            return settings_files[0] if str(fixture_project) in contents else None

        wait_until(recent_project_was_saved)

    with launch_editor(editor_binary, editor_environment) as editor:
        window = first_window(editor)
        recent_row = element(
            window, fixture_project.name, role=slint_testing.AccessibleRole.ListItem
        )
        assert recent_row.accessible_description == str(fixture_project)

        element(window, "File").single_click(slint_testing.PointerEventButton.Left)
        open_recent = element(window, "Open Recent")
        open_recent.single_click(slint_testing.PointerEventButton.Left)

        def recent_menu_item() -> slint_testing.Element | None:
            return next(
                (
                    element
                    for element in elements(window, fixture_project.name)
                    if element.accessible_role != slint_testing.AccessibleRole.ListItem
                ),
                None,
            )

        recent_item = wait_until(recent_menu_item)
        recent_item.single_click(slint_testing.PointerEventButton.Left)
        element(window, "Fixture text", role=slint_testing.AccessibleRole.Text)
