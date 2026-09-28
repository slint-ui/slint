# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center, fixture_element
from slint_test import Window
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    select_fixture_element,
)


def begin_inline_edit(window: Window) -> slint_testing.Element:
    select_fixture_element(window, "Text")
    window.get_by_accessible_name("Text move handle").dblclick()
    editor = window.get_by_role("text-input", name="Inline text editor").resolve()
    window.get_by_role("text", name="Fixture text").wait_for(state="hidden")
    return editor


def edited_source(source_file: Path, text: str) -> bytes:
    source = source_file.read_bytes()
    assert source.count(b'        text: "Fixture text";') == 1
    return source.replace(
        b'        text: "Fixture text";',
        f'        text: "{text}";'.encode(),
    )


@pytest.mark.parametrize("commit_key", [keys.Return, keys.Escape])
def test_inline_text_key_commit(
    editor_factory,
    fixture_project: Path,
    commit_key: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = edited_source(source_file, "Edited")

    with editor_factory(source_file) as editor:
        window = editor.window
        begin_inline_edit(window)
        window.keyboard.press_sequentially("Edited")
        window.keyboard.press(commit_key)
        snapshot.wait_for_applied(expected)
        window.get_by_accessible_name("Inline text editor").wait_for(state="hidden")
        window.get_by_role("text", name="Edited").wait_for()


def test_inline_text_focus_commit_selects_clicked_item(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = edited_source(source_file, "Changed")

    with editor_factory(source_file) as editor:
        window = editor.window
        begin_inline_edit(window)
        window.keyboard.press_sequentially("Changed")
        position = center(fixture_element(window, "Rectangle"))
        window.pointer.press_at(position)
        window.pointer.release_at(position)
        snapshot.wait_for_applied(expected)
        window.get_by_role("text", name="Changed").wait_for()
        window.get_by_accessible_name("Selected Rectangle").wait_for()


def test_inline_text_focus_loss_without_change_restores_text(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"

    with editor_factory(source_file) as editor:
        window = editor.window
        begin_inline_edit(window)
        position = center(fixture_element(window, "Rectangle"))
        window.pointer.press_at(position)
        window.pointer.release_at(position)
        window.get_by_role("text", name="Fixture text").wait_for()


@pytest.mark.parametrize(
    "text_declaration",
    [
        b'        text: "Fixture\\ntext";',
        b'        text: "Fixture text";\n        wrap: word-wrap;',
        b'        text: "Fixture text";\n        transform-scale-x: 200%;',
    ],
)
def test_inline_text_rejects_unsupported_layouts(
    editor_factory,
    fixture_project: Path,
    text_declaration: bytes,
) -> None:
    source_file = fixture_project / "Main.slint"
    source = source_file.read_bytes()
    source_file.write_bytes(
        source.replace(b'        text: "Fixture text";', text_declaration)
    )

    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Text")
        window.get_by_accessible_name("Text move handle").dblclick()
        window.get_by_accessible_name("Inline text editor").wait_for(state="hidden")
