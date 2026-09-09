# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import threading
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
    fixture_project: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    main_file = fixture_project / "Main.slint"
    original = main_file.read_bytes()
    changed = original.replace(b"#f8fafc", b"#ffffff")
    main_file.write_bytes(changed)

    with pytest.raises(AssertionError):
        snapshot.assert_unchanged(quiescence=0)

    main_file.write_bytes(original)
    delayed_snapshot = SourceSnapshot.capture(fixture_project)
    allow_write = threading.Event()

    def write_after_handshake() -> None:
        allow_write.wait(timeout=5)
        main_file.write_bytes(changed)

    delayed_write = threading.Thread(target=write_after_handshake)
    delayed_write.start()
    original_sleep = __import__("source_snapshot").time.sleep
    sleep_calls = 0

    def release_after_first_observation(seconds: float) -> None:
        nonlocal sleep_calls
        sleep_calls += 1
        if sleep_calls == 1:
            allow_write.set()
        original_sleep(0)

    monkeypatch.setattr("source_snapshot.time.sleep", release_after_first_observation)
    try:
        with pytest.raises(AssertionError):
            delayed_snapshot.assert_unchanged(quiescence=0.2)
    finally:
        allow_write.set()
        delayed_write.join()

    message = exact_source_mismatch(
        {Path("Main.slint"): b"export component Main { width: 2px; }\n"},
        {Path("Main.slint"): b"export component Main { width: 1px; }\n"},
    )
    assert "--- expected/Main.slint" in message
    assert "+++ actual/Main.slint" in message


def test_source_snapshot_observes_unchanged_project(fixture_project: Path) -> None:
    SourceSnapshot.capture(fixture_project).assert_unchanged(quiescence=0.01)
