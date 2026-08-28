# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import (
    cancel_pointer_interaction,
    center,
    live_modifier_resize,
    manual_drag,
    manual_radius_drag,
    manual_rotation_drag,
    position_distance,
    rotation_delta,
    same_state,
    selection_frame,
)
from source_snapshot import SourceSnapshot
from ui_driver import (
    elements_with_label,
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
RUST_FIX_REQUIRED = pytest.mark.skip(
    reason="Requires a Rust visual editor behavior fix"
)


def replace_once(source: bytes, old: bytes, new: bytes) -> bytes:
    assert source.count(old) == 1
    return source.replace(old, new, 1)


def wait_for_source_change(source_file: Path, baseline: bytes) -> bytes:
    def changed_source() -> bytes | None:
        source = source_file.read_bytes()
        return source if source != baseline else None

    return wait_until(changed_source)


def begin_palette_drag(
    window: slint_testing.Window,
    kind: str,
    target: slint_testing.LogicalPosition,
) -> None:
    def leftmost_enabled_row() -> slint_testing.Element | None:
        rows = [
            row
            for row in elements_with_label(
                window.root_element,
                kind,
                slint_testing.AccessibleRole.ListItem,
            )
            if row.accessible_enabled
        ]
        return min(rows, key=lambda row: row.absolute_position.x) if rows else None

    palette_row = wait_until(leftmost_enabled_row)
    start = center(palette_row)
    button = slint_testing.PointerEventButton.Left
    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    window.dispatch_event(
        slint_testing.PointerMoveEvent(
            slint_testing.LogicalPosition(x=start.x + 16, y=start.y + 16)
        )
    )
    window.dispatch_event(slint_testing.PointerMoveEvent(target))


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
        rows = [
            window_element_with_label(
                window, kind, slint_testing.AccessibleRole.ListItem
            )
            for kind in PALETTE_DROP_SIZES
        ]

        assert section.absolute_position.y + section.size.height + 8 == pytest.approx(
            rows[0].absolute_position.y
        )
        assert all(row.size.height == pytest.approx(62) for row in rows)
        assert all(
            row.absolute_position.x == rows[0].absolute_position.x for row in rows
        )
        assert all(row.size.width == rows[0].size.width for row in rows)
        assert rows[1].absolute_position.y - rows[
            0
        ].absolute_position.y == pytest.approx(70)
        assert rows[2].absolute_position.y - rows[
            1
        ].absolute_position.y == pytest.approx(70)


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
            snapshot.wait_for_exact(expected, "RepeatedPaletteDrops.slint")
            reload_label = f"Reload probe {step}"
            source_file.write_bytes(
                expected.replace(b"Reload probe", reload_label.encode(), 1)
            )
            window_element_with_label(
                window, reload_label, slint_testing.AccessibleRole.Text
            )
            source_file.write_bytes(expected)
            window_element_with_label(
                window, "Reload probe", slint_testing.AccessibleRole.Text
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
    source_file.write_bytes(baseline)
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

        asset_directory_row = window_element_with_label(
            window, str(asset_directory), slint_testing.AccessibleRole.ListItem
        )
        asset_directory_row.single_click(slint_testing.PointerEventButton.Left)
        image_row = window_element_with_label(
            window, str(image_file), slint_testing.AccessibleRole.ListItem
        )
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

        component_row = window_element_with_label(
            window, str(source_file), slint_testing.AccessibleRole.ListItem
        )
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
    angle_degrees: float = 30,
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


def radius_handle(window: slint_testing.Window, corner: str) -> slint_testing.Element:
    selection = window_element_with_label(
        window, "Selected Rectangle", slint_testing.AccessibleRole.Region
    )
    # A live reload can replace the frame while the pointer remains at the same logical
    # position. Move away first so the real frame receives a fresh hover transition.
    window.dispatch_event(
        slint_testing.PointerMoveEvent(slint_testing.LogicalPosition(x=1, y=1))
    )
    window.dispatch_event(slint_testing.PointerMoveEvent(center(selection)))
    return window_element_with_label(window, f"Rectangle radius {corner}")


@pytest.mark.parametrize(
    "kind",
    [pytest.param("Rectangle", marks=RUST_FIX_REQUIRED), "Text", "Image"],
)
def test_move_element_writes_exact_source_on_release(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    positions = {
        "Rectangle": (
            b"        x: 40px;\n        y: 40px;",
            b"        x: 60px;\n        y: 56px;",
        ),
        "Text": (
            b"        x: 180px;\n        y: 56px;",
            b"        x: 200px;\n        y: 72px;",
        ),
        "Image": (
            b"        x: 230px;\n        y: 132px;",
            b"        x: 250px;\n        y: 148px;",
        ),
    }
    source_file.write_bytes(baseline)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_fixture_element(window, kind)
        manual_drag(
            window,
            window_element_with_label(window, f"{kind} move handle"),
            20,
            16,
            snapshot,
        )
        expected = replace_once(baseline, *positions[kind])
        snapshot.wait_for_exact(expected)


@RUST_FIX_REQUIRED
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
        "Rectangle": (
            b"        x: 64px;\n        y: 56px;",
            b"        x: 84px;\n        y: 72px;",
        ),
        "Text": (
            b"        x: 160px;\n        y: 64px;",
            b"        x: 180px;\n        y: 80px;",
        ),
        "Image": (
            b"        x: 200px;\n        y: 208px;",
            b"        x: 220px;\n        y: 224px;",
        ),
    }

    source_file.write_bytes(baseline)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_outline_row(window, f"rotated-free-{kind.lower()}")
        manual_drag(
            window,
            window_element_with_label(window, f"{kind} move handle"),
            20,
            16,
            snapshot,
        )
        expected = replace_once(baseline, *positions[kind])
        snapshot.wait_for_exact(expected, "RotatedCanvasCases.slint")


@RUST_FIX_REQUIRED
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
    original = b"        x: 40px;\n        y: 40px;\n        width: 180px;\n        height: 120px;"
    geometries = {
        "top-left": b"        x: 20px;\n        y: 24px;\n        width: 200px;\n        height: 136px;",
        "top-right": b"        x: 40px;\n        y: 24px;\n        width: 200px;\n        height: 136px;",
        "bottom-right": b"        x: 40px;\n        y: 40px;\n        width: 200px;\n        height: 136px;",
        "bottom-left": b"        x: 20px;\n        y: 40px;\n        width: 200px;\n        height: 136px;",
    }
    source_file.write_bytes(baseline)
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
        expected = replace_once(baseline, original, geometries[corner])
        snapshot.wait_for_exact(expected)


def test_shift_resize_is_proportional(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    source_file.write_bytes(baseline)
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
    source_file.write_bytes(baseline)
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


@RUST_FIX_REQUIRED
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

    x, y, width, height = geometries[kind]
    original = (
        f"        x: {x}px;\n"
        f"        y: {y}px;\n"
        f"        width: {width}px;\n"
        f"        height: {height}px;"
    ).encode()
    source_file.write_bytes(baseline)
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
        new_x, new_y, new_width, new_height = rotated_resize_values(
            x,
            y,
            width,
            height,
            corner,
            *CORNER_DELTAS[corner],
        )
        changed = (
            f"        x: {new_x}px;\n"
            f"        y: {new_y}px;\n"
            f"        width: {new_width}px;\n"
            f"        height: {new_height}px;"
        ).encode()
        snapshot.wait_for_exact(
            replace_once(baseline, original, changed),
            "RotatedCanvasCases.slint",
        )
        assert (
            position_distance(
                center(window_element_with_label(window, opposite_label)),
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
    source_file.write_bytes(baseline)
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
            desired_pointer = (
                (
                    left - OUTSIDE_ARTBOARD_DISTANCE,
                    top - OUTSIDE_ARTBOARD_DISTANCE,
                )
                if target == "top-left"
                else (
                    right + OUTSIDE_ARTBOARD_DISTANCE,
                    bottom + OUTSIDE_ARTBOARD_DISTANCE,
                )
            )
            start = center(window_element_with_label(window, label))
            delta = (
                desired_pointer[0] - start.x,
                desired_pointer[1] - start.y,
            )
            expected_geometry = outside_move_values(geometry, target)
        else:
            label = f"{kind} resize {target}"
            handle = window_element_with_label(window, label)
            start = center(handle)
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
            delta = (
                target_position[0] - start.x,
                target_position[1] - start.y,
            )
            expected_geometry = outside_resize_values(geometry, target)

        snapshot = SourceSnapshot.capture(fixture_project)
        manual_drag(
            window,
            window_element_with_label(window, label),
            *delta,
            snapshot,
            require_multiple_transient_states=False,
        )
        expected = replace_once(
            baseline,
            geometry_source(geometry),
            geometry_source(expected_geometry),
        )
        snapshot.wait_for_exact(expected, "BoundsCases.slint")


@pytest.mark.parametrize(
    ("kind", "direction"),
    [
        pytest.param(
            kind,
            direction,
            marks=RUST_FIX_REQUIRED if kind == "Text" else (),
            id=f"{kind}-{direction}",
        )
        for kind in BOUNDARY_KINDS
        for direction in BOUNDARY_MOVE_DIRECTIONS
    ],
)
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
    source_file.write_bytes(baseline)
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
    properties = {"border-radius": 12}
    properties.update(
        {f"border-{corner}-radius": radius for corner, radius in radii.items()}
    )
    changed = "\n".join(
        f"        {name}: {value}px;" for name, value in sorted(properties.items())
    ).encode()
    return replace_once(baseline, original, changed)


@RUST_FIX_REQUIRED
@pytest.mark.parametrize(
    ("single", "corner"),
    [
        pytest.param(single, corner, id=f"{single}-{corner}")
        for single in (False, True)
        for corner in CORNERS
    ],
)
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
    source_file.write_bytes(baseline)
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
        snapshot.wait_for_exact(expected)


@RUST_FIX_REQUIRED
def test_radius_is_clamped_to_half_the_shortest_side(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    expected = radius_source(baseline, {name: 60 for name in CORNERS})
    source_file.write_bytes(baseline)
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
        snapshot.wait_for_exact(expected)


@RUST_FIX_REQUIRED
@pytest.mark.parametrize(
    ("kind", "label", "delta"),
    [
        pytest.param(
            "Rectangle", "Rectangle resize bottom-right", (20, 16), id="resize"
        ),
        pytest.param("Text", "Text rotate top-left", (20, -20), id="rotation"),
        pytest.param(
            "Rectangle-radius", "Rectangle radius top-left", (4, 4), id="radius"
        ),
    ],
)
def test_pointer_exit_cancels_interaction_and_allows_recovery(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    label: str,
    delta: tuple[int, int],
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    source_file.write_bytes(baseline)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_fixture_element(window, "Text" if kind == "Text" else "Rectangle")
        handle = (
            radius_handle(window, "top-left")
            if kind == "Rectangle-radius"
            else window_element_with_label(window, label)
        )
        snapshot = SourceSnapshot.capture(fixture_project)
        cancel_pointer_interaction(window, handle, *delta, snapshot, kind)
        if kind == "Rectangle":
            manual_drag(
                window,
                window_element_with_label(window, label),
                *delta,
                snapshot,
            )
            expected = replace_once(
                baseline,
                b"        width: 180px;\n        height: 120px;",
                b"        width: 200px;\n        height: 136px;",
            )
        elif kind == "Text":
            manual_rotation_drag(
                window,
                window_element_with_label(window, label),
                *delta,
                snapshot,
            )
            expected = replace_once(
                baseline,
                b'        text: "Fixture text";',
                b'        text: "Fixture text";\n        transform-rotation: 15deg;',
            )
        else:
            manual_radius_drag(
                window,
                radius_handle(window, "top-left"),
                *delta,
                snapshot,
            )
            expected = radius_source(baseline, {corner: 16 for corner in CORNERS})
        snapshot.wait_for_exact(expected)


@pytest.mark.parametrize("label", THRESHOLD_LABELS)
def test_handle_click_below_drag_threshold_does_not_edit_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    label: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    source_file.write_bytes(baseline)
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
        end = slint_testing.LogicalPosition(x=start.x + 1, y=start.y + 1)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        assert same_state(selection_frame(window, "Rectangle"), before)
        snapshot.assert_unchanged()


@pytest.mark.parametrize("element_id", DISABLED_IDS)
def test_disabled_manipulation_does_not_edit_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    element_id: str,
) -> None:
    source_file = fixture_project / "CanvasCases.slint"
    baseline = source_file.read_bytes()
    source_file.write_bytes(baseline)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        snapshot = SourceSnapshot.capture(fixture_project)
        select_outline_row(window, element_id)
        window_element_with_label(window, "Selected Rectangle")
        handle = window_element_with_label(window, "Rectangle resize bottom-right")
        assert not handle.accessible_enabled
        target = center(handle)
        window.drag_and_drop(
            target,
            slint_testing.LogicalPosition(x=target.x + 20, y=target.y + 16),
        )
        snapshot.assert_unchanged()
