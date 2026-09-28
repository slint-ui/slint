# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import slint_testing
from ui_driver import (
    first_window,
    launch_editor,
    wait_until,
)


def test_startup_page_shows_project_actions_without_editor_panes(
    editor_binary: Path,
    editor_environment: dict[str, str],
) -> None:
    with launch_editor(editor_binary, editor_environment) as editor:
        window = first_window(editor)
        window.get_by_role(
            slint_testing.AccessibleRole.Region, name="Startup wizard"
        ).wait_for()
        assert not window.get_by_accessible_name("Editor canvas").all()
        assert not window.get_by_accessible_name("Project and elements").all()
        assert not window.get_by_accessible_name("Inspector and outline").all()

        create = window.get_by_role(
            slint_testing.AccessibleRole.Button, name="Create New Project..."
        )
        assert create.accessible_enabled
        open_existing = window.get_by_role(
            slint_testing.AccessibleRole.Button, name="Open Existing Project..."
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
        assert window.size.width > 0
        assert window.size.height > 0
        window.get_by_role(
            slint_testing.AccessibleRole.Main, name="Editor canvas"
        ).wait_for()
        window.get_by_role(
            slint_testing.AccessibleRole.Navigation, name="Project and elements"
        ).wait_for()
        window.get_by_role(
            slint_testing.AccessibleRole.Complementary, name="Inspector and outline"
        ).wait_for()
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Fixture text"
        ).wait_for()
        window.get_by_role(
            slint_testing.AccessibleRole.ListItem, name="root-text"
        ).wait_for()
        assert not window.get_by_accessible_name("Startup wizard").all()

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
        )
        assert recent_row.accessible_description == str(fixture_project)

        window.get_by_accessible_name("File").single_click(
            slint_testing.PointerEventButton.Left
        )
        open_recent = window.get_by_accessible_name("Open Recent")
        open_recent.single_click(slint_testing.PointerEventButton.Left)

        def recent_menu_item() -> slint_testing.Element | None:
            return next(
                (
                    element
                    for element in window.get_by_accessible_name(
                        fixture_project.name
                    ).all()
                    if element.accessible_role != slint_testing.AccessibleRole.ListItem
                ),
                None,
            )

        recent_item = wait_until(recent_menu_item)
        recent_item.single_click(slint_testing.PointerEventButton.Left)
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Fixture text"
        ).wait_for()
