# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore getbbox getextrema getpixel tobytes

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import wait_for_source
from PIL import Image, ImageChops
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    outline_row,
    outline_rows,
    press_key,
    press_shortcut,
    screenshot,
    select_outline_row,
    wait_until,
    window_element_with_label,
)

GOLDENS = Path(__file__).resolve().parents[1] / "goldens"


def known_outline_state(
    window: slint_testing.Window,
) -> list[tuple[str, str, bool]]:
    labels = {"container", "child-a", "child-b", "sibling-a", "sibling-b"}
    return [
        (
            row.accessible_label,
            row.accessible_description,
            row.accessible_item_selected,
        )
        for row in outline_rows(window)
        if row.accessible_label in labels
    ]


def wait_for_outline_state(
    window: slint_testing.Window,
    expected: list[tuple[str, str, bool]],
) -> None:
    wait_until(
        lambda: (
            current if (current := known_outline_state(window)) == expected else None
        ),
        timeout=15,
    )


def drop_position(
    window: slint_testing.Window, target: str, location: str
) -> slint_testing.LogicalPosition:
    if target == "<outline-root>":
        return center(
            window_element_with_label(
                window,
                "Outline root drop target",
                slint_testing.AccessibleRole.ListItem,
            )
        )
    row = (
        outline_rows(window)[0]
        if target == "<component-root>"
        else outline_row(window, target)
    )
    fraction = {"before": 1 / 6, "onto": 1 / 2, "after": 5 / 6}[location]
    return slint_testing.LogicalPosition(
        x=row.absolute_position.x + row.size.width / 2,
        y=row.absolute_position.y + row.size.height * fraction,
    )


def drag_row(
    window: slint_testing.Window,
    source: str,
    target: str,
    location: str,
) -> None:
    source_row = (
        outline_rows(window)[0] if source == "<root>" else outline_row(window, source)
    )
    window.drag_and_drop(center(source_row), drop_position(window, target, location))


@pytest.mark.parametrize(
    "source,target,location,golden",
    [
        (
            "sibling-b",
            "sibling-a",
            "before",
            "OutlineCases.reorder.sibling-b-before-a.slint",
        ),
        (
            "sibling-a",
            "sibling-b",
            "after",
            "OutlineCases.reorder.sibling-a-after-b.slint",
        ),
    ],
    ids=["before", "after"],
)
def test_outline_reorders_siblings_with_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    source: str,
    target: str,
    location: str,
    golden: str,
) -> None:
    source_file = fixture_project / "OutlineCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        drag_row(window, source, target, location)
        snapshot.wait_for_exact((GOLDENS / golden).read_bytes(), "OutlineCases.slint")
        wait_for_outline_state(
            window,
            [
                ("container", "Hierarchy level 2", False),
                ("child-a", "Hierarchy level 3", False),
                ("child-b", "Hierarchy level 3", False),
                ("sibling-b", "Hierarchy level 2", False),
                ("sibling-a", "Hierarchy level 2", False),
            ],
        )


@pytest.mark.parametrize(
    "source,target,golden",
    [
        (
            "sibling-a",
            "container",
            "OutlineCases.reparent.sibling-a.slint",
        ),
        (
            "child-a",
            "<outline-root>",
            "OutlineCases.reparent.child-a-root.slint",
        ),
    ],
    ids=["child", "root"],
)
def test_outline_changes_element_parent_with_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    source: str,
    target: str,
    golden: str,
) -> None:
    source_file = fixture_project / "OutlineCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        drag_row(window, source, target, "onto")
        snapshot.wait_for_exact((GOLDENS / golden).read_bytes(), "OutlineCases.slint")
        expected = (
            [
                ("container", "Hierarchy level 2", False),
                ("child-a", "Hierarchy level 3", False),
                ("child-b", "Hierarchy level 3", False),
                ("sibling-a", "Hierarchy level 3", False),
                ("sibling-b", "Hierarchy level 2", False),
            ]
            if source == "sibling-a"
            else [
                ("container", "Hierarchy level 2", False),
                ("child-b", "Hierarchy level 3", False),
                ("sibling-a", "Hierarchy level 2", False),
                ("sibling-b", "Hierarchy level 2", False),
                ("child-a", "Hierarchy level 2", False),
            ]
        )
        wait_for_outline_state(window, expected)


def test_outline_disclosure_collapses_and_expands_without_source_edit(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "OutlineCases.slint"
    ) as editor:
        window = first_window(editor)
        outline_row(window, "container").invoke_accessible_expand_action()
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "child-a")
                else None
            )
        )
        outline_row(window, "container").invoke_accessible_expand_action()
        outline_row(window, "child-a")
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    ("key", "initial", "target", "selection"),
    [
        (keys.Return, "child-a", "child-b", "Selected Text"),
        (keys.Space, "sibling-a", "sibling-b", "Selected Image"),
    ],
    ids=["return", "space"],
)
def test_outline_keyboard_selection_synchronizes_editor(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    key: str,
    initial: str,
    target: str,
    selection: str,
) -> None:
    source_file = fixture_project / "OutlineCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, source_file.read_bytes())
        select_outline_row(window, initial)
        row = outline_row(window, target)
        assert not row.accessible_item_selected
        press_key(window, keys.Tab)
        press_key(window, key)
        wait_until(lambda: row if row.accessible_item_selected else None)
        window_element_with_label(
            window,
            selection,
            slint_testing.AccessibleRole.Region,
        )
        press_shortcut(window, keys.Shift, keys.Tab)
        press_key(window, key)
        wait_until(
            lambda: outline_row(window, initial).accessible_item_selected or None
        )
        window_element_with_label(
            window, "Selected Rectangle", slint_testing.AccessibleRole.Region
        )
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "source,target",
    [
        ("sibling-a", "sibling-a"),
        ("container", "child-a"),
        ("<root>", "child-a"),
    ],
    ids=["self", "cycle", "root"],
)
def test_illegal_outline_drops_do_not_change_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    source: str,
    target: str,
) -> None:
    source_file = fixture_project / "OutlineCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        wait_for_source(source_file, source_file.read_bytes())
        drag_row(window, source, target, "onto")
        snapshot.assert_unchanged()


def test_escape_cancels_outline_drag_without_source_edit(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "OutlineCases.slint"
    ) as editor:
        window = first_window(editor)
        source = fixture_project / "OutlineCases.slint"
        wait_for_source(source, source.read_bytes())
        start = center(outline_row(window, "sibling-a"))
        end = drop_position(window, "container", "onto")
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        window_element_with_label(
            window, "Outline drag preview", slint_testing.AccessibleRole.Region
        )
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Escape))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Escape))
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "Outline drag preview")
                else None
            )
        )
        assert not elements_with_label(window.root_element, "Outline insertion preview")
        snapshot.assert_unchanged()


def test_prohibited_layout_outline_drop_does_not_change_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    canvas_file = fixture_project / "CanvasCases.slint"
    with launch_editor(editor_binary, editor_environment, canvas_file) as editor:
        window = first_window(editor)
        drag_row(window, "prohibited-layout", "<component-root>", "before")
        snapshot.assert_unchanged()


@pytest.mark.parametrize("grab_fraction", [0.25, 0.75])
def test_outline_ghost_follows_pointer_in_tree(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tmp_path: Path,
    grab_fraction: float,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "OutlineCases.slint"
    ) as editor:
        window = first_window(editor)
        source = fixture_project / "OutlineCases.slint"
        wait_for_source(source, source.read_bytes())
        row = outline_row(window, "sibling-a")
        row_width, row_height = row.size.width, row.size.height
        grab = slint_testing.LogicalPosition(
            x=row.size.width * grab_fraction, y=row.size.height * grab_fraction
        )
        start = slint_testing.LogicalPosition(
            x=row.absolute_position.x + grab.x, y=row.absolute_position.y + grab.y
        )
        end = drop_position(window, "container", "onto")
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        ghost = window_element_with_label(window, "Outline drag preview")
        moved = slint_testing.LogicalPosition(x=end.x + 20, y=end.y)
        window.dispatch_event(slint_testing.PointerMoveEvent(moved))
        ghost = window_element_with_label(window, "Outline drag preview")
        assert ghost.absolute_position.x == pytest.approx(moved.x - grab.x)
        assert ghost.absolute_position.y == pytest.approx(moved.y - grab.y)
        assert ghost.size.width == pytest.approx(row_width)
        assert ghost.size.height == pytest.approx(row_height)
        (tmp_path / "outline-ghost.png").write_bytes(window.grab_window_as_png())
        press_key(window, keys.Escape)
        window.dispatch_event(slint_testing.PointerReleaseEvent(moved, button))
        assert outline_row(window, "sibling-a").size.height == pytest.approx(row_height)
        snapshot.assert_unchanged()


def outline_image(
    window: slint_testing.Window,
    row: slint_testing.Element,
    image: Image.Image | None = None,
) -> Image.Image:
    assert row.is_valid
    image = screenshot(window) if image is None else image
    scale = image.width / window.root_element.size.width
    x, y = row.absolute_position.x, row.absolute_position.y
    return image.crop(
        (
            round(x * scale),
            round(y * scale),
            round((x + row.size.width) * scale),
            round((y + row.size.height) * scale),
        )
    ).resize((round(row.size.width), round(row.size.height)))


@pytest.mark.parametrize("selected", [False, True], ids=["hovered", "selected"])
def test_outline_ghost_preserves_row_highlight_and_fades(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    selected: bool,
) -> None:
    source = fixture_project / "OutlineCases.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, source.read_bytes())
        row = outline_row(window, "sibling-b")
        if selected:
            select_outline_row(window, "sibling-b")
        start = center(row)
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        original = outline_image(window, row)
        sample = (original.width - 40, original.height // 2)
        highlight = original.getpixel(sample)
        blank = outline_image(window, outline_row(window, "sibling-a")).getpixel(sample)
        assert isinstance(highlight, tuple)
        assert isinstance(blank, tuple)
        assert highlight != blank
        end = slint_testing.LogicalPosition(x=start.x, y=start.y - row.size.height * 8)
        before = screenshot(window)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        window_element_with_label(window, "Outline drag preview")
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        ghost = window_element_with_label(window, "Outline drag preview")
        assert ghost.absolute_position.y == pytest.approx(end.y - row.size.height / 2)
        faded = outline_image(window, ghost)
        underneath = outline_image(window, ghost, before)
        underlying_pixel = underneath.getpixel(sample)
        assert isinstance(underlying_pixel, tuple)
        expected = tuple(
            round((a + b) / 2) for a, b in zip(highlight, underlying_pixel)
        )
        assert faded.getpixel(sample) == pytest.approx(expected, abs=2)
        # The faded icon and text retain the original shape and position.
        content = (40, 6, original.width - 40, original.height - 6)
        expected_content = Image.blend(original, underneath, 0.5).crop(content)
        difference = ImageChops.difference(faded.crop(content), expected_content)
        assert difference.point(lambda value: 255 if value > 3 else 0).getbbox() is None
        press_key(window, keys.Escape)
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        assert (
            outline_image(window, outline_row(window, "sibling-b")).tobytes()
            == original.tobytes()
        )


def test_outline_click_does_not_draw_focus_ring(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "OutlineCases.slint"
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, source.read_bytes())
        row = outline_row(window, "sibling-b")
        position = center(row)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(position, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(position, button))
        wait_until(lambda: row.accessible_item_selected or None)
        rendered = outline_image(window, row)
        background = rendered.getpixel((rendered.width - 40, rendered.height // 2))
        for y in range(6, rendered.height - 10):
            assert rendered.getpixel((rendered.width - 2, y)) == background
