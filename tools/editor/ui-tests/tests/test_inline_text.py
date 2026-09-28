# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center, fixture_element
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    first_window,
    launch_editor,
    press_key,
    press_keys,
    select_fixture_element,
    wait_until,
)
from ui_locators import Window


def begin_inline_edit(window: Window) -> slint_testing.Element:
    select_fixture_element(window, "Text")
    window.get_by_accessible_name("Text move handle").double_click(
        slint_testing.PointerEventButton.Left
    )
    editor = window.get_by_role(
        slint_testing.AccessibleRole.TextInput, name="Inline text editor"
    ).resolve()
    assert not window.get_by_role(
        slint_testing.AccessibleRole.Text, name="Fixture text"
    ).all()
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
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    commit_key: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = edited_source(source_file, "Edited")

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        begin_inline_edit(window)
        press_keys(window, "Edited")
        press_key(window, commit_key)
        snapshot.wait_for_applied(expected)
        assert not window.get_by_accessible_name("Inline text editor").all()
        window.get_by_role(slint_testing.AccessibleRole.Text, name="Edited").resolve()


def test_inline_text_focus_commit_selects_clicked_item(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = edited_source(source_file, "Changed")

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        begin_inline_edit(window)
        press_keys(window, "Changed")
        position = center(fixture_element(window, "Rectangle"))
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(position, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(position, button))
        snapshot.wait_for_applied(expected)
        window.get_by_role(slint_testing.AccessibleRole.Text, name="Changed").resolve()
        wait_until(
            lambda: next(
                iter(window.get_by_accessible_name("Selected Rectangle").all()),
                None,
            )
        )


def test_inline_text_focus_loss_without_change_restores_text(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        begin_inline_edit(window)
        position = center(fixture_element(window, "Rectangle"))
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(position, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(position, button))
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Fixture text"
        ).resolve()


@pytest.mark.parametrize(
    "text_declaration",
    [
        b'        text: "Fixture\\ntext";',
        b'        text: "Fixture text";\n        wrap: word-wrap;',
        b'        text: "Fixture text";\n        transform-scale-x: 200%;',
    ],
)
def test_inline_text_rejects_unsupported_layouts(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    text_declaration: bytes,
) -> None:
    source_file = fixture_project / "Main.slint"
    source = source_file.read_bytes()
    source_file.write_bytes(
        source.replace(b'        text: "Fixture text";', text_declaration)
    )

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Text")
        window.get_by_accessible_name("Text move handle").double_click(
            slint_testing.PointerEventButton.Left
        )
        assert not window.get_by_accessible_name("Inline text editor").all()
