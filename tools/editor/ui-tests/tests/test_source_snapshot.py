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


def test_source_change_ignores_truncated_write(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    baseline = b"component Main inherits Window {}\n"
    updated = b"component Main inherits Window { width: 40px; }\n"
    readings = iter([baseline, b"", updated])
    monkeypatch.setattr(Path, "read_bytes", lambda _: next(readings))
    monkeypatch.setattr("ui_driver.time.sleep", lambda _: None)
    assert wait_for_source_change(tmp_path / "Main.slint", baseline) == updated
