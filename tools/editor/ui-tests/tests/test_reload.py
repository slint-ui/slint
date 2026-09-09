# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from editor_sync import current_editor_sync
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


@pytest.mark.parametrize("continuous_messages", [False, True])
def test_external_root_source_reload(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    continuous_messages: bool,
) -> None:
    source_file = fixture_project / "Main.slint"
    if continuous_messages:
        original = source_file.read_text()
        closing_brace = original.rfind("}")
        source_file.write_text(
            original[:closing_brace]
            + '    Timer { interval: 10ms; running: true; triggered => { debug("reload pulse"); } }\n'
            + original[closing_brace:]
        )
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
        sync = current_editor_sync.get()
        checkpoint = sync.checkpoint()
        with sync.gate("source", source_file) as gate:
            source_file.write_bytes(original.replace(b"Fixture text", b"Revision one"))
            source_file.write_bytes(original.replace(b"Fixture text", b"Revision two"))
            expected = original.replace(b"Fixture text", b"Newest revision")
            source_file.write_bytes(expected)
            gate.wait_for_reached()
        snapshot.wait_for_exact(expected)
        sync.wait_for_processed(
            source_file,
            expected,
            after=int(checkpoint["cursor"]),
            outcome="compiled",
        )
        sync.wait_for_source(source_file, expected, after=int(checkpoint["cursor"]))
        window_element_with_label(
            window, "Newest revision", slint_testing.AccessibleRole.Text, timeout=15
        )
        assert source_file.read_bytes() == expected
        assert not elements_with_label(window.root_element, "Revision one")
        assert not elements_with_label(window.root_element, "Revision two")
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
