# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import time
from pathlib import Path

import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import FIELDS
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    file_row,
    first_window,
    launch_editor,
    press_key,
    select_outline_row,
    wait_until,
)
from ui_locators import Window


def stage_field_text(window: Window, label: str, value: str) -> slint_testing.Element:
    field = window.get_by_role(
        slint_testing.AccessibleRole.TextInput, name=label
    ).resolve()
    current_value = field.accessible_value
    field.invoke_accessible_default_action()
    for _ in current_value:
        press_key(window, keys.Delete)
    for character in value:
        press_key(window, character)
    return wait_until(lambda: field if field.accessible_value == value else None)


def test_broken_source_preserves_preview_and_recovers(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Fixture text"
        ).resolve()
        handle, size = window.handle, window.size
        broken = source_file.read_bytes() + b"\nthis is not valid Slint\n"
        source_file.write_bytes(broken)
        snapshot.wait_for_exact(broken)
        time.sleep(0.25)
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Fixture text"
        ).resolve()
        window.get_by_role(
            slint_testing.AccessibleRole.ListItem, name="root-text"
        ).resolve()
        assert editor.process.poll() is None
        assert window.handle == handle
        assert window.size == size
        assert source_file.read_bytes() == broken
        repaired = baseline.replace(b"Fixture text", b"Recovered source", 1)
        source_file.write_bytes(repaired)
        snapshot.wait_for_exact(repaired)
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Recovered source"
        ).resolve()
        assert editor.process.poll() is None


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
        wait_until(
            lambda: (
                current
                if (current := file_row(window, main_file)).accessible_item_selected
                else None
            ),
            timeout=15,
        )
        components = fixture_project / "components"
        file_row(window, components).invoke_accessible_default_action()
        file_row(window, nested_file).invoke_accessible_default_action()
        window.get_by_role(
            slint_testing.AccessibleRole.ListItem, name="nested-text"
        ).invoke_accessible_default_action(timeout=15)
        window.get_by_role(
            slint_testing.AccessibleRole.Region, name="Selected Text"
        ).resolve(timeout=15)
        field = window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Text content"
        ).resolve()
        field.accessible_value = '"Edited import"'
        expected = nested_baseline.replace(
            b'        text: "Imported component";',
            b'        text: "Edited import";',
            1,
        )
        snapshot.wait_for_exact(expected, relative_path="components/Nested.slint")
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Edited import"
        ).resolve(timeout=15)


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
        wait_until(
            lambda: (
                field
                if (
                    field := window.get_by_role(
                        slint_testing.AccessibleRole.TextInput, name=FIELDS["x"]
                    ).resolve()
                ).accessible_value
                == "224"
                else None
            ),
            timeout=15,
        )
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
        wait_until(
            lambda: (
                field
                if (
                    field := window.get_by_role(
                        slint_testing.AccessibleRole.TextInput, name=label
                    ).resolve()
                ).accessible_value
                == updated
                else None
            ),
            timeout=15,
        )
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
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Fixture text"
        ).resolve()
        source_file.unlink()
        SourceSnapshot.capture(fixture_project).assert_unchanged()
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Fixture text"
        ).resolve()
        assert not source_file.exists()
        restored = baseline.replace(b"Fixture text", b"Restored root", 1)
        source_file.write_bytes(restored)
        snapshot.wait_for_applied(restored)
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Restored root"
        ).resolve()
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
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Imported component"
        ).resolve()
        imported_file.unlink()
        time.sleep(0.25)
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Imported component"
        ).resolve()
        restored = baseline.replace(b"Imported component", b"Restored import", 1)
        imported_file.write_bytes(restored)
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Restored import"
        ).resolve(timeout=15)
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
        source_file.write_bytes(repaired)
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Initial source recovered"
        ).resolve()
        assert source_file.read_bytes() == repaired
        assert editor.process.poll() is None
