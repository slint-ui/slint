# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import slint_testing
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
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
