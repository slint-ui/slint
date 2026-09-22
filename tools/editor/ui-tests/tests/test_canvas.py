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
    frame_rotation,
    hover_fixture_element,
    live_modifier_resize,
    manual_drag,
    manual_radius_drag,
    manual_rotation_drag,
    position_distance,
    radius_handle,
    rotation_delta,
    rotation_start,
    same_state,
    selection_frame,
)
from editor_sync import wait_for_source
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once, wait_for_source_change
from ui_driver import (
    elements_with_label,
    file_row,
    first_window,
    launch_editor,
    select_fixture_element,
    select_outline_row,
    wait_until,
    window_element_with_label,
)

GOLDENS = Path(__file__).resolve().parents[1] / "goldens"
CORNERS = ["top-left", "top-right", "bottom-right", "bottom-left"]
OPPOSITE_CORNERS = {
    "top-left": "bottom-right",
    "top-right": "bottom-left",
    "bottom-right": "top-left",
    "bottom-left": "top-right",
}
CORNER_DELTAS: dict[str, tuple[int, int]] = {
    "top-left": (-20, -16),
    "top-right": (20, -16),
    "bottom-right": (20, 16),
    "bottom-left": (-20, 16),
}
RADIUS_DELTAS: dict[str, tuple[int, int]] = {
    "top-left": (4, 4),
    "top-right": (-4, 4),
    "bottom-right": (-4, -4),
    "bottom-left": (4, -4),
}
MOVE_KINDS = ("Rectangle", "Text", "Image")
ROTATED_KINDS = ("Rectangle", "Text", "Image")
# The rotation the elements of RotatedCanvasCases.slint carry.
ROTATED_FIXTURE_ANGLE = 30
BOUNDARY_KINDS = ("Rectangle", "Text", "Image")
BOUNDARY_MOVE_DIRECTIONS = ("top-left", "bottom-right")
OUTSIDE_ARTBOARD_DISTANCE = 32
BOUNDS_WIDTH = 390
BOUNDS_HEIGHT = 720
THRESHOLD_LABELS = (
    "Rectangle move handle",
    "Rectangle resize bottom-right",
    "Rectangle rotate top-left",
    "Rectangle radius top-left",
)
DISABLED_IDS = ("layout-rectangle", "rotated-rectangle")
PALETTE_DROP_SIZES = {
    "Rectangle": (160, 64),
    "Text": (220, 40),
    "Image": (160, 96),
}


def finish_palette_drag(
    window: slint_testing.Window, target: slint_testing.LogicalPosition
) -> None:
    window.dispatch_event(
        slint_testing.PointerReleaseEvent(target, slint_testing.PointerEventButton.Left)
    )


def test_component_palette_preserves_compact_row_layout(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        section = window_element_with_label(
            window, "ELEMENTS", slint_testing.AccessibleRole.Text
        )
        search = window_element_with_label(window, "Search elements")
        group = window_element_with_label(
            window, "Visual", slint_testing.AccessibleRole.Button
        )
        rows = [
            window_element_with_label(
                window, kind, slint_testing.AccessibleRole.ListItem
            )
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
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "PaletteDropCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        artboard = window_element_with_label(
            window, "Artboard", slint_testing.AccessibleRole.Region
        )
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
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "PaletteDropCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        artboard = window_element_with_label(
            window, "Artboard", slint_testing.AccessibleRole.Region
        )
        first_rejected_position = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x - 60,
            y=artboard.absolute_position.y + 180,
        )
        second_rejected_position = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x - 100,
            y=artboard.absolute_position.y + 280,
        )

        begin_palette_drag(window, "Rectangle", first_rejected_position)
        preview = window_element_with_label(
            window,
            "Rectangle drag preview",
            slint_testing.AccessibleRole.Region,
        )
        expected_width, expected_height = PALETTE_DROP_SIZES["Rectangle"]
        assert preview.size.width == pytest.approx(expected_width)
        assert preview.size.height == pytest.approx(expected_height)
        assert preview.absolute_position.x == pytest.approx(
            first_rejected_position.x - expected_width / 2
        )
        assert preview.absolute_position.y == pytest.approx(
            first_rejected_position.y - expected_height / 2
        )

        window.dispatch_event(slint_testing.PointerMoveEvent(second_rejected_position))
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
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        artboard = window_element_with_label(
            window, "Artboard", slint_testing.AccessibleRole.Region
        )
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
        assert not elements_with_label(window.root_element, f"Selected {kind}")
        snapshot.assert_unchanged()

        blank = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + artboard.size.width - 12,
            y=artboard.absolute_position.y + artboard.size.height - 12,
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(blank))
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, f"Hovered {kind}")
                else None
            )
        )
        snapshot.assert_unchanged()

        outside = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x - 12,
            y=artboard.absolute_position.y - 12,
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(outside))
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, f"Hovered {kind}")
                else None
            )
        )
        snapshot.assert_unchanged()


def test_overlapping_elements_update_hover_outline_to_topmost_item(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        artboard = window_element_with_label(
            window, "Artboard", slint_testing.AccessibleRole.Region
        )
        rectangle_only = center(fixture_element(window, "Rectangle"))
        overlapping = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + 200,
            y=artboard.absolute_position.y + 80,
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(rectangle_only))
        window_element_with_label(window, "Hovered Rectangle")
        window.dispatch_event(slint_testing.PointerMoveEvent(overlapping))
        wait_until(
            lambda: (
                True
                if elements_with_label(window.root_element, "Hovered Text")
                and not elements_with_label(window.root_element, "Hovered Rectangle")
                else None
            )
        )
        snapshot.assert_unchanged()


def test_overlapping_hover_does_not_intercept_selected_element_drag(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        artboard = window_element_with_label(
            window, "Artboard", slint_testing.AccessibleRole.Region
        )
        select_fixture_element(window, "Rectangle")
        overlapping = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + 200,
            y=artboard.absolute_position.y + 80,
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(overlapping))
        window_element_with_label(window, "Hovered Text")
        initial_frame = selection_frame(window, "Rectangle")
        target = slint_testing.LogicalPosition(
            x=overlapping.x + 20,
            y=overlapping.y + 16,
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(overlapping, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(target))
        assert selection_frame(window, "Rectangle") != initial_frame
        assert not elements_with_label(window.root_element, "Selected Text")
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "Hovered Text")
                else None
            )
        )
        snapshot.assert_unchanged_now()
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))


def test_hover_outside_selected_element_can_select_and_drag_child(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        start = center(fixture_element(window, "Text"))
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window_element_with_label(window, "Hovered Text")
        target = slint_testing.LogicalPosition(x=start.x + 20, y=start.y + 16)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window_element_with_label(window, "Selected Text")
        initial_frame = selection_frame(window, "Text")
        window.dispatch_event(slint_testing.PointerMoveEvent(target))
        assert selection_frame(window, "Text") != initial_frame
        snapshot.assert_unchanged_now()
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))


def test_selected_element_shows_hover_outline(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Text")
        window.dispatch_event(
            slint_testing.PointerMoveEvent(center(fixture_element(window, "Text")))
        )
        window_element_with_label(window, "Hovered Text")
        window_element_with_label(window, "Selected Text")
        snapshot.assert_unchanged()


@pytest.mark.parametrize("kind", MOVE_KINDS)
@pytest.mark.parametrize("jitter", [0, 1])
def test_unselected_element_click_selects_without_editing_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    jitter: int,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        hover = hover_fixture_element(window, kind)
        target = center(hover)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        below_threshold = slint_testing.LogicalPosition(
            x=target.x + jitter,
            y=target.y + jitter,
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(below_threshold))
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(below_threshold, button)
        )
        window_element_with_label(
            window, f"Selected {kind}", slint_testing.AccessibleRole.Region
        )
        # Releasing a click restores hover without another pointer move.
        window_element_with_label(window, f"Hovered {kind}")
        snapshot.assert_unchanged()


@pytest.mark.parametrize("kind", ["Text", "Image"])
def test_unselected_element_can_be_dragged_in_one_gesture(
    editor_binary: Path,
    editor_environment: dict[str, str],
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
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        hover = hover_fixture_element(window, kind)
        start = center(hover)
        end = slint_testing.LogicalPosition(x=start.x + 20, y=start.y + 16)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window_element_with_label(
            window, f"Selected {kind}", slint_testing.AccessibleRole.Region
        )
        initial_frame = selection_frame(window, kind)
        for step in range(1, 4):
            fraction = step / 3
            position = slint_testing.LogicalPosition(
                x=start.x + (end.x - start.x) * fraction,
                y=start.y + (end.y - start.y) * fraction,
            )
            window.dispatch_event(slint_testing.PointerMoveEvent(position))
        assert selection_frame(window, kind) != initial_frame
        assert not elements_with_label(window.root_element, f"{kind} resize top-left")
        snapshot.assert_unchanged_now()
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        snapshot.wait_for_exact(replace_once(baseline, original, updated))


def test_unselected_rectangle_direct_drag_starts_before_release(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        hover = hover_fixture_element(window, "Rectangle")
        start = center(hover)
        end = slint_testing.LogicalPosition(x=start.x + 20, y=start.y + 16)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window_element_with_label(window, "Selected Rectangle")
        initial_frame = selection_frame(window, "Rectangle")
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        assert selection_frame(window, "Rectangle") != initial_frame
        assert not elements_with_label(window.root_element, "Rectangle resize top-left")
        snapshot.assert_unchanged_now()
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))


@pytest.mark.parametrize("kind", PALETTE_DROP_SIZES)
def test_repeated_palette_drop_preserves_component_kind(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "RepeatedPaletteDrops.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        window_element_with_label(
            window, "Reload probe", slint_testing.AccessibleRole.Text
        )
        select_outline_row(window, "drop-target")
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
            window_element_with_label(
                window, reload_label, slint_testing.AccessibleRole.Text, timeout=15
            )
            source_file.write_bytes(expected)
            wait_for_source(source_file, expected)
            window_element_with_label(
                window, "Reload probe", slint_testing.AccessibleRole.Text, timeout=15
            )
            wait_until(
                lambda: (
                    True
                    if not elements_with_label(
                        window.root_element, f"{kind} drag preview"
                    )
                    else None
                )
            )
            button = slint_testing.PointerEventButton.Left
            window.dispatch_event(slint_testing.PointerPressEvent(target, button))
            window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
            window_element_with_label(
                window,
                f"Selected {kind}",
                slint_testing.AccessibleRole.Region,
                timeout=15,
            )
            select_outline_row(window, "drop-target")
            window_element_with_label(
                window,
                "Selected Rectangle",
                slint_testing.AccessibleRole.Region,
                timeout=15,
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
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "RotatedCanvasCases.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_outline_row(window, "rotated-free-rectangle")
        rotated_rectangle = window_element_with_label(
            window, "Selected Rectangle", slint_testing.AccessibleRole.Region
        )
        target = center(rotated_rectangle)
        artboard = window_element_with_label(
            window, "Artboard", slint_testing.AccessibleRole.Region
        )
        outside_artboard = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x - 20,
            y=artboard.absolute_position.y,
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(outside_artboard, button))
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(outside_artboard, button)
        )
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "Selected Rectangle")
                else None
            )
        )
        begin_palette_drag(window, "Rectangle", target)
        finish_palette_drag(window, target)

        updated = wait_for_source_change(source_file, baseline)
        assert updated.count(b"Rectangle {") == baseline.count(b"Rectangle {") + 1


def test_image_asset_mode_destroys_canvas_without_replaying_palette_drop(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "PaletteDropCases.slint"
    asset_directory = fixture_project / "assets"
    image_file = asset_directory / "checker.svg"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        artboard = window_element_with_label(
            window, "Artboard", slint_testing.AccessibleRole.Region
        )
        target = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + 260,
            y=artboard.absolute_position.y + 220,
        )
        begin_palette_drag(window, "Rectangle", target)
        finish_palette_drag(window, target)
        wait_for_source_change(source_file, baseline)
        snapshot = SourceSnapshot.capture(fixture_project)

        asset_directory_row = file_row(window, asset_directory)
        asset_directory_row.single_click(slint_testing.PointerEventButton.Left)
        image_row = file_row(window, image_file)
        image_row.single_click(slint_testing.PointerEventButton.Left)
        preview_tab = window_element_with_label(
            window, "Preview", slint_testing.AccessibleRole.Button
        )
        assert preview_tab.accessible_checked
        assert not elements_with_label(
            window.root_element, "Artboard", slint_testing.AccessibleRole.Region
        )
        assert (
            not window.root_element.query_descendants()
            .match_type_name("EditorCanvas")
            .find_all()
        )

        component_row = file_row(window, source_file)
        component_row.single_click(slint_testing.PointerEventButton.Left)
        window_element_with_label(
            window, "Artboard", slint_testing.AccessibleRole.Region
        )
        assert (
            len(
                window.root_element.query_descendants()
                .match_type_name("EditorCanvas")
                .find_all()
            )
            == 1
        )
        snapshot.assert_unchanged()


def rotated_resize_values(
    x: float,
    y: float,
    width: float,
    height: float,
    corner: str,
    dx: float,
    dy: float,
    angle_degrees: float = ROTATED_FIXTURE_ANGLE,
) -> tuple[int, int, int, int]:
    angle = math.radians(angle_degrees)
    cosine = math.cos(angle)
    sine = math.sin(angle)
    local_dx = dx * cosine + dy * sine
    local_dy = -dx * sine + dy * cosine
    new_width = width - local_dx if corner.endswith("left") else width + local_dx
    new_height = height - local_dy if corner.startswith("top") else height + local_dy

    fixed_left = not corner.endswith("left")
    fixed_top = not corner.startswith("top")
    old_fixed_x = 0 if fixed_left else width
    old_fixed_y = 0 if fixed_top else height
    old_center_x = width / 2
    old_center_y = height / 2
    fixed_parent_x = (
        x
        + old_center_x
        + (old_fixed_x - old_center_x) * cosine
        - (old_fixed_y - old_center_y) * sine
    )
    fixed_parent_y = (
        y
        + old_center_y
        + (old_fixed_x - old_center_x) * sine
        + (old_fixed_y - old_center_y) * cosine
    )
    new_fixed_x = 0 if fixed_left else new_width
    new_fixed_y = 0 if fixed_top else new_height
    new_center_x = new_width / 2
    new_center_y = new_height / 2
    fixed_delta_x = (new_fixed_x - new_center_x) * cosine - (
        new_fixed_y - new_center_y
    ) * sine
    fixed_delta_y = (new_fixed_x - new_center_x) * sine + (
        new_fixed_y - new_center_y
    ) * cosine
    new_x = fixed_parent_x - fixed_delta_x - new_center_x
    new_y = fixed_parent_y - fixed_delta_y - new_center_y
    return (round(new_x), round(new_y), round(new_width), round(new_height))


def geometry_source(values: tuple[int, int, int, int]) -> bytes:
    x, y, width, height = values
    return (
        f"        x: {x}px;\n"
        f"        y: {y}px;\n"
        f"        width: {width}px;\n"
        f"        height: {height}px;"
    ).encode()


def outside_move_values(
    geometry: tuple[int, int, int, int], direction: str
) -> tuple[int, int, int, int]:
    _, _, width, height = geometry
    if direction == "top-left":
        return (
            -OUTSIDE_ARTBOARD_DISTANCE - width // 2,
            -OUTSIDE_ARTBOARD_DISTANCE - height // 2,
            width,
            height,
        )
    return (
        BOUNDS_WIDTH + OUTSIDE_ARTBOARD_DISTANCE - width // 2,
        BOUNDS_HEIGHT + OUTSIDE_ARTBOARD_DISTANCE - height // 2,
        width,
        height,
    )


def outside_resize_values(
    geometry: tuple[int, int, int, int], corner: str
) -> tuple[int, int, int, int]:
    x, y, width, height = geometry
    if corner == "top-left":
        return (
            -OUTSIDE_ARTBOARD_DISTANCE,
            -OUTSIDE_ARTBOARD_DISTANCE,
            x + width + OUTSIDE_ARTBOARD_DISTANCE,
            y + height + OUTSIDE_ARTBOARD_DISTANCE,
        )
    if corner == "top-right":
        return (
            x,
            -OUTSIDE_ARTBOARD_DISTANCE,
            BOUNDS_WIDTH + OUTSIDE_ARTBOARD_DISTANCE - x,
            y + height + OUTSIDE_ARTBOARD_DISTANCE,
        )
    if corner == "bottom-right":
        return (
            x,
            y,
            BOUNDS_WIDTH + OUTSIDE_ARTBOARD_DISTANCE - x,
            BOUNDS_HEIGHT + OUTSIDE_ARTBOARD_DISTANCE - y,
        )
    return (
        -OUTSIDE_ARTBOARD_DISTANCE,
        y,
        x + width + OUTSIDE_ARTBOARD_DISTANCE,
        BOUNDS_HEIGHT + OUTSIDE_ARTBOARD_DISTANCE - y,
    )


def radius_handle_positions(
    window: slint_testing.Window,
) -> dict[str, slint_testing.LogicalPosition]:
    return {
        corner: center(window_element_with_label(window, f"Rectangle radius {corner}"))
        for corner in CORNERS
    }


def assert_radius_handle_positions(
    window: slint_testing.Window,
    initial_positions: dict[str, slint_testing.LogicalPosition],
    active_corner: str,
    radius_delta: float,
    single_corner: bool,
) -> None:
    current_positions = radius_handle_positions(window)
    for corner in CORNERS:
        direction = RADIUS_DELTAS[corner]
        expected_position = (
            slint_testing.LogicalPosition(
                x=initial_positions[corner].x
                + math.copysign(radius_delta, direction[0]),
                y=initial_positions[corner].y
                + math.copysign(radius_delta, direction[1]),
            )
            if not single_corner or corner == active_corner
            else initial_positions[corner]
        )
        assert position_distance(current_positions[corner], expected_position) < 1.5


def wait_for_radius_tooltip(window: slint_testing.Window, radius: float) -> None:
    wait_until(
        lambda: (
            True
            if float(
                window_element_with_label(
                    window, "Radius value", slint_testing.AccessibleRole.Text
                ).accessible_value
            )
            == radius
            else None
        )
    )


@pytest.mark.parametrize("kind", MOVE_KINDS)
@pytest.mark.parametrize("outside_window", (False, True))
def test_move_element_writes_exact_source_on_release(
    editor_binary: Path,
    editor_environment: dict[str, str],
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
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_fixture_element(window, kind)
        handle = window_element_with_label(window, f"{kind} move handle")
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


@pytest.mark.parametrize("kind", MOVE_KINDS)
def test_move_rotated_element_writes_exact_source_on_release(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "RotatedCanvasCases.slint"
    baseline = source_file.read_bytes()
    positions = {
        "Rectangle": (64, 56),
        "Text": (160, 64),
        "Image": (200, 208),
    }
    dx, dy = 20, 16
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_outline_row(window, f"rotated-free-{kind.lower()}")
        manual_drag(
            window,
            window_element_with_label(window, f"{kind} move handle"),
            dx,
            dy,
            snapshot,
        )
        x, y = positions[kind]
        expected = replace_once(
            baseline,
            f"        x: {x}px;\n        y: {y}px;".encode(),
            f"        x: {x + dx}px;\n        y: {y + dy}px;".encode(),
        )
        snapshot.wait_for_exact(expected, "RotatedCanvasCases.slint")


def test_nested_rotated_element_move_writes_exact_local_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "RotatedCanvasCases.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_outline_row(window, "nested-rotated-text")
        manual_drag(
            window,
            window_element_with_label(window, "Text move handle"),
            20,
            16,
            snapshot,
        )
        expected = replace_once(
            baseline,
            b"                x: 44px;\n                y: 52px;",
            b"                x: 60px;\n                y: 32px;",
        )
        snapshot.wait_for_exact(expected, "RotatedCanvasCases.slint")


@pytest.mark.parametrize("corner", CORNERS)
def test_each_resize_handle_writes_exact_source_on_release(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    corner: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    original = geometry_source((40, 40, 180, 120))
    geometries = {
        "top-left": (20, 24, 200, 136),
        "top-right": (40, 24, 200, 136),
        "bottom-right": (40, 40, 200, 136),
        "bottom-left": (20, 40, 200, 136),
    }
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_drag(
            window,
            window_element_with_label(window, f"Rectangle resize {corner}"),
            *CORNER_DELTAS[corner],
            snapshot,
        )
        expected = replace_once(baseline, original, geometry_source(geometries[corner]))
        snapshot.wait_for_exact(expected)


def test_shift_resize_is_proportional(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_drag(
            window,
            window_element_with_label(window, "Rectangle resize bottom-right"),
            20,
            16,
            snapshot,
            shift=True,
        )
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                b"        width: 180px;\n        height: 120px;",
                b"        width: 200px;\n        height: 200px;",
            ),
        )


@pytest.mark.parametrize(
    "press_shift_during_drag",
    [pytest.param(True, id="press-shift"), pytest.param(False, id="release-shift")],
)
def test_resize_modifier_changes_during_drag(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    press_shift_during_drag: bool,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        live_modifier_resize(
            window,
            snapshot,
            press_shift_during_drag=press_shift_during_drag,
        )
        geometry = (
            b"        width: 200px;\n        height: 200px;"
            if press_shift_during_drag
            else b"        width: 200px;\n        height: 136px;"
        )
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                b"        width: 180px;\n        height: 120px;",
                geometry,
            ),
        )


@pytest.mark.parametrize("kind", ROTATED_KINDS)
@pytest.mark.parametrize("corner", CORNERS)
def test_rotated_element_resize_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    corner: str,
) -> None:
    source_file = fixture_project / "RotatedCanvasCases.slint"
    baseline = source_file.read_bytes()
    geometries = {
        "Rectangle": (64, 56, 140, 96),
        "Text": (160, 64, 180, 56),
        "Image": (200, 208, 144, 96),
    }
    element_id = f"rotated-free-{kind.lower()}"

    geometry = geometries[kind]
    original = geometry_source(geometry)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_outline_row(window, element_id)
        opposite_label = f"{kind} resize {OPPOSITE_CORNERS[corner]}"
        fixed_handle_center = manual_drag(
            window,
            window_element_with_label(window, f"{kind} resize {corner}"),
            *CORNER_DELTAS[corner],
            snapshot,
            fixed_handle_label=opposite_label,
        )
        assert fixed_handle_center is not None
        expected_geometry = rotated_resize_values(
            *geometry,
            corner,
            *CORNER_DELTAS[corner],
        )
        changed = geometry_source(expected_geometry)
        # The handles are read back, so the preview has to hold the edited source.
        snapshot.wait_for_applied(
            replace_once(baseline, original, changed),
            "RotatedCanvasCases.slint",
        )
        assert (
            position_distance(
                center(
                    window_element_with_label(window, opposite_label),
                    math.radians(ROTATED_FIXTURE_ANGLE),
                ),
                fixed_handle_center,
            )
            < 1.5
        )


def run_canvas_boundary_case(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    operation: str,
    target: str,
) -> None:
    source_file = fixture_project / "BoundsCases.slint"
    baseline = source_file.read_bytes()
    geometries = {
        "Rectangle": (96, 80, 120, 96),
        "Text": (104, 260, 140, 48),
        "Image": (128, 480, 128, 96),
    }
    element_id = f"bounds-{kind.lower()}"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_outline_row(window, element_id)

        geometry = geometries[kind]
        artboard = window_element_with_label(
            window, "Artboard", slint_testing.AccessibleRole.Region
        )
        left = artboard.absolute_position.x
        top = artboard.absolute_position.y
        right = left + artboard.size.width
        bottom = top + artboard.size.height
        if operation == "move":
            label = f"{kind} move handle"
            expected_geometry = outside_move_values(geometry, target)
        else:
            label = f"{kind} resize {target}"
            expected_geometry = outside_resize_values(geometry, target)
        target_position = {
            "top-left": (
                left - OUTSIDE_ARTBOARD_DISTANCE,
                top - OUTSIDE_ARTBOARD_DISTANCE,
            ),
            "top-right": (
                right + OUTSIDE_ARTBOARD_DISTANCE,
                top - OUTSIDE_ARTBOARD_DISTANCE,
            ),
            "bottom-right": (
                right + OUTSIDE_ARTBOARD_DISTANCE,
                bottom + OUTSIDE_ARTBOARD_DISTANCE,
            ),
            "bottom-left": (
                left - OUTSIDE_ARTBOARD_DISTANCE,
                bottom + OUTSIDE_ARTBOARD_DISTANCE,
            ),
        }[target]
        handle = window_element_with_label(window, label)
        start = center(handle)
        delta = (
            target_position[0] - start.x,
            target_position[1] - start.y,
        )

        snapshot = SourceSnapshot.capture(fixture_project)
        manual_drag(window, handle, *delta, snapshot)
        expected = replace_once(
            baseline,
            geometry_source(geometry),
            geometry_source(expected_geometry),
        )
        snapshot.wait_for_applied(expected, "BoundsCases.slint")


@pytest.mark.parametrize("kind", BOUNDARY_KINDS)
@pytest.mark.parametrize("direction", BOUNDARY_MOVE_DIRECTIONS)
def test_artboard_allows_moved_element_outside(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    direction: str,
) -> None:
    run_canvas_boundary_case(
        editor_binary,
        editor_environment,
        fixture_project,
        kind,
        "move",
        direction,
    )


@pytest.mark.parametrize("kind", BOUNDARY_KINDS)
@pytest.mark.parametrize("corner", CORNERS)
def test_artboard_allows_resized_element_outside(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    corner: str,
) -> None:
    run_canvas_boundary_case(
        editor_binary,
        editor_environment,
        fixture_project,
        kind,
        "resize",
        corner,
    )


@pytest.mark.parametrize("corner", CORNERS)
@pytest.mark.parametrize("resizable", [True, False])
@pytest.mark.parametrize("outside", [False, True])
def test_rotation_starts_only_outside_resize_handle(
    editor_binary, editor_environment, fixture_project, corner, resizable, outside
):
    source = "Main.slint" if resizable else "CanvasCases.slint"
    kind = "Text" if resizable else "Rectangle"
    with launch_editor(
        editor_binary, editor_environment, fixture_project / source
    ) as editor:
        window = first_window(editor)
        if resizable:
            select_fixture_element(window, kind)
        else:
            select_outline_row(window, "layout-rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        handle = window_element_with_label(window, f"{kind} resize {corner}")
        assert handle.accessible_enabled == resizable
        offset = 1 if outside else -1
        position = center(handle)
        position = slint_testing.LogicalPosition(
            x=position.x
            + (handle.size.width / 2 + offset) * (-1 if "left" in corner else 1),
            y=position.y
            + (handle.size.height / 2 + offset) * (-1 if "top" in corner else 1),
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(position, button))
        if outside:
            window_element_with_label(window, "Rotation angle")
        else:
            assert not elements_with_label(window.root_element, "Rotation angle")
        window.dispatch_event(slint_testing.PointerReleaseEvent(position, button))
        snapshot.assert_unchanged()


@pytest.mark.parametrize("corner", CORNERS)
def test_each_rotation_zone_writes_exact_source_on_release(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    corner: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    original = b'        text: "Fixture text";'
    rotated = original + b"\n        transform-rotation: 15deg;"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Text")
        snapshot = SourceSnapshot.capture(fixture_project)
        handle = window_element_with_label(window, f"Text rotate {corner}")
        manual_rotation_drag(
            window,
            handle,
            *rotation_delta(window, handle, 15),
            snapshot,
        )
        expected = replace_once(baseline, original, rotated)
        snapshot.wait_for_exact(expected)


def test_rotation_crosses_zero_with_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    original = b'        text: "Fixture text";'
    crossing_baseline = replace_once(
        baseline, original, original + b"\n        transform-rotation: 350deg;"
    )
    source_file.write_bytes(crossing_baseline)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Text")
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_rotation_drag(
            window,
            window_element_with_label(window, "Text rotate top-left"),
            20,
            -20,
            snapshot,
            crosses_zero=True,
        )
        snapshot.wait_for_exact(
            crossing_baseline.replace(
                b"        transform-rotation: 350deg;",
                b"        transform-rotation: 0deg;",
                1,
            ),
        )


def radius_source(baseline: bytes, radii: dict[str, int]) -> bytes:
    original = b"        border-radius: 12px;"
    properties = {"border-radius": 12} | {
        f"border-{corner}-radius": radius for corner, radius in radii.items()
    }
    changed = "\n".join(
        f"        {name}: {value}px;" for name, value in sorted(properties.items())
    ).encode()
    return replace_once(baseline, original, changed)


@pytest.mark.parametrize("single", (False, True))
@pytest.mark.parametrize("corner", CORNERS)
def test_each_radius_handle_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    single: bool,
    corner: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    radii = {corner: 16} if single else {name: 16 for name in CORNERS}
    expected = radius_source(baseline, radii)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_radius_drag(
            window,
            radius_handle(window, corner),
            *RADIUS_DELTAS[corner],
            snapshot,
            shift=single,
        )
        snapshot.wait_for_applied(expected)


@pytest.mark.parametrize("single", (False, True))
@pytest.mark.parametrize("corner", CORNERS)
def test_radius_handles_follow_preview_during_drag(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    single: bool,
    corner: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        handle = radius_handle(window, corner)
        initial_positions = radius_handle_positions(window)
        start = center(handle)
        end = slint_testing.LogicalPosition(
            x=start.x + RADIUS_DELTAS[corner][0],
            y=start.y + RADIUS_DELTAS[corner][1],
        )
        button = slint_testing.PointerEventButton.Left
        snapshot = SourceSnapshot.capture(fixture_project)
        if single:
            window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))

        wait_for_radius_tooltip(window, 16)
        assert_radius_handle_positions(window, initial_positions, corner, 4, single)
        snapshot.assert_unchanged_now()

        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        if single:
            window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))


@pytest.mark.parametrize("corner", CORNERS)
def test_explicit_corner_radius_makes_every_handle_independent(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    corner: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    source_file.write_bytes(
        replace_once(
            baseline,
            b"        border-radius: 12px;",
            b"        border-radius: 12px;\n        border-top-left-radius: 12px;",
        )
    )
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        handle = radius_handle(window, corner)
        initial_positions = radius_handle_positions(window)
        start = center(handle)
        end = slint_testing.LogicalPosition(
            x=start.x + RADIUS_DELTAS[corner][0],
            y=start.y + RADIUS_DELTAS[corner][1],
        )
        button = slint_testing.PointerEventButton.Left
        snapshot = SourceSnapshot.capture(fixture_project)
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))

        wait_for_radius_tooltip(window, 16)
        assert_radius_handle_positions(window, initial_positions, corner, 4, True)
        snapshot.assert_unchanged_now()

        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))


@pytest.mark.parametrize(
    ("shift_before_press", "shift_during_drag", "single_corner"),
    [
        pytest.param(False, True, False, id="press-shift-after-pointer-press"),
        pytest.param(True, False, True, id="release-shift-after-pointer-press"),
    ],
)
def test_radius_drag_mode_does_not_change_after_pointer_press(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    shift_before_press: bool,
    shift_during_drag: bool,
    single_corner: bool,
) -> None:
    source_file = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        corner = "top-left"
        handle = radius_handle(window, corner)
        initial_positions = radius_handle_positions(window)
        start = center(handle)
        end = slint_testing.LogicalPosition(
            x=start.x + RADIUS_DELTAS[corner][0],
            y=start.y + RADIUS_DELTAS[corner][1],
        )
        button = slint_testing.PointerEventButton.Left
        snapshot = SourceSnapshot.capture(fixture_project)
        if shift_before_press:
            window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        if shift_during_drag:
            window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
        else:
            window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))

        wait_for_radius_tooltip(window, 16)
        assert_radius_handle_positions(
            window, initial_positions, corner, 4, single_corner
        )
        snapshot.assert_unchanged_now()

        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        if shift_during_drag:
            window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))


def test_clamped_radius_drag_keeps_handles_aligned_with_preview(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        corner = "top-left"
        handle = radius_handle(window, corner)
        initial_positions = radius_handle_positions(window)
        start = center(handle)
        end = slint_testing.LogicalPosition(x=start.x + 100, y=start.y + 100)
        button = slint_testing.PointerEventButton.Left
        snapshot = SourceSnapshot.capture(fixture_project)
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))

        wait_for_radius_tooltip(window, 60)
        assert_radius_handle_positions(window, initial_positions, corner, 48, False)
        snapshot.assert_unchanged_now()

        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))


def test_repeated_rectangles_show_radius_handles_only_on_primary_instance(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    repeated_source = replace_once(
        source_file.read_bytes(),
        b"    root-rectangle := Rectangle {\n        x: 40px;\n        y: 40px;",
        b"    for index in [0, 1, 2]: root-rectangle := Rectangle {\n"
        b"        x: 20px + index * 120px;\n        y: 400px;",
    )
    repeated_source = replace_once(
        repeated_source,
        b"        width: 180px;\n        height: 120px;",
        b"        width: 100px;\n        height: 80px;",
    )
    source_file.write_bytes(repeated_source)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        instances = wait_until(
            lambda: (
                elements
                if len(elements := window.find_elements_by_id("Main::root-rectangle"))
                == 3
                else None
            )
        )
        target = center(instances[1])
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(target))
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        wait_until(
            lambda: (
                selected
                if len(
                    selected := elements_with_label(
                        window.root_element,
                        "Selected Rectangle",
                        slint_testing.AccessibleRole.Region,
                    )
                )
                == 3
                else None
            )
        )

        for corner in CORNERS:
            handles = elements_with_label(
                window.root_element,
                f"Rectangle radius {corner}",
                slint_testing.AccessibleRole.Button,
            )
            assert len(handles) == 1
            assert position_distance(center(handles[0]), target) < 100
        snapshot.assert_unchanged()


def test_radius_is_clamped_to_half_the_shortest_side(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    expected = radius_source(baseline, {name: 60 for name in CORNERS})
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_radius_drag(
            window,
            radius_handle(window, "top-left"),
            100,
            100,
            snapshot,
        )
        snapshot.wait_for_applied(expected)


def test_resize_continues_outside_window_and_commits_on_release(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        handle = window_element_with_label(window, "Rectangle resize bottom-right")
        start = center(handle)
        size = window.root_element.size
        dx = math.ceil(size.width + OUTSIDE_ARTBOARD_DISTANCE - start.x)
        dy = math.ceil(size.height + OUTSIDE_ARTBOARD_DISTANCE - start.y)
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_drag(window, handle, dx, dy, snapshot)
        expected = replace_once(
            baseline,
            b"        width: 180px;\n        height: 120px;",
            f"        width: {180 + dx}px;\n        height: {120 + dy}px;".encode(),
        )
        snapshot.wait_for_applied(expected)


def test_rotation_continues_outside_window_and_commits_on_release(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Text")
        handle = window_element_with_label(window, "Text rotate top-left")
        start = rotation_start(handle)
        x, y, width, height = selection_frame(window, "Text")
        cx, cy = x + width / 2, y + height / 2
        vx, vy = start.x - cx, start.y - cy
        size = window.root_element.size
        scale = 4 * max(size.width, size.height) / math.hypot(vx, vy)
        snapshot = SourceSnapshot.capture(fixture_project)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
        target = start
        for degrees in (15, 30):
            angle = math.radians(degrees)
            target = slint_testing.LogicalPosition(
                x=cx + (vx * math.cos(angle) - vy * math.sin(angle)) * scale,
                y=cy + (vx * math.sin(angle) + vy * math.cos(angle)) * scale,
            )
            assert target.x < 0 or target.y < 0
            window.dispatch_event(slint_testing.PointerMoveEvent(target))
            wait_until(
                lambda degrees=degrees: (
                    True
                    if abs(math.degrees(frame_rotation(window, "Text")) - degrees) < 1
                    else None
                )
            )
            snapshot.assert_unchanged()
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
        original = b'        text: "Fixture text";'
        expected = replace_once(
            baseline, original, original + b"\n        transform-rotation: 30deg;"
        )
        snapshot.wait_for_applied(expected)


def test_radius_drag_continues_outside_window_and_commits_on_release(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        handle = radius_handle(window, "top-left")
        start = center(handle)
        size = window.root_element.size
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_radius_drag(
            window,
            handle,
            size.width + OUTSIDE_ARTBOARD_DISTANCE - start.x,
            size.height + OUTSIDE_ARTBOARD_DISTANCE - start.y,
            snapshot,
        )
        expected = radius_source(baseline, {corner: 60 for corner in CORNERS})
        snapshot.wait_for_applied(expected)


@pytest.mark.parametrize("label", THRESHOLD_LABELS)
def test_handle_click_below_drag_threshold_does_not_edit_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    label: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        handle = (
            radius_handle(window, "top-left")
            if label == "Rectangle radius top-left"
            else window_element_with_label(window, label)
        )
        before = selection_frame(window, "Rectangle")
        start = center(handle)
        radius_position_before = start if label == "Rectangle radius top-left" else None
        end = slint_testing.LogicalPosition(x=start.x + 1, y=start.y + 1)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        assert same_state(selection_frame(window, "Rectangle"), before)
        if radius_position_before is not None:
            assert (
                position_distance(
                    center(
                        window_element_with_label(
                            window, label, slint_testing.AccessibleRole.Button
                        )
                    ),
                    radius_position_before,
                )
                < 1.5
            )
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        snapshot.assert_unchanged()


@pytest.mark.parametrize("element_id", DISABLED_IDS)
@pytest.mark.parametrize("corner", CORNERS)
def test_disabled_manipulation_does_not_edit_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    element_id: str,
    corner: str,
) -> None:
    source_file = fixture_project / "CanvasCases.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_outline_row(window, element_id)
        window_element_with_label(window, "Selected Rectangle")
        handle = window_element_with_label(window, f"Rectangle resize {corner}")
        assert not handle.accessible_enabled
        target = center(handle)
        window.drag_and_drop(
            target,
            slint_testing.LogicalPosition(x=target.x + 20, y=target.y + 16),
        )
        snapshot.assert_unchanged()
