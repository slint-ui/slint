# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import (
    center,
    fixture_element,
    hover_rectangle,
    wait_for_no_rectangle_hover,
)
from editor_sync import wait_for_source
from inspector_interactions import FIELDS
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_assertions import expect
from ui_driver import (
    element,
    elements,
    first_window,
    launch_editor,
    press_key,
    press_shortcut,
    query,
    select_fixture_element,
    wait_until,
)

GOLDENS = Path(__file__).resolve().parents[1] / "goldens"
FOLLOWING_ELEMENT = {
    "Rectangle": b"root-text",
    "Text": b"root-image",
    "Image": b"NestedCard",
}


def deletion_golden(element_type: str) -> bytes:
    expected = (GOLDENS / f"Main.delete-{element_type.lower()}.slint").read_bytes()
    following = FOLLOWING_ELEMENT[element_type]
    return expected.replace(
        b"\n\n    " + following,
        b"\n    \n\n    " + following,
        1,
    )


def test_canvas_selection_synchronizes_outline_and_inspector(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        text = wait_until(
            lambda: next(iter(elements(window, id="Main::root-text")), None)
        )
        target = slint_testing.LogicalPosition(
            x=text.absolute_position.x + text.size.width / 2,
            y=text.absolute_position.y + text.size.height / 2,
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        row = element(window, "root-text", role=slint_testing.AccessibleRole.ListItem)
        expect(row).to_be_selected()
        element(window, "Selected Text", role=slint_testing.AccessibleRole.Region)
        expect(
            query(window, FIELDS["x"], role=slint_testing.AccessibleRole.TextInput)
        ).to_have_value("180")
        snapshot.assert_unchanged()


@pytest.mark.parametrize("side", ["left", "right"])
def test_canvas_hovers_and_selects_element_outside_root(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    side: str,
) -> None:
    source = fixture_project / "BoundsCases.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        canvas = element(window, "Editor canvas")
        artboard = element(window, "Artboard", role=slint_testing.AccessibleRole.Region)
        artboard_left = artboard.absolute_position.x
        artboard_right = artboard_left + artboard.size.width
        canvas_left = canvas.absolute_position.x
        canvas_right = canvas_left + canvas.size.width
        if side == "left":
            assert artboard_left - canvas_left > 120
            center_x = (canvas_left + artboard_left) / 2
        else:
            assert canvas_right - artboard_right > 120
            center_x = (artboard_right + canvas_right) / 2
        expected = (
            source.read_text()
            .replace("x: 96px;", f"x: {center_x - 60 - artboard_left}px;", 1)
            .encode()
        )
        source.write_bytes(expected)
        wait_for_source(source, expected)
        rectangle = element(window, id="BoundsCases::bounds-rectangle")
        target = center(rectangle)
        assert target.x < artboard_left or target.x > artboard_right

        window.dispatch_event(slint_testing.PointerMoveEvent(target))
        element(window, "Hovered Rectangle", role=slint_testing.AccessibleRole.Region)

        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        row = element(
            window, "bounds-rectangle", role=slint_testing.AccessibleRole.ListItem
        )
        expect(row).to_be_selected()
        element(window, "Selected Rectangle", role=slint_testing.AccessibleRole.Region)


def test_clear_canvas_selection_does_not_edit_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, source.read_bytes())
        button = slint_testing.PointerEventButton.Left
        for outside in (False, True):
            select_fixture_element(window, "Rectangle")
            for label, value in ((FIELDS["x"], "40"), (FIELDS["width"], "180")):
                assert (
                    element(
                        window, label, role=slint_testing.AccessibleRole.TextInput
                    ).accessible_value
                    == value
                )
            snapshot.assert_unchanged()
            if outside:
                element(window, "Rectangle background")
                canvas = element(window, "Editor canvas")
                target = slint_testing.LogicalPosition(
                    x=canvas.absolute_position.x + 10, y=canvas.absolute_position.y + 10
                )
                window.dispatch_event(slint_testing.PointerMoveEvent(target))
            else:
                artboard = element(
                    window, "Artboard", role=slint_testing.AccessibleRole.Region
                )
                target = slint_testing.LogicalPosition(
                    x=artboard.absolute_position.x + artboard.size.width - 12,
                    y=artboard.absolute_position.y + artboard.size.height - 12,
                )
            window.dispatch_event(slint_testing.PointerPressEvent(target, button))
            window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))

            def selection_cleared() -> bool:
                outline = element(
                    window,
                    "Current file outline",
                    role=slint_testing.AccessibleRole.List,
                )
                rows = (
                    outline.query_descendants()
                    .match_accessible_role(slint_testing.AccessibleRole.ListItem)
                    .find_all()
                )
                return (
                    bool(rows)
                    and not any(row.accessible_item_selected for row in rows)
                    and not any(
                        elements(window, label)
                        for label in (
                            "Selected Rectangle",
                            "Rectangle background",
                            "Root background",
                        )
                    )
                )

            wait_until(selection_cleared, timeout=15)
            snapshot.assert_unchanged()
        select_fixture_element(window, "Rectangle")
        outline = element(window, "Current file outline")
        root_row = (
            outline.query_descendants()
            .match_accessible_role(slint_testing.AccessibleRole.ListItem)
            .find_all()[0]
        )
        root_row.invoke_accessible_default_action()
        element(window, "Root background")
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "key", [keys.Delete, keys.Backspace], ids=["delete", "backspace"]
)
def test_delete_without_element_selection_does_not_edit_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    key: str,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        element(window, "Editor canvas", role=slint_testing.AccessibleRole.Main)
        press_key(window, key)
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "key", [keys.Backspace, keys.Delete], ids=["backspace", "delete"]
)
def test_focused_inspector_field_consumes_delete_key(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    key: str,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        field = element(
            window, FIELDS["x"], role=slint_testing.AccessibleRole.TextInput
        )
        target = slint_testing.LogicalPosition(
            x=field.absolute_position.x + field.size.width / 2,
            y=field.absolute_position.y + field.size.height / 2,
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        press_key(window, key)
        element(window, "Selected Rectangle", role=slint_testing.AccessibleRole.Region)
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "key", [keys.Delete, keys.Backspace], ids=["delete", "backspace"]
)
@pytest.mark.parametrize("element_type", ["Rectangle", "Text", "Image"])
def test_delete_selected_element_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    key: str,
    element_type: str,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, source_file.read_bytes())
        select_fixture_element(window, element_type)
        if element_type == "Rectangle":
            hover_rectangle(window)
        press_key(window, key)
        expected = deletion_golden(element_type)
        snapshot.wait_for_applied(expected)
        wait_until(
            lambda: (
                not elements(
                    window,
                    f"root-{element_type.lower()}",
                    role=slint_testing.AccessibleRole.ListItem,
                )
            ),
            timeout=15,
        )
        assert not elements(
            window, f"Selected {element_type}", role=slint_testing.AccessibleRole.Region
        )
        if element_type == "Rectangle":
            assert not window.find_elements_by_id("Main::root-rectangle")
            wait_for_no_rectangle_hover(window)
            if key == keys.Delete:
                window.dispatch_event(
                    slint_testing.PointerMoveEvent(
                        center(fixture_element(window, "Image"))
                    )
                )
                element(window, "Hovered Image")
                wait_for_no_rectangle_hover(window)


def element_block(source: bytes, start: bytes) -> bytes:
    begin = source.index(start)
    return source[begin : source.index(b"\n    }\n", begin) + len(b"\n    }")]


def test_copy_paste_selected_element_inserts_copy_with_unique_id(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Main.slint"
    source = source_file.read_bytes()
    rectangle = element_block(source, b"root-rectangle := ")
    expected = source.replace(
        rectangle,
        rectangle
        + b"\n    "
        + rectangle.replace(b"root-rectangle", b"root-rectangle-2", 1),
        1,
    )
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, source)
        select_fixture_element(window, "Rectangle")
        press_shortcut(window, keys.Control, "c")
        press_shortcut(window, keys.Control, "v")
        snapshot.wait_for_applied(expected)
        element(window, "root-rectangle-2", role=slint_testing.AccessibleRole.ListItem)


def test_cut_paste_moves_element_after_selection(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Main.slint"
    source = source_file.read_bytes()
    rectangle = element_block(source, b"root-rectangle := ")
    after_cut = deletion_golden("Rectangle")
    image = element_block(after_cut, b"root-image := ")
    expected = after_cut.replace(image, image + b"\n    " + rectangle, 1)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, source)
        select_fixture_element(window, "Rectangle")
        press_shortcut(window, keys.Control, "x")
        snapshot.wait_for_applied(after_cut)
        select_fixture_element(window, "Image")
        press_shortcut(window, keys.Control, "v")
        snapshot.wait_for_applied(expected)


def test_paste_with_empty_clipboard_does_not_edit_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, source_file.read_bytes())
        select_fixture_element(window, "Text")
        press_shortcut(window, keys.Control, "v")
        element(window, "Selected Text", role=slint_testing.AccessibleRole.Region)
        snapshot.assert_unchanged()


def test_paste_inserts_element_at_pointer(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Main.slint"
    source = source_file.read_bytes()
    rectangle = element_block(source, b"root-rectangle := ")
    copy = (
        rectangle.replace(b"root-rectangle", b"root-rectangle-2", 1)
        .replace(b"x: 40px;", b"x: 300px;", 1)
        .replace(b"y: 40px;", b"y: 420px;", 1)
    )
    nested = element_block(source, b"NestedCard {")
    expected = source.replace(nested, nested + b"\n    " + copy, 1)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, source)
        select_fixture_element(window, "Rectangle")
        press_shortcut(window, keys.Control, "c")
        origin = fixture_element(window, "Rectangle").absolute_position
        window.dispatch_event(
            slint_testing.PointerMoveEvent(
                slint_testing.LogicalPosition(x=origin.x + 260, y=origin.y + 380)
            )
        )
        press_shortcut(window, keys.Control, "v")
        snapshot.wait_for_applied(expected)


def test_paste_over_selected_container_inserts_child_at_pointer(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Main.slint"
    source = source_file.read_bytes()
    rectangle = element_block(source, b"root-rectangle := ")
    child = (
        rectangle.replace(b"root-rectangle", b"root-rectangle-2", 1)
        .replace(b"x: 40px;", b"x: 90px;", 1)
        .replace(b"y: 40px;", b"y: 60px;", 1)
        .replace(b"\n", b"\n    ")
    )
    expected = source.replace(
        rectangle, rectangle[: -len(b"}")] + b"    " + child + b"\n    }", 1
    )
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, source)
        select_fixture_element(window, "Rectangle")
        press_shortcut(window, keys.Control, "c")
        hover_rectangle(window)
        press_shortcut(window, keys.Control, "v")
        snapshot.wait_for_applied(expected)
