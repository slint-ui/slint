# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import threading
from pathlib import Path

import pytest
from source_snapshot import (
    SourceSnapshot,
    exact_source_mismatch,
    replace_once,
    wait_for_source_change,
)


def test_source_snapshot_accepts_exact_edit(fixture_project: Path) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    main_file = fixture_project / "Main.slint"
    expected = replace_once(main_file.read_bytes(), b"#f8fafc", b"#ffffff")

    main_file.write_bytes(expected)

    snapshot.wait_for_exact(expected, timeout=0.1)


def test_source_snapshot_rejects_unexpected_edit(fixture_project: Path) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    main_file = fixture_project / "Main.slint"
    original = main_file.read_bytes()
    changed = original.replace(b"#f8fafc", b"#ffffff")
    main_file.write_bytes(changed)

    with pytest.raises(AssertionError):
        snapshot.assert_unchanged(quiescence=0)

    main_file.write_bytes(original)
    delayed_snapshot = SourceSnapshot.capture(fixture_project)
    delayed_write = threading.Timer(0.03, main_file.write_bytes, args=(changed,))
    delayed_write.start()
    try:
        with pytest.raises(AssertionError):
            delayed_snapshot.assert_unchanged(quiescence=0.2)
    finally:
        delayed_write.cancel()
        delayed_write.join()

    message = exact_source_mismatch(
        {Path("Main.slint"): b"export component Main { width: 2px; }\n"},
        {Path("Main.slint"): b"export component Main { width: 1px; }\n"},
    )
    assert "--- expected/Main.slint" in message
    assert "+++ actual/Main.slint" in message


def test_source_snapshot_observes_unchanged_project(fixture_project: Path) -> None:
    SourceSnapshot.capture(fixture_project).assert_unchanged(quiescence=0.01)


@pytest.mark.parametrize("source", [b"missing", b"target target"])
def test_replace_once_rejects_non_unique_target(source: bytes) -> None:
    with pytest.raises(AssertionError):
        replace_once(source, b"target", b"replacement")


@pytest.mark.parametrize(
    "incomplete", [b"", b"export component Main inherits Window {"]
)
def test_source_change_waits_for_applied_revision(
    editor_binary, editor_environment, tmp_path: Path, incomplete: bytes
) -> None:
    from editor_sync import wait_for_source
    from ui_driver import launch_editor

    source = tmp_path / "Main.slint"
    baseline = b"export component Main inherits Window { width: 40px; }\n"
    updated = baseline.replace(b"40px", b"80px")
    source.write_bytes(baseline)
    with launch_editor(editor_binary, editor_environment, source):
        wait_for_source(source, baseline)
        source.write_bytes(incomplete)
        completed_write = threading.Timer(0.2, source.write_bytes, args=(updated,))
        completed_write.start()
        try:
            assert wait_for_source_change(source, baseline) == updated
        finally:
            completed_write.cancel()
            completed_write.join()
