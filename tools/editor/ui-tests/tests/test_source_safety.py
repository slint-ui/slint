# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import time
from pathlib import Path

import pytest
import slint_testing
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    first_window,
    launch_editor,
    select_outline_row,
    wait_until,
    window_element_with_label,
)


def press_key(window: slint_testing.Window, key: str) -> None:
    window.dispatch_event(slint_testing.KeyPressedEvent(text=key))
    window.dispatch_event(slint_testing.KeyReleasedEvent(text=key))


def stage_field_text(
    window: slint_testing.Window, label: str, value: str
) -> slint_testing.Element:
    field = window_element_with_label(
        window, label, slint_testing.AccessibleRole.TextInput
    )
    current_value = field.accessible_value
    field.invoke_accessible_default_action()
    for _ in current_value:
        press_key(window, keys.Delete)
    for character in value:
        press_key(window, character)
    return wait_until(lambda: field if field.accessible_value == value else None)


def test_broken_source_preserves_last_valid_preview(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        window_element_with_label(
            window, "Fixture text", slint_testing.AccessibleRole.Text
        )
        handle, size = window.handle, window.size
        broken = source_file.read_bytes() + b"\nthis is not valid Slint\n"
        source_file.write_bytes(broken)
        snapshot.wait_for_exact(broken)
        time.sleep(0.25)
        window_element_with_label(
            window, "Fixture text", slint_testing.AccessibleRole.Text
        )
        window_element_with_label(
            window, "root-text", slint_testing.AccessibleRole.ListItem
        )
        assert editor.process.poll() is None
        assert window.handle == handle
        assert window.size == size
        assert source_file.read_bytes() == broken


def test_repaired_source_recovers_preview(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        window_element_with_label(
            window, "Fixture text", slint_testing.AccessibleRole.Text
        )
        source_file.write_bytes(baseline + b"\ninvalid source\n")
        time.sleep(0.25)
        repaired = baseline.replace(b"Fixture text", b"Recovered source", 1)
        source_file.write_bytes(repaired)
        snapshot.wait_for_exact(repaired)
        window_element_with_label(
            window, "Recovered source", slint_testing.AccessibleRole.Text
        )
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
        window = first_window(editor)
        main_row = window_element_with_label(
            window, str(main_file), slint_testing.AccessibleRole.ListItem
        )
        wait_until(
            lambda: current if (current := main_row).accessible_item_selected else None,
            timeout=15,
        )
        components = fixture_project / "components"
        window_element_with_label(
            window, str(components), slint_testing.AccessibleRole.ListItem
        ).invoke_accessible_default_action()
        window_element_with_label(
            window, str(nested_file), slint_testing.AccessibleRole.ListItem
        ).invoke_accessible_default_action()
        window_element_with_label(
            window, "nested-text", slint_testing.AccessibleRole.ListItem, timeout=15
        ).invoke_accessible_default_action()
        window_element_with_label(
            window, "Selected Text", slint_testing.AccessibleRole.Region, timeout=15
        )
        field = window_element_with_label(
            window, "Text content", slint_testing.AccessibleRole.TextInput
        )
        field.accessible_value = '"Edited import"'
        expected = nested_baseline.replace(
            b'        text: "Imported component";',
            b'        text: "Edited import";',
            1,
        )
        snapshot.wait_for_exact(expected, relative_path="components/Nested.slint")
        window_element_with_label(
            window, "Edited import", slint_testing.AccessibleRole.Text, timeout=15
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
        stage_field_text(window, "Position X", "99")
        snapshot.assert_unchanged_now()
        select_outline_row(window, "inspect-text")
        wait_until(
            lambda: (
                field
                if (
                    field := window_element_with_label(
                        window, "Position X", slint_testing.AccessibleRole.TextInput
                    )
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
    source_file = fixture_project / "InspectorCases.slint"
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_outline_row(window, "inspect-rectangle")
        stage_field_text(window, "Position X", "99")
        snapshot.assert_unchanged_now()
        external = baseline.replace(b"        x: 32px;", b"        x: 36px;", 1)
        source_file.write_bytes(external)
        snapshot.wait_for_exact(external, relative_path="InspectorCases.slint")
        wait_until(
            lambda: (
                field
                if (
                    field := window_element_with_label(
                        window, "Position X", slint_testing.AccessibleRole.TextInput
                    )
                ).accessible_value
                == "36"
                else None
            ),
            timeout=15,
        )
        press_key(window, keys.Return)
        SourceSnapshot.capture(fixture_project).assert_unchanged()


@pytest.mark.skip(reason="Requires a Rust source-watcher recovery fix")
def test_deleted_root_file_recovers_without_relaunch(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        window_element_with_label(
            window, "Fixture text", slint_testing.AccessibleRole.Text
        )
        source_file.unlink()
        time.sleep(0.25)
        window_element_with_label(
            window, "Fixture text", slint_testing.AccessibleRole.Text
        )
        assert not source_file.exists()
        restored = baseline.replace(b"Fixture text", b"Restored root", 1)
        source_file.write_bytes(restored)
        window_element_with_label(
            window, "Restored root", slint_testing.AccessibleRole.Text
        )
        assert source_file.read_bytes() == restored
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
        window_element_with_label(
            window, "Imported component", slint_testing.AccessibleRole.Text
        )
        imported_file.unlink()
        time.sleep(0.25)
        window_element_with_label(
            window, "Imported component", slint_testing.AccessibleRole.Text
        )
        restored = baseline.replace(b"Imported component", b"Restored import", 1)
        imported_file.write_bytes(restored)
        window_element_with_label(
            window, "Restored import", slint_testing.AccessibleRole.Text, timeout=15
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
        source_file.write_bytes(repaired)
        window_element_with_label(
            window, "Initial source recovered", slint_testing.AccessibleRole.Text
        )
        assert source_file.read_bytes() == repaired
        assert editor.process.poll() is None
