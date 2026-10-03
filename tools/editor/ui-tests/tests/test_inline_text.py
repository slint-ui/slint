# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore tobytes

from pathlib import Path
from textwrap import indent

import pytest
import slint_testing
from canvas_interactions import (
    begin_palette_drag,
    center,
    fixture_element,
    hover_fixture_element,
    zoom_canvas,
)
from PIL import Image
from slint_testing import keys
from source_snapshot import SourceSnapshot
from test_palette import canvas_drop_position, release_palette_drag
from test_undo_redo import shortcut
from ui_driver import (
    element,
    elements,
    first_window,
    launch_editor,
    press_key,
    press_keys,
    press_shortcut,
    screenshot,
    select_fixture_element,
    wait_until,
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
    editor = element(
        window, "Inline text editor", role=slint_testing.AccessibleRole.TextInput
    )
    assert editor.accessible_value == "Text"
    return editor, expected


def begin_inline_edit(window: slint_testing.Window) -> slint_testing.Element:
    select_fixture_element(window, "Text")
    element(window, "Text move handle").double_click(
        slint_testing.PointerEventButton.Left
    )
    editor = element(
        window, "Inline text editor", role=slint_testing.AccessibleRole.TextInput
    )
    assert not elements(window, "Fixture text", role=slint_testing.AccessibleRole.Text)
    return editor


def inline_editor_image(window: slint_testing.Window) -> Image.Image:
    editor = element(window, "Inline text editor")
    image = screenshot(window)
    scale = image.width / window.root_element.size.width
    position, size = editor.absolute_position, editor.size
    return image.crop(
        (
            round(position.x * scale),
            round(position.y * scale),
            round((position.x + size.width) * scale),
            round((position.y + size.height) * scale),
        )
    )


def test_unselected_text_double_click_begins_inline_edit(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        target = center(hover_fixture_element(window, "Text"))
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        inline_editor = element(
            window, "Inline text editor", role=slint_testing.AccessibleRole.TextInput
        )
        assert inline_editor.accessible_value == "Fixture text"
        press_key(window, keys.Escape)
        snapshot.assert_unchanged()


def test_unselected_text_click_then_drag_moves_instead_of_editing(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = source_file.read_bytes().replace(
        b"        x: 180px;", b"        x: 210px;", 1
    )

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        target = center(hover_fixture_element(window, "Text"))
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))

        start = center(element(window, "Text move handle"))
        end = slint_testing.LogicalPosition(x=start.x + 30, y=start.y)
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))

        assert not elements(window.root_element, "Inline text editor")
        snapshot.wait_for_applied(expected)


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
        assert not elements(window, "Inline text editor")
        element(window, "Edited", role=slint_testing.AccessibleRole.Text)


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
        element(window, "Changed", role=slint_testing.AccessibleRole.Text)
        wait_until(
            lambda: next(
                iter(elements(window, "Selected Rectangle")),
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
        element(window, "Fixture text", role=slint_testing.AccessibleRole.Text)


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
        assert not elements(window.root_element, "Inline text editor")
        element(window, "Replacement", role=slint_testing.AccessibleRole.Text)


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
        element(window, "Search elements").single_click(
            slint_testing.PointerEventButton.Left
        )
        expected = dropped.replace(b'text: "Text";', b'text: "Focus commit";', 1)
        snapshot.wait_for_applied(expected, source_file.name)
        assert not elements(window.root_element, "Inline text editor")


@pytest.mark.parametrize("percent", [50, 100, 200])
def test_dropped_text_keeps_caret_selection_and_grows_editing_bounds(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    percent: int,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        inline_editor, dropped = drop_palette_text(window, snapshot)
        zoom_canvas(window, percent)
        initial_width = inline_editor.size.width
        draft = "A longer replacement"
        press_keys(window, draft)
        wait_until(
            lambda: True if inline_editor.size.width > initial_width * 3 else None
        )
        press_key(window, keys.RightArrow)
        unselected = inline_editor_image(window)
        assert (
            sum(
                isinstance(pixel, tuple) and max(pixel) < 128
                for pixel in unselected.get_flattened_data()
            )
            > 10
        )
        caret = unselected.crop(
            (unselected.width - 3, 0, unselected.width, unselected.height)
        )
        assert sum(pixel != (248, 250, 252) for pixel in caret.get_flattened_data()) > 3

        press_shortcut(window, keys.Control, "a")
        selected = inline_editor_image(window)
        assert (
            sum(
                a != b
                for a, b in zip(
                    unselected.get_flattened_data(), selected.get_flattened_data()
                )
            )
            > selected.width * selected.height / 4
        )

        target = slint_testing.LogicalPosition(
            x=inline_editor.absolute_position.x + inline_editor.size.width - 1,
            y=inline_editor.absolute_position.y + inline_editor.size.height / 2,
        )
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                target, slint_testing.PointerEventButton.Left
            )
        )
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                target, slint_testing.PointerEventButton.Left
            )
        )
        press_keys(window, "!")
        assert inline_editor.accessible_value == draft + "!"
        press_key(window, keys.Return)
        snapshot.wait_for_applied(
            dropped.replace(b'text: "Text";', f'text: "{draft}!";'.encode(), 1),
            source_file.name,
        )


def test_dropped_text_draft_survives_reload_and_continues_typing(
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
        press_shortcut(window, keys.Control, "a")
        before_reload = inline_editor_image(window)
        external = dropped + b"// External source edit.\n"
        source_file.write_bytes(external)
        snapshot.wait_for_applied(external, source_file.name)
        after_reload = inline_editor_image(window)
        assert after_reload.tobytes() == before_reload.tobytes()
        artboard = element(window, "Artboard")
        assert not elements(artboard, "Text", role=slint_testing.AccessibleRole.Text)
        press_keys(window, "Still editing")
        press_shortcut(window, keys.Control, "a")
        assert inline_editor_image(window).tobytes() != after_reload.tobytes()
        press_key(window, keys.Return)
        snapshot.wait_for_applied(
            external.replace(b'text: "Text";', b'text: "Still editing";', 1),
            source_file.name,
        )
        assert not elements(window.root_element, "Inline text editor")


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
        inline_editor = element(
            window, "Inline text editor", role=slint_testing.AccessibleRole.TextInput
        )
        assert inline_editor.accessible_value == "Stale edit"


def test_fixed_width_text_draft_survives_reload(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    source = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        inline_editor = begin_inline_edit(window)
        original_width = inline_editor.size.width
        press_keys(window, "Fixed width draft")
        assert inline_editor.size.width == original_width
        press_shortcut(window, keys.Control, "a")
        before_reload = inline_editor_image(window)
        external = source + b"// External source edit.\n"
        source_file.write_bytes(external)
        snapshot.wait_for_applied(external)
        assert inline_editor_image(window).tobytes() == before_reload.tobytes()
        assert not elements(
            window.root_element, "Fixture text", role=slint_testing.AccessibleRole.Text
        )
        press_keys(window, "After reload")
        press_key(window, keys.Return)
        snapshot.wait_for_applied(
            external.replace(b'text: "Fixture text";', b'text: "After reload";', 1)
        )


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
        element(window, "Search elements").single_click(
            slint_testing.PointerEventButton.Left
        )
        snapshot.wait_for_applied(dropped, source_file.name)
        assert not elements(window.root_element, "Inline text editor")
        assert elements(
            window.root_element, "Text", role=slint_testing.AccessibleRole.Text
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
        element(window, "Selected Text").single_click(
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
    text_element = """Text {
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
        "        height: 96px;\n" + indent(text_element, "        ") + "    }",
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
        inline_editor = element(
            window, "Inline text editor", role=slint_testing.AccessibleRole.TextInput
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
                element(
                    window,
                    "Project and elements",
                    role=slint_testing.AccessibleRole.Navigation,
                )
            )
            release_palette_drag(window, outside)
        snapshot.assert_unchanged()

        begin_palette_drag(window, "Rectangle", target)
        release_palette_drag(window, target)
        expected = (GOLDENS / "Palette.insert-rectangle.slint").read_bytes()
        snapshot.wait_for_applied(expected, source_file.name)
        assert not elements(window.root_element, "Inline text editor")


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
        element(window, "Text move handle").double_click(
            slint_testing.PointerEventButton.Left
        )
        assert not elements(window, "Inline text editor")
