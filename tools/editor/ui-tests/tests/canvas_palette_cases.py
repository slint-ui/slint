# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import (
    begin_palette_drag,
    center,
    fixture_element,
    hover_fixture_element,
    manual_drag,
    selection_frame,
)
from editor_sync import wait_for_source
from slint_test import expect
from source_snapshot import SourceSnapshot, replace_once, wait_for_source_change
from test_canvas import (
    GOLDENS,
    MOVE_KINDS,
    OUTSIDE_ARTBOARD_DISTANCE,
    PALETTE_DROP_SIZES,
    finish_palette_drag,
    selection_outline_pixel_count,
)
from ui_driver import (
    wait_until,
)


def test_component_palette_preserves_compact_row_layout(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    with editor_factory(source_file) as editor:
        window = editor.window
        section = window.get_by_role("text", name="ELEMENTS").resolve()
        search = window.get_by_accessible_name("Search elements").resolve()
        group = window.get_by_role("button", name="Visual").resolve()
        rows = [
            window.get_by_role("list-item", name=kind).resolve()
            for kind in sorted(PALETTE_DROP_SIZES)
        ]
        assert (
            section.absolute_position.y + section.size.height
            <= search.absolute_position.y
        )
        assert (
            search.absolute_position.y + search.size.height <= group.absolute_position.y
        )
        assert group.absolute_position.y + group.size.height == pytest.approx(
            rows[0].absolute_position.y
        )
        assert all(row.size.height == pytest.approx(36) for row in rows)
        assert all(row.size.width == rows[0].size.width for row in rows)
        assert rows[0].absolute_position.y == rows[1].absolute_position.y
        assert rows[1].absolute_position.x > rows[0].absolute_position.x
        assert rows[2].absolute_position.x == rows[0].absolute_position.x
        assert rows[2].absolute_position.y - rows[
            0
        ].absolute_position.y == pytest.approx(36)


@pytest.mark.parametrize("kind", PALETTE_DROP_SIZES)
def test_component_palette_drop_can_extend_outside_artboard(
    editor_factory,
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "PaletteDropCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source_file) as editor:
        window = editor.window
        artboard = window.get_by_role("region", name="Artboard").resolve()
        target = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + 8,
            y=artboard.absolute_position.y + 8,
        )
        begin_palette_drag(window, kind, target)
        expected_width, expected_height = PALETTE_DROP_SIZES[kind]

        expected_x = round(target.x - artboard.absolute_position.x - expected_width / 2)
        expected_y = round(
            target.y - artboard.absolute_position.y - expected_height / 2
        )
        assert expected_x < 0
        assert expected_y < 0

        finish_palette_drag(window, target)
        expected = (
            GOLDENS / f"PaletteDropCases.outside-{kind.lower()}.slint"
        ).read_bytes()
        snapshot.wait_for_exact(expected, "PaletteDropCases.slint")


def test_palette_preview_follows_rejected_pointer(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "PaletteDropCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source_file) as editor:
        window = editor.window
        artboard = window.get_by_role("region", name="Artboard").resolve()
        first_rejected_position = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x - 60,
            y=artboard.absolute_position.y + 180,
        )
        second_rejected_position = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x - 100,
            y=artboard.absolute_position.y + 280,
        )

        begin_palette_drag(window, "Rectangle", first_rejected_position)
        preview = window.get_by_role("region", name="Rectangle drag preview").resolve()
        expected_width, expected_height = PALETTE_DROP_SIZES["Rectangle"]
        assert preview.size.width == pytest.approx(expected_width)
        assert preview.size.height == pytest.approx(expected_height)
        assert preview.absolute_position.x == pytest.approx(
            first_rejected_position.x - expected_width / 2
        )
        assert preview.absolute_position.y == pytest.approx(
            first_rejected_position.y - expected_height / 2
        )

        window.pointer.move_to(second_rejected_position)
        wait_until(
            lambda: (
                preview
                if preview.absolute_position.x
                == pytest.approx(second_rejected_position.x - expected_width / 2)
                and preview.absolute_position.y
                == pytest.approx(second_rejected_position.y - expected_height / 2)
                else None
            )
        )
        finish_palette_drag(window, second_rejected_position)
        snapshot.assert_unchanged()


@pytest.mark.parametrize("kind", MOVE_KINDS)
def test_unselected_element_shows_hover_outline_without_side_effects(
    editor_factory,
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source_file) as editor:
        window = editor.window
        artboard = window.get_by_role("region", name="Artboard").resolve()
        hover = hover_fixture_element(window, kind)
        expected = {
            "Rectangle": (40, 40, 180, 120),
            "Text": (180, 56, 180, 48),
            "Image": (230, 132, 128, 96),
        }[kind]
        x, y, width, height = expected
        assert hover.absolute_position.x == pytest.approx(
            artboard.absolute_position.x + x
        )
        assert hover.absolute_position.y == pytest.approx(
            artboard.absolute_position.y + y
        )
        assert hover.size.width == pytest.approx(width)
        assert hover.size.height == pytest.approx(height)
        window.get_by_accessible_name(f"Selected {kind}").wait_for(state="hidden")
        snapshot.assert_unchanged()

        blank = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + artboard.size.width - 12,
            y=artboard.absolute_position.y + artboard.size.height - 12,
        )
        window.pointer.move_to(blank)
        expect(window.get_by_accessible_name(f"Hovered {kind}")).to_be_hidden()
        snapshot.assert_unchanged()

        outside = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x - 12,
            y=artboard.absolute_position.y - 12,
        )
        window.pointer.move_to(outside)
        expect(window.get_by_accessible_name(f"Hovered {kind}")).to_be_hidden()
        snapshot.assert_unchanged()


def test_overlapping_elements_update_hover_outline_to_topmost_item(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source_file) as editor:
        window = editor.window
        artboard = window.get_by_role("region", name="Artboard").resolve()
        rectangle_only = center(fixture_element(window, "Rectangle"))
        overlapping = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + 200,
            y=artboard.absolute_position.y + 80,
        )
        window.pointer.move_to(rectangle_only)
        window.get_by_accessible_name("Hovered Rectangle").wait_for()
        window.pointer.move_to(overlapping)
        expect(window.get_by_accessible_name("Hovered Text")).to_be_visible()
        expect(window.get_by_accessible_name("Hovered Rectangle")).to_be_hidden()
        snapshot.assert_unchanged()


def test_overlapping_hover_does_not_intercept_selected_element_drag(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source_file) as editor:
        window = editor.window
        artboard = window.get_by_role("region", name="Artboard").resolve()
        editor.canvas.select("Rectangle")
        overlapping = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + 200,
            y=artboard.absolute_position.y + 80,
        )
        window.pointer.move_to(overlapping)
        window.get_by_accessible_name("Hovered Text").wait_for()
        initial_frame = selection_frame(window, "Rectangle")
        target = slint_testing.LogicalPosition(
            x=overlapping.x + 20,
            y=overlapping.y + 16,
        )
        window.pointer.press_at(overlapping)
        window.pointer.move_to(target)
        assert selection_frame(window, "Rectangle") != initial_frame
        window.get_by_accessible_name("Selected Text").wait_for(state="hidden")
        expect(window.get_by_accessible_name("Hovered Text")).to_be_hidden()
        snapshot.assert_unchanged_now()
        window.pointer.release_at(target)


def test_hover_outside_selected_element_can_select_and_drag_child(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        start = center(fixture_element(window, "Text"))
        window.pointer.move_to(start)
        window.get_by_accessible_name("Hovered Text").wait_for()
        target = slint_testing.LogicalPosition(x=start.x + 20, y=start.y + 16)
        window.pointer.press_at(start)
        window.get_by_accessible_name("Selected Text").wait_for()
        initial_frame = selection_frame(window, "Text")
        window.pointer.move_to(target)
        assert selection_frame(window, "Text") != initial_frame
        snapshot.assert_unchanged_now()
        window.pointer.release_at(target)


def test_selected_element_shows_hover_outline(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Text")
        window.pointer.move_to(center(fixture_element(window, "Text")))
        window.get_by_accessible_name("Hovered Text").wait_for()
        window.get_by_accessible_name("Selected Text").wait_for()
        snapshot.assert_unchanged()


@pytest.mark.parametrize("kind", MOVE_KINDS)
@pytest.mark.parametrize("jitter", [0, 1])
def test_unselected_element_click_selects_without_editing_source(
    editor_factory,
    fixture_project: Path,
    kind: str,
    jitter: int,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source_file) as editor:
        window = editor.window
        hover = hover_fixture_element(window, kind)
        target = center(hover)
        window.pointer.press_at(target)
        below_threshold = slint_testing.LogicalPosition(
            x=target.x + jitter,
            y=target.y + jitter,
        )
        window.pointer.move_to(below_threshold)
        window.pointer.release_at(below_threshold)
        window.get_by_role("region", name=f"Selected {kind}").wait_for()
        # Releasing a click restores hover without another pointer move.
        window.get_by_accessible_name(f"Hovered {kind}").wait_for()
        snapshot.assert_unchanged()


@pytest.mark.parametrize("kind", ["Text", "Image"])
def test_unselected_element_can_be_dragged_in_one_gesture(
    editor_factory,
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    original = {
        "Text": b"        x: 180px;\n        y: 56px;",
        "Image": b"        x: 230px;\n        y: 132px;",
    }[kind]
    updated = {
        "Text": b"        x: 200px;\n        y: 72px;",
        "Image": b"        x: 250px;\n        y: 148px;",
    }[kind]
    with editor_factory(source_file) as editor:
        window = editor.window
        hover = hover_fixture_element(window, kind)
        start = center(hover)
        end = slint_testing.LogicalPosition(x=start.x + 20, y=start.y + 16)
        window.pointer.press_at(start)
        window.get_by_role("region", name=f"Selected {kind}").wait_for()
        initial_frame = selection_frame(window, kind)
        for step in range(1, 4):
            fraction = step / 3
            position = slint_testing.LogicalPosition(
                x=start.x + (end.x - start.x) * fraction,
                y=start.y + (end.y - start.y) * fraction,
            )
            window.pointer.move_to(position)
        assert selection_frame(window, kind) != initial_frame
        window.get_by_accessible_name(f"{kind} resize top-left").wait_for(
            state="hidden"
        )
        snapshot.assert_unchanged_now()
        window.pointer.release_at(end)
        snapshot.wait_for_exact(replace_once(baseline, original, updated))


def test_unselected_rectangle_direct_drag_starts_before_release(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source_file) as editor:
        window = editor.window
        hover = hover_fixture_element(window, "Rectangle")
        start = center(hover)
        end = slint_testing.LogicalPosition(x=start.x + 20, y=start.y + 16)
        window.pointer.press_at(start)
        window.get_by_accessible_name("Selected Rectangle").wait_for()
        initial_frame = selection_frame(window, "Rectangle")
        window.pointer.move_to(end)
        assert selection_frame(window, "Rectangle") != initial_frame
        window.get_by_accessible_name("Rectangle resize top-left").wait_for(
            state="hidden"
        )
        assert selection_outline_pixel_count(window, "Rectangle") == 0
        snapshot.assert_unchanged_now()
        window.pointer.release_at(end)


@pytest.mark.parametrize(
    ("element_id", "kind"),
    [("empty-image", "Image"), ("touch-area", "TouchArea")],
)
@pytest.mark.parametrize("direct_drag", [False, True])
def test_invisible_element_keeps_selection_outline_during_drag(
    editor_factory,
    fixture_project: Path,
    element_id: str,
    kind: str,
    direct_drag: bool,
) -> None:
    source_file = fixture_project / "DragOutlineCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source_file) as editor:
        window = editor.window
        element = window.get_by_id(f"DragOutlineCases::{element_id}").resolve()
        if direct_drag:
            start = center(element)
            window.pointer.move_to(start)
            window.get_by_accessible_name(f"Hovered {kind}").wait_for()
        else:
            editor.outline.select(element_id)
            start = center(
                window.get_by_accessible_name(f"{kind} move handle").resolve()
            )
        end = slint_testing.LogicalPosition(x=start.x + 20, y=start.y + 16)
        window.pointer.press_at(start)
        window.pointer.move_to(end)

        assert selection_outline_pixel_count(window, kind) > 10
        window.get_by_accessible_name(f"{kind} resize top-left").wait_for(
            state="hidden"
        )
        snapshot.assert_unchanged_now()
        window.pointer.release_at(end)


@pytest.mark.parametrize("kind", PALETTE_DROP_SIZES)
def test_repeated_palette_drop_preserves_component_kind(
    editor_factory,
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "RepeatedPaletteDrops.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source_file) as editor:
        window = editor.window
        window.get_by_role("text", name="Reload probe").wait_for()
        editor.outline.select("drop-target")
        x, y, width, height = selection_frame(window, "Rectangle")
        target = slint_testing.LogicalPosition(x=x + width / 2, y=y + height / 2)

        for step in (1, 2):
            begin_palette_drag(window, kind, target)
            finish_palette_drag(window, target)
            expected = (
                GOLDENS / f"RepeatedPaletteDrops.{kind.lower()}-{step}.slint"
            ).read_bytes()
            snapshot.wait_for_applied(expected, "RepeatedPaletteDrops.slint")
            reload_label = f"Reload probe {step}"
            reloaded = expected.replace(b"Reload probe", reload_label.encode(), 1)
            source_file.write_bytes(reloaded)
            wait_for_source(source_file, reloaded)
            window.get_by_role("text", name=reload_label).wait_for(timeout=(15) * 1000)
            source_file.write_bytes(expected)
            wait_for_source(source_file, expected)
            window.get_by_role("text", name="Reload probe").wait_for(
                timeout=(15) * 1000
            )
            window.get_by_accessible_name(f"{kind} drag preview").wait_for(
                state="hidden"
            )
            window.pointer.press_at(target)
            window.pointer.release_at(target)
            window.get_by_role("region", name=f"Selected {kind}").wait_for(
                timeout=(15) * 1000
            )
            editor.outline.select("drop-target")
            window.get_by_role("region", name="Selected Rectangle").wait_for(
                timeout=(15) * 1000
            )

        element_types = {
            line.strip().split(maxsplit=1)[0]
            for line in source_file.read_text().splitlines()
            if line.strip().endswith("{") and not line.strip().startswith("export ")
        }
        assert element_types.isdisjoint(
            {"Flickable", "Window", "HorizontalLayout", "VerticalLayout", "GridLayout"}
        )


def test_component_palette_drag_over_rotated_element_does_not_crash(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "RotatedCanvasCases.slint"
    baseline = source_file.read_bytes()
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.outline.select("rotated-free-rectangle")
        rotated_rectangle = window.get_by_role(
            "region", name="Selected Rectangle"
        ).resolve()
        target = center(rotated_rectangle)
        artboard = window.get_by_role("region", name="Artboard").resolve()
        outside_artboard = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x - 20,
            y=artboard.absolute_position.y,
        )
        window.pointer.press_at(outside_artboard)
        window.pointer.release_at(outside_artboard)
        expect(window.get_by_accessible_name("Selected Rectangle")).to_be_hidden()
        begin_palette_drag(window, "Rectangle", target)
        finish_palette_drag(window, target)

        updated = wait_for_source_change(source_file, baseline)
        assert updated.count(b"Rectangle {") == baseline.count(b"Rectangle {") + 1


def test_image_asset_mode_destroys_canvas_without_replaying_palette_drop(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "PaletteDropCases.slint"
    asset_directory = fixture_project / "assets"
    image_file = asset_directory / "checker.svg"
    baseline = source_file.read_bytes()
    with editor_factory(source_file) as editor:
        window = editor.window
        artboard = window.get_by_role("region", name="Artboard").resolve()
        target = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + 260,
            y=artboard.absolute_position.y + 220,
        )
        begin_palette_drag(window, "Rectangle", target)
        finish_palette_drag(window, target)
        wait_for_source_change(source_file, baseline)
        snapshot = SourceSnapshot.capture(fixture_project)

        asset_directory_row = editor.files.row(asset_directory)
        asset_directory_row.click()
        image_row = editor.files.row(image_file)
        image_row.click()
        preview_tab = window.get_by_role("button", name="Preview").resolve()
        assert preview_tab.accessible_checked
        window.get_by_role("region", name="Artboard").wait_for(state="hidden")
        assert (
            not window.root_element.query_descendants()
            .match_type_name("EditorCanvas")
            .find_all()
        )

        component_row = editor.files.row(source_file)
        component_row.click()
        window.get_by_role("region", name="Artboard").wait_for()
        assert (
            len(
                window.root_element.query_descendants()
                .match_type_name("EditorCanvas")
                .find_all()
            )
            == 1
        )
        snapshot.assert_unchanged()


@pytest.mark.parametrize("kind", MOVE_KINDS)
@pytest.mark.parametrize("outside_window", (False, True))
def test_move_element_writes_exact_source_on_release(
    editor_factory,
    fixture_project: Path,
    kind: str,
    outside_window: bool,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    positions = {
        "Rectangle": (40, 40),
        "Text": (180, 56),
        "Image": (230, 132),
    }
    with editor_factory(source_file) as editor:
        window = editor.window
        snapshot = SourceSnapshot.capture(fixture_project)
        editor.canvas.select(kind)
        handle = window.get_by_accessible_name(f"{kind} move handle").resolve()
        dx, dy = 20, 16
        if outside_window:
            start = center(handle)
            size = window.root_element.size
            dx = math.ceil(size.width + OUTSIDE_ARTBOARD_DISTANCE - start.x)
            dy = math.ceil(size.height + OUTSIDE_ARTBOARD_DISTANCE - start.y)
        manual_drag(window, handle, dx, dy, snapshot)
        x, y = positions[kind]
        expected = replace_once(
            baseline,
            f"        x: {x}px;\n        y: {y}px;".encode(),
            f"        x: {x + dx}px;\n        y: {y + dy}px;".encode(),
        )
        snapshot.wait_for_applied(expected)
