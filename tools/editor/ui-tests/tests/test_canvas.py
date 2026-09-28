# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# ruff: noqa: I001

import math
from pathlib import Path

import slint_testing
from canvas_interactions import (
    center,
    manual_drag,
    position_distance,
    selection_frame,
)
from slint_test import Window, expect
from source_snapshot import SourceSnapshot, replace_once
from ui_driver import (
    screenshot,
)

GOLDENS = Path(__file__).resolve().parents[1] / "goldens"
CORNERS = ["top-left", "top-right", "bottom-right", "bottom-left"]
EDGES = ["top", "right", "bottom", "left"]
OPPOSITE_EDGES = {"top": "bottom", "right": "left", "bottom": "top", "left": "right"}
EDGE_DELTAS = {"top": (0, -16), "right": (20, 0), "bottom": (0, 16), "left": (-20, 0)}
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
ROTATED_GEOMETRIES = {
    "Rectangle": (64, 56, 140, 96),
    "Text": (160, 64, 180, 56),
    "Image": (200, 208, 144, 96),
}
# The rotation the elements of RotatedCanvasCases.slint carry.
ROTATED_FIXTURE_ANGLE = 30
BOUNDARY_KINDS = ("Rectangle", "Text", "Image")
BOUNDARY_MOVE_DIRECTIONS = ("top-left", "bottom-right")
OUTSIDE_ARTBOARD_DISTANCE = 32
BOUNDS_WIDTH = 388
BOUNDS_HEIGHT = 718
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
SELECTION_ACCENT = (11, 153, 254)


def selection_outline_pixel_count(window: Window, kind: str) -> int:
    x, y, width, height = selection_frame(window, kind)
    image = screenshot(window)
    scale = image.width / window.root_element.size.width
    padding = round(3 * scale)
    bounds = (
        round(x * scale) - padding,
        round(y * scale) - padding,
        round((x + width) * scale) + padding,
        round((y + height) * scale) + padding,
    )
    outline = image.crop(bounds)
    return sum(pixel == SELECTION_ACCENT for pixel in outline.get_flattened_data())


def finish_palette_drag(window: Window, target: slint_testing.LogicalPosition) -> None:
    window.pointer.release_at(target)


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


def rotated_edge_resize_values(
    geometry: tuple[int, int, int, int], edge: str, dx: float, dy: float
) -> tuple[int, int, int, int]:
    angle = math.radians(ROTATED_FIXTURE_ANGLE)
    local_dx = dx * math.cos(angle) + dy * math.sin(angle)
    local_dy = -dx * math.sin(angle) + dy * math.cos(angle)
    if edge in ("left", "right"):
        local_dy = 0
    else:
        local_dx = 0
    projected_dx = local_dx * math.cos(angle) - local_dy * math.sin(angle)
    projected_dy = local_dx * math.sin(angle) + local_dy * math.cos(angle)
    corner = {
        "top": "top-left",
        "right": "top-right",
        "bottom": "bottom-left",
        "left": "top-left",
    }[edge]
    return rotated_resize_values(*geometry, corner, projected_dx, projected_dy)


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
    window: Window,
) -> dict[str, slint_testing.LogicalPosition]:
    return {
        corner: center(
            window.get_by_accessible_name(f"Rectangle radius {corner}").resolve()
        )
        for corner in CORNERS
    }


def assert_radius_handle_positions(
    window: Window,
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


def wait_for_radius_tooltip(window: Window, radius: float) -> None:
    tooltip = window.get_by_role("text", name="Radius value")
    expect.poll(
        lambda: float(tooltip.value()),
        session=window.session,
        message="radius tooltip value",
    ).to_equal(radius)


def run_canvas_boundary_case(
    editor_factory,
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
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.outline.select(element_id)

        geometry = geometries[kind]
        artboard = window.get_by_role("region", name="Artboard").resolve()
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
        handle = window.get_by_accessible_name(label).resolve()
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


def radius_source(baseline: bytes, radii: dict[str, int]) -> bytes:
    original = b"        border-radius: 12px;"
    properties = {"border-radius": 12} | {
        f"border-{corner}-radius": radius for corner, radius in radii.items()
    }
    changed = "\n".join(
        f"        {name}: {value}px;" for name, value in sorted(properties.items())
    ).encode()
    return replace_once(baseline, original, changed)


from canvas_palette_cases import (  # noqa: F401
    test_component_palette_preserves_compact_row_layout,
    test_component_palette_drop_can_extend_outside_artboard,
    test_palette_preview_follows_rejected_pointer,
    test_unselected_element_shows_hover_outline_without_side_effects,
    test_overlapping_elements_update_hover_outline_to_topmost_item,
    test_overlapping_hover_does_not_intercept_selected_element_drag,
    test_hover_outside_selected_element_can_select_and_drag_child,
    test_selected_element_shows_hover_outline,
    test_unselected_element_click_selects_without_editing_source,
    test_unselected_element_can_be_dragged_in_one_gesture,
    test_unselected_rectangle_direct_drag_starts_before_release,
    test_invisible_element_keeps_selection_outline_during_drag,
    test_repeated_palette_drop_preserves_component_kind,
    test_component_palette_drag_over_rotated_element_does_not_crash,
    test_image_asset_mode_destroys_canvas_without_replaying_palette_drop,
    test_move_element_writes_exact_source_on_release,
)

from canvas_geometry_cases import (  # noqa: F401
    test_move_rotated_element_writes_exact_source_on_release,
    test_nested_rotated_element_move_writes_exact_local_source,
    test_each_resize_handle_writes_exact_source_on_release,
    test_each_edge_resizes_only_its_axis,
    test_shift_edge_resize_stays_single_axis,
    test_edge_click_and_tangential_drag_do_not_edit,
    test_layout_selection_has_no_edge_resize_controls,
    test_small_selection_keeps_corners_instead_of_short_edges,
    test_shift_resize_is_proportional,
    test_rotated_element_resize_writes_exact_source,
    test_rotated_element_edge_resize_writes_exact_source,
    test_artboard_allows_moved_element_outside,
    test_artboard_allows_resized_element_outside,
    test_rotation_starts_only_outside_resize_handle,
    test_each_rotation_zone_writes_exact_source_on_release,
)

from canvas_rotation_cases import (  # noqa: F401
    test_rotation_crosses_zero_with_exact_source,
    test_zero_radius_handles_stay_inside_and_drag_back_to_zero,
)

from canvas_radius_cases import (  # noqa: F401
    test_each_radius_handle_writes_exact_source,
    test_radius_handles_follow_preview_during_drag,
    test_radius_tooltip_follows_handle_clear_of_corner,
    test_explicit_corner_radius_makes_every_handle_independent,
    test_radius_drag_mode_does_not_change_after_pointer_press,
    test_clamped_radius_drag_keeps_handles_aligned_with_preview,
    test_repeated_rectangles_show_radius_handles_only_on_primary_instance,
    test_radius_is_clamped_to_half_the_shortest_side,
    test_resize_continues_outside_window_and_commits_on_release,
    test_rotation_continues_outside_window_and_commits_on_release,
    test_radius_drag_continues_outside_window_and_commits_on_release,
    test_handle_click_below_drag_threshold_does_not_edit_source,
    test_disabled_manipulation_does_not_edit_source,
    test_resize_modifier_changes_during_drag,
)
