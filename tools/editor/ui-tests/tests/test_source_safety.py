# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import time
from pathlib import Path

import pytest
import slint_testing
from source_oracle import SourceSnapshot
from ui_driver import first_window, launch_editor, window_element_with_label


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


@pytest.mark.skip(
    reason="The Text content inspector field is added by the inspector layer"
)
def test_imported_file_edit_targets_only_nested_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    # The stacked inspector branch replaces this skip with the real UI interaction.
    assert editor_binary
    assert editor_environment
    assert fixture_project


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
