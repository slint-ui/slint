# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import (
    center,
    manual_drag,
    manual_rotation_drag,
    position_distance,
    rotation_delta,
    selection_frame,
)
from slint_test import step
from source_snapshot import SourceSnapshot, replace_once
from test_canvas import (
    BOUNDARY_KINDS,
    BOUNDARY_MOVE_DIRECTIONS,
    CORNER_DELTAS,
    CORNERS,
    EDGE_DELTAS,
    EDGES,
    MOVE_KINDS,
    OPPOSITE_CORNERS,
    OPPOSITE_EDGES,
    ROTATED_FIXTURE_ANGLE,
    ROTATED_GEOMETRIES,
    ROTATED_KINDS,
    geometry_source,
    rotated_edge_resize_values,
    rotated_resize_values,
    run_canvas_boundary_case,
)


@pytest.mark.parametrize("kind", MOVE_KINDS)
def test_move_rotated_element_writes_exact_source_on_release(
    editor_factory,
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
    with editor_factory(source_file) as editor:
        window = editor.window
        snapshot = SourceSnapshot.capture(fixture_project)
        editor.outline.select(f"rotated-free-{kind.lower()}")
        manual_drag(
            window,
            window.get_by_accessible_name(f"{kind} move handle").resolve(),
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
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "RotatedCanvasCases.slint"
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source_file) as editor:
        element = editor.canvas.element("nested-rotated-text", kind="Text")
        element.select()
        before = element.selection.bounds()
        with element.drag() as drag:
            previews = []
            for fraction in (1 / 3, 2 / 3, 1):
                drag.move_by(20 * fraction, 16 * fraction, origin="start")
                previews.append(element.selection.bounds())
            with step("Move preview changes before source commit", layer="assertion"):
                assert previews[-1] != before
                assert len(set(previews)) >= 2
                snapshot.assert_unchanged_now()
            drag.release()
        with step("Window movement saves exact local coordinates", layer="assertion"):
            snapshot.wait_for_exact(
                replace_once(
                    baseline,
                    b"                x: 44px;\n                y: 52px;",
                    b"                x: 60px;\n                y: 32px;",
                ),
                "RotatedCanvasCases.slint",
            )


@pytest.mark.parametrize("corner", CORNERS)
def test_each_resize_handle_writes_exact_source_on_release(
    editor_factory,
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
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_drag(
            window,
            window.get_by_accessible_name(f"Rectangle resize {corner}").resolve(),
            *CORNER_DELTAS[corner],
            snapshot,
        )
        expected = replace_once(baseline, original, geometry_source(geometries[corner]))
        snapshot.wait_for_exact(expected)


@pytest.mark.parametrize("edge", EDGES)
def test_each_edge_resizes_only_its_axis(
    editor_factory,
    fixture_project: Path,
    edge: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    original = geometry_source((40, 40, 180, 120))
    geometries = {
        "top": (40, 24, 180, 136),
        "right": (40, 40, 200, 120),
        "bottom": (40, 40, 180, 136),
        "left": (20, 40, 200, 120),
    }
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        fixed_label = f"Rectangle resize {OPPOSITE_EDGES[edge]}"
        fixed_center = manual_drag(
            window,
            window.get_by_accessible_name(f"Rectangle resize {edge}").resolve(),
            *EDGE_DELTAS[edge],
            snapshot,
            fixed_handle_label=fixed_label,
        )
        snapshot.wait_for_applied(
            replace_once(baseline, original, geometry_source(geometries[edge]))
        )
        assert fixed_center is not None
        assert (
            position_distance(
                center(window.get_by_accessible_name(fixed_label).resolve()),
                fixed_center,
            )
            < 1.5
        )


def test_shift_edge_resize_stays_single_axis(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_drag(
            window,
            window.get_by_accessible_name("Rectangle resize right").resolve(),
            20,
            0,
            snapshot,
            shift=True,
        )
        snapshot.wait_for_applied(
            replace_once(
                baseline,
                geometry_source((40, 40, 180, 120)),
                geometry_source((40, 40, 200, 120)),
            )
        )


@pytest.mark.parametrize("edge", ("top", "right"))
@pytest.mark.parametrize("rotated", (False, True))
def test_edge_click_and_tangential_drag_do_not_edit(
    editor_factory,
    fixture_project: Path,
    edge: str,
    rotated: bool,
) -> None:
    source_file = fixture_project / (
        "RotatedCanvasCases.slint" if rotated else "Main.slint"
    )
    angle = math.radians(ROTATED_FIXTURE_ANGLE if rotated else 0)
    with editor_factory(source_file) as editor:
        window = editor.window
        if rotated:
            editor.outline.select("rotated-free-rectangle")
        else:
            editor.canvas.select("Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        handle = window.get_by_accessible_name(f"Rectangle resize {edge}").resolve()
        start = center(handle, angle)
        initial_frame = selection_frame(window, "Rectangle")
        window.pointer.press_at(start)
        window.pointer.release_at(start)
        assert selection_frame(window, "Rectangle") == initial_frame
        snapshot.assert_unchanged()

        normal = (
            (-1.5 * math.sin(angle), 1.5 * math.cos(angle))
            if edge == "top"
            else (1.5 * math.cos(angle), 1.5 * math.sin(angle))
        )
        near = slint_testing.LogicalPosition(
            x=start.x + normal[0], y=start.y + normal[1]
        )
        window.pointer.press_at(start)
        window.pointer.move_to(near)
        window.pointer.release_at(near)
        assert selection_frame(window, "Rectangle") == initial_frame
        snapshot.assert_unchanged()

        tangent = (
            (20 * math.cos(angle), 20 * math.sin(angle))
            if edge == "top"
            else (-20 * math.sin(angle), 20 * math.cos(angle))
        )
        end = slint_testing.LogicalPosition(
            x=start.x + tangent[0], y=start.y + tangent[1]
        )
        window.pointer.move_to(start)
        window.pointer.press_at(start)
        window.pointer.move_to(end)
        assert selection_frame(window, "Rectangle") == initial_frame
        snapshot.assert_unchanged_now()
        window.pointer.release_at(end)
        snapshot.assert_unchanged()


def test_layout_selection_has_no_edge_resize_controls(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "CanvasCases.slint"
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.outline.select("layout-rectangle")
        for edge in EDGES:
            window.get_by_accessible_name(f"Rectangle resize {edge}").wait_for(
                state="hidden"
            )


@pytest.mark.parametrize(
    ("size", "hidden_edges"),
    [((10, 120), ("top", "bottom")), ((180, 10), ("left", "right"))],
)
def test_small_selection_keeps_corners_instead_of_short_edges(
    editor_factory,
    fixture_project: Path,
    size: tuple[int, int],
    hidden_edges: tuple[str, str],
) -> None:
    source_file = fixture_project / "Main.slint"
    source_file.write_bytes(
        replace_once(
            source_file.read_bytes(),
            geometry_source((40, 40, 180, 120)),
            geometry_source((40, 40, *size)),
        )
    )
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.outline.select("root-rectangle")
        for edge in hidden_edges:
            window.get_by_accessible_name(f"Rectangle resize {edge}").wait_for(
                state="hidden"
            )
        for corner in CORNERS:
            window.get_by_accessible_name(f"Rectangle resize {corner}").wait_for()

        baseline = source_file.read_bytes()
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_drag(
            window,
            window.get_by_accessible_name("Rectangle resize top-left").resolve(),
            -16,
            -16,
            snapshot,
            fixed_handle_label="Rectangle resize bottom-right",
        )
        snapshot.wait_for_applied(
            replace_once(
                baseline,
                geometry_source((40, 40, *size)),
                geometry_source((24, 24, size[0] + 16, size[1] + 16)),
            )
        )


def test_shift_resize_is_proportional(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        manual_drag(
            window,
            window.get_by_accessible_name("Rectangle resize bottom-right").resolve(),
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


@pytest.mark.parametrize("kind", ROTATED_KINDS)
@pytest.mark.parametrize("corner", CORNERS)
def test_rotated_element_resize_writes_exact_source(
    editor_factory,
    fixture_project: Path,
    kind: str,
    corner: str,
) -> None:
    source_file = fixture_project / "RotatedCanvasCases.slint"
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    geometry = ROTATED_GEOMETRIES[kind]
    dx, dy = CORNER_DELTAS[corner]
    opposite = f"resize {OPPOSITE_CORNERS[corner]}"
    with editor_factory(source_file) as editor:
        element = editor.canvas.element(f"rotated-free-{kind.lower()}", kind=kind)
        element.select()
        before = element.selection.bounds()
        fixed = element.handle_center(opposite)
        with element.drag(f"resize {corner}") as drag:
            previews = []
            for fraction in (1 / 3, 2 / 3, 1):
                drag.move_by(dx * fraction, dy * fraction, origin="start")
                previews.append(element.selection.bounds())
            with step(
                "Resize preview follows pointer without moving opposite corner",
                layer="assertion",
            ):
                assert previews[-1] != before
                assert len(set(previews)) >= 2
                assert (
                    position_distance(
                        element.handle_center(f"resize {corner}"),
                        editor.window.pointer.position,
                    )
                    < 1.5
                )
                assert position_distance(element.handle_center(opposite), fixed) < 1.5
                snapshot.assert_unchanged_now()
            drag.release()
        with step(
            "Release saves exact geometry and applies preview", layer="assertion"
        ):
            expected = rotated_resize_values(*geometry, corner, dx, dy)
            snapshot.wait_for_applied(
                replace_once(
                    baseline, geometry_source(geometry), geometry_source(expected)
                ),
                "RotatedCanvasCases.slint",
            )
            assert position_distance(element.handle_center(opposite), fixed) < 1.5


@pytest.mark.parametrize("kind", ROTATED_KINDS)
@pytest.mark.parametrize("edge", EDGES)
def test_rotated_element_edge_resize_writes_exact_source(
    editor_factory,
    fixture_project: Path,
    kind: str,
    edge: str,
) -> None:
    source_file = fixture_project / "RotatedCanvasCases.slint"
    baseline = source_file.read_bytes()
    geometry = ROTATED_GEOMETRIES[kind]
    dx, dy = EDGE_DELTAS[edge]
    with editor_factory(source_file) as editor:
        window = editor.window
        snapshot = SourceSnapshot.capture(fixture_project)
        editor.outline.select(f"rotated-free-{kind.lower()}")
        fixed_label = f"{kind} resize {OPPOSITE_EDGES[edge]}"
        fixed_center = manual_drag(
            window,
            window.get_by_accessible_name(f"{kind} resize {edge}").resolve(),
            dx,
            dy,
            snapshot,
            fixed_handle_label=fixed_label,
            follow_pointer=False,
        )
        expected_geometry = rotated_edge_resize_values(geometry, edge, dx, dy)
        snapshot.wait_for_applied(
            replace_once(
                baseline, geometry_source(geometry), geometry_source(expected_geometry)
            ),
            "RotatedCanvasCases.slint",
        )
        assert fixed_center is not None
        assert (
            position_distance(
                center(
                    window.get_by_accessible_name(fixed_label).resolve(),
                    math.radians(ROTATED_FIXTURE_ANGLE),
                ),
                fixed_center,
            )
            < 1.5
        )


@pytest.mark.parametrize("kind", BOUNDARY_KINDS)
@pytest.mark.parametrize("direction", BOUNDARY_MOVE_DIRECTIONS)
def test_artboard_allows_moved_element_outside(
    editor_factory,
    fixture_project: Path,
    kind: str,
    direction: str,
) -> None:
    run_canvas_boundary_case(
        editor_factory,
        fixture_project,
        kind,
        "move",
        direction,
    )


@pytest.mark.parametrize("kind", BOUNDARY_KINDS)
@pytest.mark.parametrize("corner", CORNERS)
def test_artboard_allows_resized_element_outside(
    editor_factory,
    fixture_project: Path,
    kind: str,
    corner: str,
) -> None:
    run_canvas_boundary_case(
        editor_factory,
        fixture_project,
        kind,
        "resize",
        corner,
    )


@pytest.mark.parametrize("corner", CORNERS)
@pytest.mark.parametrize("resizable", [True, False])
@pytest.mark.parametrize("outside", [False, True])
def test_rotation_starts_only_outside_resize_handle(
    editor_factory, fixture_project, corner, resizable, outside
):
    source = "Main.slint" if resizable else "CanvasCases.slint"
    kind = "Text" if resizable else "Rectangle"
    with editor_factory(fixture_project / source) as editor:
        window = editor.window
        if resizable:
            editor.canvas.select(kind)
        else:
            editor.outline.select("layout-rectangle")
        snapshot = SourceSnapshot.capture(fixture_project)
        handle = window.get_by_accessible_name(f"{kind} resize {corner}").resolve()
        assert handle.accessible_enabled == resizable
        offset = 1 if outside else -1
        position = center(handle)
        position = slint_testing.LogicalPosition(
            x=position.x
            + (handle.size.width / 2 + offset) * (-1 if "left" in corner else 1),
            y=position.y
            + (handle.size.height / 2 + offset) * (-1 if "top" in corner else 1),
        )
        window.pointer.press_at(position)
        if outside:
            window.get_by_accessible_name("Rotation angle").wait_for()
        else:
            window.get_by_accessible_name("Rotation angle").wait_for(state="hidden")
        window.pointer.release_at(position)
        snapshot.assert_unchanged()


@pytest.mark.parametrize("corner", CORNERS)
def test_each_rotation_zone_writes_exact_source_on_release(
    editor_factory,
    fixture_project: Path,
    corner: str,
) -> None:
    source_file = fixture_project / "Main.slint"
    baseline = source_file.read_bytes()
    original = b'        text: "Fixture text";'
    rotated = original + b"\n        transform-rotation: 15deg;"
    with editor_factory(source_file) as editor:
        window = editor.window
        editor.canvas.select("Text")
        snapshot = SourceSnapshot.capture(fixture_project)
        handle = window.get_by_accessible_name(f"Text rotate {corner}").resolve()
        manual_rotation_drag(
            window,
            handle,
            *rotation_delta(window, handle, 15),
            snapshot,
        )
        expected = replace_once(baseline, original, rotated)
        snapshot.wait_for_exact(expected)
