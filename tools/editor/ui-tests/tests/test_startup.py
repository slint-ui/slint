# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import slint_testing
from ui_driver import (
    first_window,
    launch_editor,
    wait_until,
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
        window.get_by_role(
            slint_testing.AccessibleRole.Main, name="Editor canvas"
        ).resolve()
        window.get_by_role(
            slint_testing.AccessibleRole.Navigation, name="Project and elements"
        ).resolve()
        window.get_by_role(
            slint_testing.AccessibleRole.Complementary, name="Inspector and outline"
        ).resolve()
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Fixture text"
        ).resolve()
        window.get_by_role(
            slint_testing.AccessibleRole.ListItem, name="root-text"
        ).resolve()
        assert not window.get_by_accessible_name("Startup wizard").all()


def test_startup_page_shows_project_actions_without_editor_panes(
    editor_binary: Path,
    editor_environment: dict[str, str],
) -> None:
    with launch_editor(editor_binary, editor_environment) as editor:
        window = first_window(editor)
        window.get_by_role(
            slint_testing.AccessibleRole.Region, name="Startup wizard"
        ).resolve()
        assert not window.get_by_accessible_name("Editor canvas").all()
        assert not window.get_by_accessible_name("Project and elements").all()
        assert not window.get_by_accessible_name("Inspector and outline").all()

        create = window.get_by_role(
            slint_testing.AccessibleRole.Button, name="Create New Project..."
        ).resolve()
        assert create.accessible_enabled
        open_existing = window.get_by_role(
            slint_testing.AccessibleRole.Button, name="Open Existing Project..."
        ).resolve()
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
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Fixture text"
        ).resolve()

        def recent_project_was_saved() -> Path | None:
            settings_files = list(tmp_path.rglob("visual-editor-user-settings.json"))
            if len(settings_files) != 1:
                return None
            contents = settings_files[0].read_text()
            return settings_files[0] if str(fixture_project) in contents else None

        wait_until(recent_project_was_saved)

    with launch_editor(editor_binary, editor_environment) as editor:
        window = first_window(editor)
        recent_row = window.get_by_role(
            slint_testing.AccessibleRole.ListItem, name=fixture_project.name
        ).resolve()
        assert recent_row.accessible_description == str(fixture_project)

        window.get_by_accessible_name("File").click(force=True)
        window.get_by_accessible_name("Open Recent").click(force=True)

        window.get_by_role(
            slint_testing.AccessibleRole.Text, name=fixture_project.name
        ).nth(0).click(force=True)
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Fixture text"
        ).resolve()
