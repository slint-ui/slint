# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import FIELDS
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_assertions import expect
from ui_driver import (
    element,
    file_row,
    first_window,
    launch_editor,
    press_key,
    query,
    select_outline_row,
)


def stage_field_text(
    window: slint_testing.Window, label: str, value: str
) -> slint_testing.Element:
    field = element(window, label, role=slint_testing.AccessibleRole.TextInput)
    current_value = field.accessible_value
    field.invoke_accessible_default_action()
    for _ in current_value:
        press_key(window, keys.Delete)
    for character in value:
        press_key(window, character)
    expect(field).to_have_value(value)
    return field


def test_imported_file_edit_targets_only_nested_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    main_file = fixture_project / "Main.slint"
    nested_file = fixture_project / "components" / "Nested.slint"
    nested_baseline = nested_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, main_file) as editor:
        wait_for_source(main_file, main_file.read_bytes())
        window = first_window(editor)
        expect(file_row(window, main_file)).to_be_selected(timeout=15)
        components = fixture_project / "components"
        file_row(window, components).invoke_accessible_default_action()
        file_row(window, nested_file).invoke_accessible_default_action()
        element(
            window,
            "nested-text",
            role=slint_testing.AccessibleRole.ListItem,
            timeout=15,
        ).invoke_accessible_default_action()
        element(
            window,
            "Selected Text",
            role=slint_testing.AccessibleRole.Region,
            timeout=15,
        )
        field = element(
            window, "Text content", role=slint_testing.AccessibleRole.TextInput
        )
        field.accessible_value = '"Edited import"'
        expected = nested_baseline.replace(
            b'        text: "Imported component";',
            b'        text: "Edited import";',
            1,
        )
        snapshot.wait_for_exact(expected, relative_path="components/Nested.slint")
        element(
            window, "Edited import", role=slint_testing.AccessibleRole.Text, timeout=15
        )


def test_stale_selection_commit_is_rejected(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "InspectorCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_outline_row(window, "inspect-rectangle")
        stage_field_text(window, FIELDS["x"], "99")
        snapshot.assert_unchanged_now()
        select_outline_row(window, "inspect-text")
        expect(
            query(window, FIELDS["x"], role=slint_testing.AccessibleRole.TextInput)
        ).to_have_value("224", timeout=15)
        press_key(window, keys.Return)
        snapshot.assert_unchanged()


def test_stale_revision_commit_is_rejected(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    label, property_name, original, updated = FIELDS["x"], "x", "32", "36"
    source_file = fixture_project / "InspectorCases.slint"
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_outline_row(window, "inspect-rectangle")
        stage_field_text(window, label, "99")
        snapshot.assert_unchanged_now()
        external = baseline.replace(
            f"        {property_name}: {original}px;".encode(),
            f"        {property_name}: {updated}px;".encode(),
            1,
        )
        assert external != baseline
        source_file.write_bytes(external)
        snapshot.wait_for_exact(external, relative_path="InspectorCases.slint")
        snapshot = SourceSnapshot.capture(fixture_project)
        expect(
            query(window, label, role=slint_testing.AccessibleRole.TextInput)
        ).to_have_value(updated, timeout=15)
        press_key(window, keys.Return)
        snapshot.assert_unchanged()


def test_deleted_root_file_recovers_without_relaunch(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, baseline)
        element(window, "Fixture text", role=slint_testing.AccessibleRole.Text)
        source_file.unlink()
        element(window, "Stale preview", role=slint_testing.AccessibleRole.Region)
        SourceSnapshot.capture(fixture_project).assert_unchanged()
        element(window, "Fixture text", role=slint_testing.AccessibleRole.Text)
        assert not source_file.exists()
        restored = baseline.replace(b"Fixture text", b"Restored root", 1)
        source_file.write_bytes(restored)
        snapshot.wait_for_applied(restored)
        element(window, "Restored root", role=slint_testing.AccessibleRole.Text)
        assert editor.process.poll() is None


def test_deleted_import_recovers_without_relaunch(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    imported_file = fixture_project / "components" / "Nested.slint"
    baseline = imported_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        element(window, "Imported component", role=slint_testing.AccessibleRole.Text)
        imported_file.unlink()
        element(window, "Stale preview", role=slint_testing.AccessibleRole.Region)
        element(window, "Imported component", role=slint_testing.AccessibleRole.Text)
        restored = baseline.replace(b"Imported component", b"Restored import", 1)
        imported_file.write_bytes(restored)
        element(
            window,
            "Restored import",
            role=slint_testing.AccessibleRole.Text,
            timeout=15,
        )
        assert imported_file.read_bytes() == restored
        assert editor.process.poll() is None


def test_initial_broken_source_recovers_without_relaunch(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "InitiallyBroken.slint"
    source_file.write_bytes(
        b"export component InitiallyBroken inherits Window { broken }\n"
    )
    repaired = (
        b"export component InitiallyBroken inherits Window {\n"
        b"    width: 320px;\n"
        b"    height: 240px;\n"
        b'    Text { text: "Initial source recovered"; }\n'
        b"}\n"
    )
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        assert editor.process.poll() is None
        element(window, "Preview unavailable", role=slint_testing.AccessibleRole.Region)
        source_file.write_bytes(repaired)
        element(
            window, "Initial source recovered", role=slint_testing.AccessibleRole.Text
        )
        assert source_file.read_bytes() == repaired
        assert editor.process.poll() is None
        expect(query(window, "Preview unavailable")).to_be_hidden()
        expect.poll(
            lambda: file_row(window, source_file).accessible_description
        ).to_equal("")
