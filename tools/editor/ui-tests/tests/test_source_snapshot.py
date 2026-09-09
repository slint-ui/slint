# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
from source_snapshot import SourceSnapshot, exact_source_mismatch


def test_source_snapshot_accepts_exact_edit(fixture_project: Path) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    main_file = fixture_project / "Main.slint"
    expected = main_file.read_bytes().replace(b"#f8fafc", b"#ffffff")

    main_file.write_bytes(expected)

    snapshot.wait_for_exact(expected, timeout=0.1)


def test_source_snapshot_rejects_unexpected_edit(
    fixture_project: Path,
    monkeypatch,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    main_file = fixture_project / "Main.slint"
    original = main_file.read_bytes()
    changed = original.replace(b"#f8fafc", b"#ffffff")
    main_file.write_bytes(changed)

    with pytest.raises(AssertionError):
        snapshot.assert_unchanged_now()

    main_file.write_bytes(original)
    clock = [0.0]
    monkeypatch.setattr("source_snapshot.time.monotonic", lambda: clock[0])

    def observed_write(seconds):
        clock[0] += seconds
        main_file.write_bytes(changed)

    monkeypatch.setattr("source_snapshot.time.sleep", observed_write)
    snapshot.wait_for_exact(changed)
    assert clock[0] == 0.02

    message = exact_source_mismatch(
        {Path("Main.slint"): b"export component Main { width: 2px; }\n"},
        {Path("Main.slint"): b"export component Main { width: 1px; }\n"},
    )
    assert "--- expected/Main.slint" in message
    assert "+++ actual/Main.slint" in message


def test_source_snapshot_observes_unchanged_project(fixture_project: Path) -> None:
    SourceSnapshot.capture(fixture_project).assert_unchanged_now()
