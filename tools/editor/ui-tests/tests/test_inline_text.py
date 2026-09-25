# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path
from textwrap import indent

import pytest
import slint_testing
from canvas_interactions import begin_palette_drag, center, fixture_element
from slint_testing import keys
from source_snapshot import SourceSnapshot
from test_palette import canvas_drop_position, release_palette_drag
from test_undo_redo import shortcut
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    press_key,
    press_keys,
    select_fixture_element,
    wait_until,
    window_element_with_label,
)

GOLDENS = Path(__file__).resolve().parents[1] / "goldens"


def drop_palette_text(
    window: slint_testing.Window,
    snapshot: SourceSnapshot,
    source_name: str = "Palette.slint",
) -> tuple[slint_testing.Element, bytes]:
    target = canvas_drop_position(window)
    begin_palette_drag(window, "Text", target)
    release_palette_drag(window, target)
    expected = (GOLDENS / "Palette.insert-text.slint").read_bytes()
    snapshot.wait_for_applied(expected, source_name)
    editor = window_element_with_label(
        window, "Inline text editor", slint_testing.AccessibleRole.TextInput
    )
    assert editor.accessible_value == "Text"
    return editor, expected


def begin_inline_edit(window: slint_testing.Window) -> slint_testing.Element:
    select_fixture_element(window, "Text")
    window_element_with_label(window, "Text move handle").double_click(
        slint_testing.PointerEventButton.Left
    )
    editor = window_element_with_label(
        window, "Inline text editor", slint_testing.AccessibleRole.TextInput
    )
    assert not elements_with_label(
        window.root_element, "Fixture text", slint_testing.AccessibleRole.Text
    )
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
        assert not elements_with_label(window.root_element, "Inline text editor")
        window_element_with_label(window, "Edited", slint_testing.AccessibleRole.Text)


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
        window_element_with_label(window, "Changed", slint_testing.AccessibleRole.Text)
        wait_until(
            lambda: next(
                iter(elements_with_label(window.root_element, "Selected Rectangle")),
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
        window_element_with_label(
            window, "Fixture text", slint_testing.AccessibleRole.Text
        )


def test_dropped_text_replaces_placeholder_and_commits_with_return(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        _, dropped = drop_palette_text(window, snapshot)
        press_keys(window, "Replacement")
        press_key(window, keys.Return)
        expected = dropped.replace(b'text: "Text";', b'text: "Replacement";', 1)
        snapshot.wait_for_applied(expected, source_file.name)
        assert not elements_with_label(window.root_element, "Inline text editor")
        window_element_with_label(
            window, "Replacement", slint_testing.AccessibleRole.Text
        )


def test_dropped_text_commits_on_focus_loss(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        _, dropped = drop_palette_text(window, snapshot)
        press_keys(window, "Focus commit")
        window_element_with_label(window, "Search elements").single_click(
            slint_testing.PointerEventButton.Left
        )
        expected = dropped.replace(b'text: "Text";', b'text: "Focus commit";', 1)
        snapshot.wait_for_applied(expected, source_file.name)
        assert not elements_with_label(window.root_element, "Inline text editor")


def test_dropped_text_rejected_commit_keeps_editor_open(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        _, dropped = drop_palette_text(window, snapshot)
        press_keys(window, "Stale edit")
        external = dropped.replace(
            b"    background: #f8fafc;\n",
            b"    background: #f8fafc;\n    // External source edit.\n",
            1,
        )
        source_file.write_bytes(external)
        snapshot.wait_for_applied(external, source_file.name)
        press_key(window, keys.Return)
        snapshot.wait_for_applied(external, source_file.name)
        inline_editor = window_element_with_label(
            window, "Inline text editor", slint_testing.AccessibleRole.TextInput
        )
        assert inline_editor.accessible_value == "Stale edit"


def test_unchanged_dropped_text_focus_loss_preserves_placeholder(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        _, dropped = drop_palette_text(window, snapshot)
        window_element_with_label(window, "Search elements").single_click(
            slint_testing.PointerEventButton.Left
        )
        snapshot.wait_for_applied(dropped, source_file.name)
        assert not elements_with_label(window.root_element, "Inline text editor")
        assert elements_with_label(
            window.root_element, "Text", slint_testing.AccessibleRole.Text
        )


def test_dropped_text_undoes_commit_before_insertion(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Palette.slint"
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        _, dropped = drop_palette_text(window, snapshot)
        press_keys(window, "Undo me")
        press_key(window, keys.Return)
        committed = dropped.replace(b'text: "Text";', b'text: "Undo me";', 1)
        snapshot.wait_for_applied(committed, source_file.name)
        window_element_with_label(window, "Selected Text").single_click(
            slint_testing.PointerEventButton.Left
        )
        shortcut(window, redo=False)
        snapshot.wait_for_applied(dropped, source_file.name)
        shortcut(window, redo=False)
        snapshot.wait_for_applied(baseline, source_file.name)


def test_text_dropped_into_layout_opens_editor_after_snapping(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "CanvasCases.slint"
    baseline = source_file.read_text()
    layout = """
    drop-layout := HorizontalLayout {
        x: 32px;
        y: 160px;
        width: 240px;
        height: 96px;
    }
"""
    baseline = baseline[:-2] + layout + "}\n"
    source_file.write_text(baseline)
    element = """Text {
    text: "Text";
}
"""
    dropped = baseline.replace(
        "    drop-layout := HorizontalLayout {\n"
        "        x: 32px;\n"
        "        y: 160px;\n"
        "        width: 240px;\n"
        "        height: 96px;\n"
        "    }",
        "    drop-layout := HorizontalLayout {\n"
        "        x: 32px;\n"
        "        y: 160px;\n"
        "        width: 240px;\n"
        "        height: 96px;\n" + indent(element, "        ") + "    }",
        1,
    ).encode()
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        layout = wait_until(
            lambda: next(
                iter(window.find_elements_by_id("CanvasCases::drop-layout")),
                None,
            )
        )
        target = center(layout)
        begin_palette_drag(window, "Text", target)
        release_palette_drag(window, target)
        snapshot.wait_for_applied(dropped, source_file.name)
        inline_editor = window_element_with_label(
            window, "Inline text editor", slint_testing.AccessibleRole.TextInput
        )
        assert inline_editor.accessible_value == "Text"
        press_keys(window, "Layout text")
        press_key(window, keys.Return)
        committed = dropped.replace(b'text: "Text";', b'text: "Layout text";', 1)
        snapshot.wait_for_applied(committed, source_file.name)


@pytest.mark.parametrize("failure", ["cancel", "outside"])
def test_failed_text_drop_does_not_trigger_later_inline_edit(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    failure: str,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        target = canvas_drop_position(window)
        begin_palette_drag(window, "Text", target)
        if failure == "cancel":
            press_key(window, keys.Escape)
            release_palette_drag(window, target)
        else:
            outside = center(
                window_element_with_label(
                    window,
                    "Project and elements",
                    slint_testing.AccessibleRole.Navigation,
                )
            )
            release_palette_drag(window, outside)
        snapshot.assert_unchanged()

        begin_palette_drag(window, "Rectangle", target)
        release_palette_drag(window, target)
        expected = (GOLDENS / "Palette.insert-rectangle.slint").read_bytes()
        snapshot.wait_for_applied(expected, source_file.name)
        assert not elements_with_label(window.root_element, "Inline text editor")


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
        window_element_with_label(window, "Text move handle").double_click(
            slint_testing.PointerEventButton.Left
        )
        assert not elements_with_label(window.root_element, "Inline text editor")
