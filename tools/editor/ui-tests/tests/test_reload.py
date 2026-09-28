# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import time
from pathlib import Path

import pytest
import slint_testing
from slint_test import Window
from source_snapshot import SourceSnapshot
from ui_driver import (
    first_window,
    launch_editor,
)


def assert_editor_stable(
    editor: slint_testing.Application,
    window: Window,
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
        window.get_by_role("text", name="Fixture text").wait_for()
        handle, size = window.handle, window.size
        expected = source_file.read_bytes().replace(
            b"Fixture text", b"Reloaded root", 1
        )
        source_file.write_bytes(expected)
        snapshot.wait_for_exact(expected)
        window.get_by_role("text", name="Reloaded root").wait_for(timeout=(15) * 1000)
        window.get_by_accessible_name("Fixture text").wait_for(state="hidden")
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
        window.get_by_role("text", name="Fixture text").wait_for()
        handle, size = window.handle, window.size
        original = source_file.read_bytes()
        source_file.write_bytes(original.replace(b"Fixture text", b"Revision one"))
        source_file.write_bytes(original.replace(b"Fixture text", b"Revision two"))
        expected = original.replace(b"Fixture text", b"Newest revision")
        source_file.write_bytes(expected)
        snapshot.wait_for_exact(expected)
        window.get_by_role("text", name="Newest revision").wait_for(timeout=(15) * 1000)
        deadline = time.monotonic() + 0.25
        while time.monotonic() < deadline:
            assert source_file.read_bytes() == expected
            window.get_by_accessible_name("Revision one").wait_for(state="hidden")
            window.get_by_accessible_name("Revision two").wait_for(state="hidden")
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
        window.get_by_role("text", name="Imported component").wait_for()
        handle, size = window.handle, window.size
        expected = imported_file.read_bytes().replace(
            b"Imported component", b"Reloaded import", 1
        )
        imported_file.write_bytes(expected)
        snapshot.wait_for_exact(expected, "components/Nested.slint")
        window.get_by_role("text", name="Reloaded import").wait_for(timeout=(15) * 1000)
        assert_editor_stable(editor, window, handle, size)
