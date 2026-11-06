# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import begin_palette_drag, center, zoom_canvas
from editor_sync import wait_for_source
from slint_testing import keys
from source_snapshot import SourceSnapshot, wait_for_source_change
from ui_assertions import expect
from ui_driver import (
    PALETTE_KINDS,
    PRIMITIVE_GROUPS,
    PRIMITIVE_KINDS,
    element,
    elements,
    first_window,
    launch_editor,
    press_key,
    press_keys,
    press_shortcut,
    query,
    select_fixture_element,
    wait_until,
)

GOLDENS = Path(__file__).resolve().parents[1] / "goldens"
TOUCHAREA_PREVIEW_SIZE = (160, 96)


def release_palette_drag(
    window: slint_testing.Window, target: slint_testing.LogicalPosition
) -> None:
    window.dispatch_event(slint_testing.PointerMoveEvent(target))
    window.dispatch_event(
        slint_testing.PointerReleaseEvent(target, slint_testing.PointerEventButton.Left)
    )


def canvas_drop_position(
    window: slint_testing.Window, scale: float = 1
) -> slint_testing.LogicalPosition:
    artboard = element(window, "Artboard", role=slint_testing.AccessibleRole.Region)
    return slint_testing.LogicalPosition(
        x=artboard.absolute_position.x + 195 * scale,
        y=artboard.absolute_position.y + 360 * scale,
    )


@pytest.mark.parametrize(
    "kind", [kind for kind in PRIMITIVE_KINDS if kind not in PALETTE_KINDS]
)
def test_new_primitives_insert_and_undo(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    baseline = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        target = canvas_drop_position(window)
        begin_palette_drag(window, kind, target)
        release_palette_drag(window, target)
        inserted = wait_for_source_change(source, baseline)
        assert f"{kind} {{".encode() in inserted
        snapshot.wait_for_applied(inserted, source.name)
        element(window, f"Selected {kind}", role=slint_testing.AccessibleRole.Region)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, source.name)


@pytest.mark.parametrize("percent", [50, 100, 200])
@pytest.mark.parametrize("kind", PALETTE_KINDS)
def test_insert_palette_element_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    percent: int,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        zoom_canvas(window, percent)
        target = canvas_drop_position(window, percent / 100)
        text_preview_size = None
        snapshot.assert_unchanged_now()
        begin_palette_drag(window, kind, target)
        if kind in ("StyledText", "Text"):
            preview = element(
                window, f"{kind} drag preview", role=slint_testing.AccessibleRole.Region
            )
            assert preview.size.width < 100
            assert preview.size.height < 40
            scale = percent / 100
            assert preview.absolute_position.x + preview.size.width * scale / 2 == (
                pytest.approx(target.x, abs=1)
            )
            assert preview.absolute_position.y + preview.size.height * scale / 2 == (
                pytest.approx(target.y, abs=1)
            )
            text_preview_size = preview.size
        release_palette_drag(window, target)
        expected = (GOLDENS / f"Palette.insert-{kind.lower()}.slint").read_bytes()
        snapshot.wait_for_exact(expected, "Palette.slint")
        if kind == "Text":
            inline_editor = element(
                window,
                "Inline text editor",
                role=slint_testing.AccessibleRole.TextInput,
            )
            assert inline_editor.accessible_value == "Text"
            press_key(window, keys.Escape)
        selected = element(
            window, f"Selected {kind}", role=slint_testing.AccessibleRole.Region
        )
        if text_preview_size is not None:
            assert selected.size.width == pytest.approx(
                text_preview_size.width * scale, abs=1
            )
            assert selected.size.height == pytest.approx(
                text_preview_size.height * scale, abs=1
            )
        outline = element(
            window, "Current file outline", role=slint_testing.AccessibleRole.List
        )
        inserted = wait_until(
            lambda: next(
                (
                    row
                    for row in outline.query_descendants()
                    .match_accessible_role(slint_testing.AccessibleRole.ListItem)
                    .find_all()
                    if row.accessible_label.strip() == kind
                ),
                None,
            )
        )
        expect.poll(
            lambda: inserted.accessible_item_selected,
            message=f"inserted {kind} outline row is selected",
        ).to_equal(True)


def test_text_drag_preview_uses_preview_default_font_size(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Palette.slint"
    source = source_file.read_bytes().replace(
        b"    background: #f8fafc;",
        b"    background: #f8fafc;\n    default-font-size: 24px;",
    )
    source_file.write_bytes(source)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source)
        window = first_window(editor)
        target = canvas_drop_position(window)
        begin_palette_drag(window, "Text", target)
        preview = element(
            window, "Text drag preview", role=slint_testing.AccessibleRole.Region
        )
        preview_size = preview.size
        assert preview_size.height >= 24
        release_palette_drag(window, target)
        element(
            window, "Inline text editor", role=slint_testing.AccessibleRole.TextInput
        )
        press_key(window, keys.Escape)
        selected = element(
            window, "Selected Text", role=slint_testing.AccessibleRole.Region
        )
        assert selected.size.width == pytest.approx(preview_size.width, abs=1)
        assert selected.size.height == pytest.approx(preview_size.height, abs=1)


def test_insert_textedit_outside_layout_sets_default_size(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        target = canvas_drop_position(window)
        begin_palette_drag(window, "TextEdit", target)
        release_palette_drag(window, target)
        expected = (GOLDENS / "Palette.insert-textedit.slint").read_bytes()
        snapshot.wait_for_exact(expected, "Palette.slint")
        selected = element(
            window, "Selected TextEdit", role=slint_testing.AccessibleRole.Region
        )
        expect.poll(
            lambda: (selected.size.width, selected.size.height),
            message="inserted TextEdit size",
        ).to_equal((200, 100))


@pytest.mark.parametrize("kind", PALETTE_KINDS)
def test_palette_drop_outside_canvas_does_not_edit_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        outside = center(
            element(
                window,
                "Project and elements",
                role=slint_testing.AccessibleRole.Navigation,
            )
        )
        begin_palette_drag(window, kind, outside)
        snapshot.assert_unchanged_now()
        release_palette_drag(window, outside)
        snapshot.assert_unchanged()


@pytest.mark.parametrize("kind", (*PRIMITIVE_KINDS, "Button", "CheckBox"))
def test_escape_cancels_palette_drag_without_source_edit(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "Palette.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        target = canvas_drop_position(window)
        begin_palette_drag(window, kind, target)
        element(
            window, f"{kind} drag preview", role=slint_testing.AccessibleRole.Region
        )
        press_key(window, keys.Escape)
        release_palette_drag(window, target)
        assert not elements(window, f"{kind} drag preview")
        snapshot.assert_unchanged()


def library_rows(window: slint_testing.Window) -> list[slint_testing.Element]:
    pane = element(window, "Element library", role=slint_testing.AccessibleRole.List)
    rows = (
        pane.query_descendants()
        .match_accessible_role(slint_testing.AccessibleRole.ListItem)
        .find_all()
    )
    return sorted(
        rows,
        key=lambda row: (row.absolute_position.y, row.absolute_position.x),
    )


def expect_library(window: slint_testing.Window, labels: list[str]) -> None:
    def all_labels() -> list[str]:
        pane = element(
            window, "Element library", role=slint_testing.AccessibleRole.List
        )
        position = center(pane)
        found = []
        previous = None
        for delta in [10000] + [-max(1, pane.size.height / 2)] * 32:
            window.dispatch_event(
                slint_testing.PointerScrolledEvent(position, delta_x=0, delta_y=delta)
            )
            rows = library_rows(window)
            current = [(row.accessible_label, row.absolute_position.y) for row in rows]
            for label, _ in current:
                if label not in found:
                    found.append(label)
            if current == previous:
                break
            previous = current
        window.dispatch_event(
            slint_testing.PointerScrolledEvent(position, delta_x=0, delta_y=10000)
        )
        return found

    expect.poll(
        all_labels,
        message="element library rows across the scroll area",
    ).to_equal(labels)


def test_tabs_separate_sources_and_preserve_primitive_collapse_state(
    editor_binary, editor_environment, fixture_project
):
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Palette.slint"
    ) as editor:
        window = first_window(editor)
        expect_library(window, list(PRIMITIVE_KINDS))
        element(
            window, "Visual", role=slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        expect_library(window, list(PRIMITIVE_KINDS[4:]))
        element(
            window, "Libraries", role=slint_testing.AccessibleRole.Tab
        ).invoke_accessible_default_action()
        element(window, "std-widgets", role=slint_testing.AccessibleRole.Button)
        assert not elements(window, "Material")
        labels = [row.accessible_label for row in library_rows(window)]
        assert "Button" in labels and "CheckBox" in labels
        assert not set(PRIMITIVE_KINDS) & set(labels)
        search = element(window, "Search elements")
        search.accessible_value = "  bUtToN  "
        expect_library(window, ["Button", "StandardButton"])
        element(
            window, "Primitives", role=slint_testing.AccessibleRole.Tab
        ).invoke_accessible_default_action()
        expect_library(window, [])
        element(window, "No Results")
        search.accessible_value = ""
        expect_library(window, list(PRIMITIVE_KINDS[4:]))
        element(
            window, "Visual", role=slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        expect_library(window, list(PRIMITIVE_KINDS))
        snapshot.assert_unchanged()


@pytest.mark.parametrize("label", ["Visual", "Input & interaction", "std-widgets"])
def test_category_focus_ring_requires_keyboard_navigation(
    editor_binary, editor_environment, fixture_project, label
):
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Palette.slint"
    ) as editor:
        window = first_window(editor)
        if label == "std-widgets":
            element(
                window, "Libraries", role=slint_testing.AccessibleRole.Tab
            ).invoke_accessible_default_action()
        header = element(window, label, role=slint_testing.AccessibleRole.Button)
        ring = query(header, id="ElementPaletteGroup::category-focus-ring")
        header.single_click(slint_testing.PointerEventButton.Left)
        expect(ring).to_be_hidden()
        press_key(window, keys.Space)
        expect(ring).to_be_hidden()
        press_key(window, keys.Tab)
        press_shortcut(window, keys.Shift, keys.Tab)
        expect(ring).to_be_visible()
        press_key(window, keys.Space)
        expect(ring).to_be_visible()
        header.single_click(slint_testing.PointerEventButton.Left)
        expect(ring).to_be_hidden()
        snapshot.assert_unchanged()


def test_element_tabs_support_arrow_home_and_end_navigation(
    editor_binary, editor_environment, fixture_project
):
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Palette.slint"
    ) as editor:
        window = first_window(editor)
        element(
            window, "Primitives", role=slint_testing.AccessibleRole.Tab
        ).single_click(slint_testing.PointerEventButton.Left)
        for key, label in [
            (keys.RightArrow, "Libraries"),
            (keys.RightArrow, "Project"),
            (keys.RightArrow, "Primitives"),
            (keys.LeftArrow, "Project"),
            (keys.Home, "Primitives"),
            (keys.End, "Project"),
        ]:
            press_key(window, key)
            expect.poll(
                lambda label=label: (
                    element(
                        window, label, role=slint_testing.AccessibleRole.Tab
                    ).accessible_item_selected
                )
            ).to_equal(True)
        snapshot.assert_unchanged()


def test_project_component_drop_preserves_definition_and_undo(
    editor_binary, editor_environment, fixture_project
):
    source_file = fixture_project / "Palette.slint"
    baseline = source_file.read_bytes().replace(
        b"export component Palette",
        b'import { Reusable } from "components/Reusable.slint";\nexport component Palette',
    )
    source_file.write_bytes(baseline)
    reusable = fixture_project / "components" / "Reusable.slint"
    reusable.write_text("""export component Reusable inherits Rectangle {
    width: 84px;
    height: 40px;
    background: #ff0000;
}
export global ProjectState { out property <int> value: 1; }
""")
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = baseline.replace(
        b"    background: #f8fafc;\n",
        b"    background: #f8fafc;\n    Reusable {\n        x: 147px;\n        y: 336px;\n    }\n",
    )
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        element(
            window, "Project", role=slint_testing.AccessibleRole.Tab
        ).invoke_accessible_default_action()
        expect.poll(
            lambda: "Reusable" in [row.accessible_label for row in library_rows(window)]
        ).to_equal(True)
        labels = [row.accessible_label for row in library_rows(window)]
        assert "Palette" in labels and "Reusable" in labels
        assert "ProjectState" not in labels
        target = canvas_drop_position(window)
        begin_palette_drag(window, "Reusable", target)
        snapshot.assert_unchanged_now()
        release_palette_drag(window, target)
        snapshot.wait_for_applied(expected, relative_path="Palette.slint")
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path="Palette.slint")
        reusable.write_text(reusable.read_text().replace("Reusable", "Renamed"))
        source_file.write_bytes(baseline.replace(b"{ Reusable }", b"{ Renamed }"))
        wait_for_source(source_file, source_file.read_bytes())
        expect.poll(
            lambda: "Renamed" in [row.accessible_label for row in library_rows(window)]
        ).to_equal(True)
        expect(
            query(
                element(window, "Element library"),
                "Reusable",
                role=slint_testing.AccessibleRole.ListItem,
            )
        ).to_be_hidden()


def test_library_search_restores_independent_collapse_states(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Palette.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        search = element(window, "Search elements")
        search.accessible_value = "  tOuCh  "
        expect_library(window, ["TouchArea"])
        for label in ("Visual", "Input & interaction"):
            for header in elements(
                window, label, role=slint_testing.AccessibleRole.Button
            ):
                assert not header.accessible_enabled
        search.accessible_value = "missing"
        expect_library(window, [])
        element(window, "No Results")
        for label in ("Visual", "Input & interaction"):
            assert not elements(window, label, role=slint_testing.AccessibleRole.Button)
        search.accessible_value = ""
        expect_library(window, list(PRIMITIVE_KINDS))
        collapsed = set()
        for label, kinds in PRIMITIVE_GROUPS.items():
            collapsed.update(kinds)
            collapsed_labels = [
                kind for kind in PRIMITIVE_KINDS if kind not in collapsed
            ]
            element(
                window, label, role=slint_testing.AccessibleRole.Button
            ).invoke_accessible_default_action()
            expect_library(window, collapsed_labels)
            search.accessible_value = "t"
            expect_library(
                window,
                [kind for kind in PRIMITIVE_KINDS if "t" in kind.lower()],
            )
            search.accessible_value = "missing"
            expect_library(window, [])
            search.accessible_value = ""
            expect_library(window, collapsed_labels)
        element(
            window, "Visual", role=slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        expect_library(window, list(PRIMITIVE_GROUPS["Visual"]))
        search.accessible_value = "touch"
        expect_library(window, ["TouchArea"])
        assert not elements(window, "Visual", role=slint_testing.AccessibleRole.Button)
        search.accessible_value = ""
        expect_library(window, list(PRIMITIVE_GROUPS["Visual"]))
        snapshot.assert_unchanged()


def test_library_layout_stays_anchored_during_search_and_collapse(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Palette.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        expect_library(window, list(PRIMITIVE_KINDS))
        heading = element(
            window, "Element sources", role=slint_testing.AccessibleRole.TabList
        )
        search = element(window, "Search elements")
        library = element(
            window, "Element library", role=slint_testing.AccessibleRole.List
        )
        pane = element(
            window, "Project and elements", role=slint_testing.AccessibleRole.Navigation
        )
        assert library.absolute_position.y + library.size.height == pytest.approx(
            pane.absolute_position.y + pane.size.height, abs=1
        )
        heading_y = heading.absolute_position.y
        search_y = search.absolute_position.y
        rows = library_rows(window)
        group = element(window, "Visual", role=slint_testing.AccessibleRole.Button)
        assert (
            heading.absolute_position.y + heading.size.height
            <= search.absolute_position.y
        )
        assert (
            search.absolute_position.y + search.size.height <= group.absolute_position.y
        )
        assert group.absolute_position.y + group.size.height == pytest.approx(
            rows[0].absolute_position.y
        )
        assert all(row.size.height == pytest.approx(36) for row in rows[:3])
        assert all(row.size.width == rows[0].size.width for row in rows[:3])
        assert rows[2].absolute_position.x == rows[0].absolute_position.x
        assert rows[2].absolute_position.y - rows[
            0
        ].absolute_position.y == pytest.approx(36)
        assert rows[0].absolute_position.y == rows[1].absolute_position.y
        assert rows[0].absolute_position.x < rows[1].absolute_position.x
        assert rows[2].absolute_position.y > rows[0].absolute_position.y
        assert rows[3].absolute_position.y == rows[2].absolute_position.y
        assert rows[4].absolute_position.y > rows[3].absolute_position.y
        collapsed = set()
        for label, kinds in PRIMITIVE_GROUPS.items():
            collapsed.update(kinds)
            remaining = [kind for kind in PRIMITIVE_KINDS if kind not in collapsed]
            element(
                window, label, role=slint_testing.AccessibleRole.Button
            ).invoke_accessible_default_action()
            expect_library(window, remaining)
            assert heading.absolute_position.y == heading_y
            assert search.absolute_position.y == search_y
        for query, expected in [("image", ["Image"]), ("missing", []), ("", [])]:
            search.accessible_value = query
            expect_library(window, expected)
            assert heading.absolute_position.y == heading_y
            assert search.absolute_position.y == search_y


def test_library_search_keyboard_does_not_delete_selection(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        search = element(window, "Search elements")
        search.single_click(slint_testing.PointerEventButton.Left)
        press_keys(window, "Text")
        expect_library(window, ["StyledText", "Text", "TextInput"])
        for _ in range(5):
            press_key(window, keys.Backspace)
        expect_library(window, list(PRIMITIVE_KINDS))
        element(window, "Selected Rectangle", role=slint_testing.AccessibleRole.Region)
        snapshot.assert_unchanged()


def test_toucharea_drag_preview(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Palette.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        target = canvas_drop_position(window)
        begin_palette_drag(window, "TouchArea", target)
        preview = element(
            window, "TouchArea drag preview", role=slint_testing.AccessibleRole.Region
        )
        expected_width, expected_height = TOUCHAREA_PREVIEW_SIZE
        assert preview.size.width == expected_width
        assert preview.size.height == expected_height
        assert preview.absolute_position.x == target.x - expected_width / 2
        assert preview.absolute_position.y == target.y - expected_height / 2
        snapshot.assert_unchanged_now()
        outside = slint_testing.LogicalPosition(x=10, y=10)
        release_palette_drag(window, outside)
        snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "activation_key", [keys.Return, keys.Space], ids=["enter", "space"]
)
@pytest.mark.parametrize(
    ("group_label", "tab_count", "remaining"),
    [
        ("Visual", 1, list(PRIMITIVE_KINDS[4:])),
        (
            "Input & interaction",
            2,
            [
                kind
                for kind in PRIMITIVE_KINDS
                if kind not in PRIMITIVE_GROUPS["Input & interaction"]
            ],
        ),
    ],
)
def test_group_header_keyboard_activation(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    activation_key: str,
    group_label: str,
    tab_count: int,
    remaining: list[str],
) -> None:
    snapshot = SourceSnapshot.capture(fixture_project)
    source_file = fixture_project / "Palette.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        search = element(window, "Search elements")
        search.single_click(slint_testing.PointerEventButton.Left)
        for _ in range(tab_count):
            press_key(window, keys.Tab)
        press_key(window, activation_key)
        expect_library(window, remaining)
        element(window, group_label, role=slint_testing.AccessibleRole.Button)
        press_key(window, activation_key)
        expect_library(window, list(PRIMITIVE_KINDS))
        snapshot.assert_unchanged()
