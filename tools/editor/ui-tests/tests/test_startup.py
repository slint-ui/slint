# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
from pathlib import Path

import pytest
import slint_testing
from ui_driver import (
    element,
    elements,
    file_row,
    first_window,
    launch_editor,
    screenshot,
    wait_until,
)


def project_file(main: str | None = None) -> str:
    contents = {"slint-edition": "1.19"}
    if main is not None:
        contents["main"] = main
    return json.dumps(contents)


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


def test_editor_loads_its_own_project(
    editor_binary: Path,
    editor_environment: dict[str, str],
) -> None:
    manifest = Path(__file__).resolve().parents[2] / "slint-project.json"
    with launch_editor(editor_binary, editor_environment, manifest) as editor:
        window = first_window(editor)
        canvas = element(
            window, "Editor canvas", role=slint_testing.AccessibleRole.Main
        )
        element(
            canvas, "Project and elements", role=slint_testing.AccessibleRole.Navigation
        )


@pytest.mark.parametrize("project_input", ["source", "manifest"])
def test_file_menu_opens_the_same_recent_project_as_the_startup_page(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    project_input: str,
) -> None:
    editor_environment["SLINT_NO_MUDA"] = "1"
    target = fixture_project / "Main.slint"
    if project_input != "source":
        target = fixture_project / "slint-project.json"
        target.write_text(project_file("Main.slint"))

    with launch_editor(editor_binary, editor_environment, target) as editor:
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
            contents = json.loads(settings_files[0].read_text())
            recent = contents.get("recent_projects", [])
            return (
                settings_files[0]
                if recent and recent[0]["preview"]["url"] == target.as_uri()
                else None
            )

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
        file_row(
            window, fixture_project / "Sibling.slint"
        ).invoke_accessible_default_action()
        element(window, "sibling-rectangle", role=slint_testing.AccessibleRole.ListItem)


def test_manifest_without_entry_reports_compilation_error_and_allows_navigation(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tmp_path: Path,
) -> None:
    manifest = fixture_project / "slint-project.json"
    manifest.write_text(project_file())
    with launch_editor(editor_binary, editor_environment, manifest) as editor:
        window = first_window(editor)
        diagnostics = element(
            window, "Preview unavailable", role=slint_testing.AccessibleRole.Region
        )
        wait_until(
            lambda: (
                any(
                    "has no 'main' to compile" in message.accessible_label
                    for message in elements(
                        diagnostics, role=slint_testing.AccessibleRole.Text
                    )
                )
                or None
            )
        )
        file_row(window, fixture_project)
        assert not elements(window, "Startup wizard")
        assert not elements(window, "No component to preview")
        assert not elements(
            window, "Fixture text", role=slint_testing.AccessibleRole.Text
        )
        screenshot(window).save(tmp_path / "missing-entry-preview.png")
        file_row(
            window, fixture_project / "Sibling.slint"
        ).invoke_accessible_default_action()
        element(window, "sibling-rectangle", role=slint_testing.AccessibleRole.ListItem)
        assert editor.process.poll() is None


@pytest.mark.parametrize(
    "broken_input",
    [
        "malformed",
        "malformed-surrounding-manifest",
        "missing-manifest",
        "missing-entry",
        "no-entry",
        "missing-source",
        "missing-root",
    ],
)
def test_broken_project_recovers_when_repaired_on_disk(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    broken_input: str,
) -> None:
    target = fixture_project / "slint-project.json"
    repaired_source = fixture_project / "Repaired.slint"
    if broken_input in ["malformed", "malformed-surrounding-manifest"]:
        target.write_text("{")
        if broken_input == "malformed-surrounding-manifest":
            target = fixture_project / "Main.slint"
    elif broken_input == "missing-entry":
        target.write_text(project_file(repaired_source.name))
    elif broken_input == "no-entry":
        target.write_text(project_file())
    elif broken_input == "missing-source":
        target = fixture_project / "missing.slint"
    elif broken_input == "missing-root":
        target = fixture_project / "unavailable" / "missing.slint"
    with launch_editor(editor_binary, editor_environment, target) as editor:
        window = first_window(editor)
        diagnostics = element(
            window, "Preview unavailable", role=slint_testing.AccessibleRole.Region
        )
        wait_until(
            lambda: (
                any(
                    any(
                        error in message.accessible_label
                        for error in [
                            "EOF while parsing",
                            "has no 'main'",
                            "'slint-edition' is missing",
                            "No component found",
                            "File not registered",
                        ]
                    )
                    for message in elements(
                        diagnostics, role=slint_testing.AccessibleRole.Text
                    )
                )
                or None
            )
        )
        assert not elements(window, "Startup wizard")
        assert not elements(window, "No component to preview")
        source = 'export component Main inherits Window { Text { text: "Repaired project"; } }'
        if target.suffix == ".slint":
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(source)
            if broken_input == "malformed-surrounding-manifest":
                (fixture_project / "slint-project.json").write_text(project_file())
        else:
            repaired_source.write_text(source)
            if broken_input != "missing-entry":
                target.write_text(project_file(repaired_source.name))
        element(window, "Repaired project", role=slint_testing.AccessibleRole.Text)
        assert editor.process.poll() is None


@pytest.mark.parametrize("manifest_contents", ["{", project_file()])
def test_switching_to_a_broken_project_removes_the_previous_preview(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    manifest_contents: str,
    tmp_path: Path,
) -> None:
    editor_environment["SLINT_NO_MUDA"] = "1"
    broken_root = fixture_project / "Broken"
    broken_root.mkdir()
    manifest = broken_root / "slint-project.json"
    manifest.write_text(manifest_contents)
    sibling = broken_root / "Recovery.slint"
    sibling.write_text(
        'export component Main inherits Window { Text { text: "Recovered project"; } }'
    )
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        element(window, "Fixture text", role=slint_testing.AccessibleRole.Text)
        settings_directory = Path(editor_environment["HOME"]).parent
        settings_path = wait_until(
            lambda: next(
                settings_directory.rglob("visual-editor-user-settings.json"), None
            )
        )
    settings = json.loads(settings_path.read_text())
    settings["recent_projects"].append(
        {
            "root": str(broken_root),
            "preview": {"url": manifest.as_uri(), "component": "Main"},
        }
    )
    settings_path.write_text(json.dumps(settings))
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        element(window, "Fixture text", role=slint_testing.AccessibleRole.Text)
        element(window, "File").single_click(slint_testing.PointerEventButton.Left)
        element(window, "Open Recent").single_click(
            slint_testing.PointerEventButton.Left
        )
        recent_item = wait_until(
            lambda: next(
                (
                    item
                    for item in elements(window, "Broken")
                    if item.accessible_role != slint_testing.AccessibleRole.ListItem
                ),
                None,
            )
        )
        recent_item.single_click(slint_testing.PointerEventButton.Left)
        element(window, "Preview unavailable", role=slint_testing.AccessibleRole.Region)
        file_row(window, broken_root)
        screenshot(window).save(tmp_path / "switched-broken-project.png")
        assert not elements(
            window, "Fixture text", role=slint_testing.AccessibleRole.Text
        )
        manifest.write_text(project_file(sibling.name))
        element(window, "Recovered project", role=slint_testing.AccessibleRole.Text)
