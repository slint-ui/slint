# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import time
from pathlib import Path

import slint_testing
from source_snapshot import SourceSnapshot
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    window_element_with_label,
)


def assert_editor_stable(
    editor: slint_testing.Application,
    window: slint_testing.Window,
    original_handle: object,
    original_size: slint_testing.PhysicalSize,
) -> None:
    assert editor.process.poll() is None
    assert window.handle == original_handle
    assert window.size == original_size


def test_external_root_source_reload(
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
        expected = source_file.read_bytes().replace(
            b"Fixture text", b"Reloaded root", 1
        )
        source_file.write_bytes(expected)
        snapshot.wait_for_exact(expected)
        window_element_with_label(
            window, "Reloaded root", slint_testing.AccessibleRole.Text, timeout=15
        )
        assert not elements_with_label(window.root_element, "Fixture text")
        assert_editor_stable(editor, window, handle, size)


def test_rapid_root_writes_show_newest_revision(
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
        original = source_file.read_bytes()
        source_file.write_bytes(original.replace(b"Fixture text", b"Revision one"))
        source_file.write_bytes(original.replace(b"Fixture text", b"Revision two"))
        expected = original.replace(b"Fixture text", b"Newest revision")
        source_file.write_bytes(expected)
        snapshot.wait_for_exact(expected)
        window_element_with_label(
            window, "Newest revision", slint_testing.AccessibleRole.Text, timeout=15
        )
        deadline = time.monotonic() + 0.25
        while time.monotonic() < deadline:
            assert source_file.read_bytes() == expected
            assert not elements_with_label(window.root_element, "Revision one")
            assert not elements_with_label(window.root_element, "Revision two")
            time.sleep(0.02)
        assert_editor_stable(editor, window, handle, size)


def test_imported_dependency_reload(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    imported_file = fixture_project / "components" / "Nested.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        window_element_with_label(
            window, "Imported component", slint_testing.AccessibleRole.Text
        )
        handle, size = window.handle, window.size
        expected = imported_file.read_bytes().replace(
            b"Imported component", b"Reloaded import", 1
        )
        imported_file.write_bytes(expected)
        snapshot.wait_for_exact(expected, "components/Nested.slint")
        window_element_with_label(
            window, "Reloaded import", slint_testing.AccessibleRole.Text, timeout=15
        )
        assert_editor_stable(editor, window, handle, size)
