# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import time
from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import FIELDS
from slint_test import Window, expect
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    file_row,
    first_window,
    launch_editor,
    select_outline_row,
)


def stage_field_text(window: Window, label: str, value: str) -> None:
    field = window.get_by_role(slint_testing.AccessibleRole.TextInput, name=label)
    field.fill(value)
    expect(field).to_have_value(value)


def test_broken_source_preserves_last_valid_preview(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
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
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Fixture text"
        ).resolve()
        source_file.write_bytes(baseline + b"\ninvalid source\n")
        time.sleep(0.25)
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
        expect(file_row(window, main_file)).to_be_selected(timeout=15_000)
        components = fixture_project / "components"
        file_row(window, components).activate()
        file_row(window, nested_file).activate()
        window.get_by_role(
            slint_testing.AccessibleRole.ListItem, name="nested-text"
        ).activate()
        window.get_by_role(
            slint_testing.AccessibleRole.Region, name="Selected Text"
        ).resolve(timeout=(15) * 1000)
        field = window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Text content"
        )
        field.set_accessible_value('"Edited import"')
        expected = nested_baseline.replace(
            b'        text: "Imported component";',
            b'        text: "Edited import";',
            1,
        )
        snapshot.wait_for_exact(expected, relative_path="components/Nested.slint")
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Edited import"
        ).resolve(timeout=(15) * 1000)


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
        field = window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name=FIELDS["x"]
        )
        expect(field).to_have_value("224", timeout=15_000)
        window.keyboard.press(keys.Return)
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
        ).resolve(timeout=(15) * 1000)
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


@pytest.mark.parametrize(
    ("label", "property_name", "original", "updated"),
    [
        (FIELDS["x"], "x", "32", "36"),
        (FIELDS["y"], "y", "32", "40"),
        (FIELDS["width"], "width", "160", "180"),
        (FIELDS["height"], "height", "96", "120"),
    ],
)
def test_stale_revision_commit_is_rejected(
    editor_factory, fixture_project, label, property_name, original, updated
):
    from slint_test import expect

    source = fixture_project / "InspectorCases.slint"
    baseline = source.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source) as editor:
        editor.canvas.element("inspect-rectangle").select()
        field = editor.inspector.field(property_name)
        field.fill("99")
        snapshot.assert_unchanged_now()
        external = baseline.replace(
            f"        {property_name}: {original}px;".encode(),
            f"        {property_name}: {updated}px;".encode(),
            1,
        )
        assert external != baseline
        source.write_bytes(external)
        snapshot.wait_for_exact(external, source.name)
        external_snapshot = SourceSnapshot.capture(fixture_project)
        expect(field).to_have_value(updated, timeout=15000)
        editor.window.keyboard.press("Enter")
        external_snapshot.assert_unchanged()
