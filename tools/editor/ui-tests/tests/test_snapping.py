# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
import re
from contextlib import contextmanager
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import (
    center,
    center_canvas_selection,
    frame_rotation,
    physical_control_key,
    position_distance,
    selection_frame,
    zoom_canvas,
)
from editor_sync import wait_for_source
from PIL import Image
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once, wait_for_source_change
from ui_driver import (
    element,
    first_window,
    launch_editor,
    press_shortcut,
    screenshot,
    select_outline_row,
    wait_until,
)


def rectangle(
    name: str,
    horizontal_position: float,
    vertical_position: float,
    width: float,
    height: float,
    extra: str = "",
) -> str:
    return f"""    {name} := Rectangle {{
        x: {horizontal_position:g}px; y: {vertical_position:g}px; width: {width:g}px; height: {height:g}px;
        background: #8cb8df;
        {extra}
    }}
"""


@pytest.fixture
def snapping_scene(
    editor_binary: Path, editor_environment: dict[str, str], tmp_path: Path
):
    @contextmanager
    def open_scene(targets: str, moving: str | None = None, selected: bool = True):
        source = tmp_path / "Snapping.slint"
        source.write_text(
            "export component Snapping inherits Window {\n"
            "    width: 900px; height: 700px; background: white;\n"
            + (moving if moving is not None else rectangle("moving", 100, 100, 80, 60))
            + targets
            + "}\n"
        )
        with launch_editor(editor_binary, editor_environment, source) as editor:
            window = first_window(editor)
            element(window, id="Snapping::moving")
            if selected:
                select_outline_row(window, "moving")
            yield window, source

    return open_scene


def active_guides(window: slint_testing.Window) -> list[slint_testing.Element]:
    return window.query_descendants().match_type_name("SnapGuideLine").find_all()


def guide_axes(window: slint_testing.Window) -> list[str]:
    return sorted(guide.accessible_label.split()[1] for guide in active_guides(window))


def document_frame(window: slint_testing.Window, zoom: float = 1) -> tuple[float, ...]:
    origin = element(
        window, "Artboard", role=slint_testing.AccessibleRole.Region
    ).absolute_position
    frame = selection_frame(window, "Rectangle")
    return (
        (frame[0] - origin.x) / zoom,
        (frame[1] - origin.y) / zoom,
        frame[2] / zoom,
        frame[3] / zoom,
    )


def source_state(contents: bytes) -> tuple[dict[str, float], bytes]:
    moving = re.search(rb"moving\s*:=\s*Rectangle\s*\{([^}]*)\}", contents, re.DOTALL)
    assert moving is not None
    bindings = re.findall(rb"\b(x|y|width|height):\s*([-+0-9.eE]+)px", moving[1])
    assert len(bindings) == 4
    properties = dict(bindings)
    assert set(properties) == {b"x", b"y", b"width", b"height"}
    structure = re.sub(
        rb"(\b(?:x|y|width|height):\s*)[-+0-9.eE]+(?=px)",
        rb"\g<1><geometry>",
        moving[1],
    )
    geometry = {
        name: float(properties[name.encode()]) for name in ("x", "y", "width", "height")
    }
    return geometry, contents[: moving.start(1)] + structure + contents[moving.end(1) :]


def magenta_near(
    window: slint_testing.Window,
    rendered: Image.Image,
    horizontal: float,
    vertical: float,
    zoom: float = 1,
) -> bool:
    scale = rendered.width / window.root_element.size.width
    origin = element(
        window, "Artboard", role=slint_testing.AccessibleRole.Region
    ).absolute_position
    pixel_horizontal = round((origin.x + horizontal * zoom) * scale)
    pixel_vertical = round((origin.y + vertical * zoom) * scale)
    for horizontal_offset in range(-2, 3):
        for vertical_offset in range(-2, 3):
            pixel = rendered.getpixel(
                (pixel_horizontal + horizontal_offset, pixel_vertical + vertical_offset)
            )
            assert isinstance(pixel, tuple)
            red, green, blue = pixel
            if red > 150 and blue > 80 and red > green * 1.25 and blue > green * 1.05:
                return True
    return False


def press_handle(
    window: slint_testing.Window, handle: str = "move handle"
) -> slint_testing.LogicalPosition:
    start = center(
        element(window, f"Rectangle {handle}"), frame_rotation(window, "Rectangle")
    )
    window.dispatch_event(slint_testing.PointerMoveEvent(start))
    window.dispatch_event(
        slint_testing.PointerPressEvent(start, slint_testing.PointerEventButton.Left)
    )
    return start


def move_pointer(
    window: slint_testing.Window,
    start: slint_testing.LogicalPosition,
    horizontal: float,
    vertical: float,
) -> slint_testing.LogicalPosition:
    position = slint_testing.LogicalPosition(
        x=start.x + horizontal, y=start.y + vertical
    )
    window.dispatch_event(slint_testing.PointerMoveEvent(position))
    return position


def finish_drag(
    window: slint_testing.Window,
    position: slint_testing.LogicalPosition,
    source: Path,
    baseline: bytes,
) -> dict[str, float]:
    assert source.read_bytes() == baseline
    window.dispatch_event(
        slint_testing.PointerReleaseEvent(
            position, slint_testing.PointerEventButton.Left
        )
    )
    actual, structure = source_state(wait_for_source_change(source, baseline))
    assert structure == source_state(baseline)[1]
    wait_until(lambda: not active_guides(window))
    return actual


@pytest.mark.parametrize(
    ("handle_label", "property_name", "frame_index", "original", "snapped"),
    [("move handle", "x", 0, 40, 120), ("resize right", "width", 2, 80, 160)],
)
def test_control_toggles_snapping_without_pointer_movement(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    handle_label: str,
    property_name: str,
    frame_index: int,
    original: int,
    snapped: int,
) -> None:
    source_file = fixture_project / "ControlSnapping.slint"
    source_file.write_text("""export component ControlSnapping inherits Window {
    width: 800px;
    height: 600px;
    moving := Rectangle {
        x: 40px;
        y: 40px;
        width: 80px;
        height: 60px;
        background: red;
    }
    target := Rectangle {
        x: 200px;
        y: 250px;
        width: 100px;
        height: 60px;
        background: blue;
    }
}
""")
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    control = physical_control_key()
    right_control = keys.MetaR if control == keys.Meta else keys.ControlR
    other_modifier = keys.Control if control == keys.Meta else keys.Meta
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_outline_row(window, "moving")
        start = center(element(window, f"Rectangle {handle_label}"))
        end = slint_testing.LogicalPosition(x=start.x + 76, y=start.y)
        initial = selection_frame(window, "Rectangle")[frame_index]
        button = slint_testing.PointerEventButton.Left

        def wait_for_delta(delta: int) -> None:
            wait_until(
                lambda: math.isclose(
                    selection_frame(window, "Rectangle")[frame_index],
                    initial + delta,
                    abs_tol=0.1,
                )
            )

        assert not active_guides(window)
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_delta(80)
        assert active_guides(window)
        window.dispatch_event(slint_testing.KeyPressedEvent(text=other_modifier))
        wait_for_delta(80)
        assert active_guides(window)
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=other_modifier))
        window.dispatch_event(slint_testing.KeyPressedEvent(text=control))
        wait_for_delta(76)
        assert not active_guides(window)
        window.dispatch_event(slint_testing.KeyPressedEvent(text=right_control))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=control))
        wait_for_delta(76)
        assert not active_guides(window)
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=right_control))
        wait_for_delta(80)
        assert active_guides(window)
        snapshot.assert_unchanged_now()
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        wait_until(lambda: not active_guides(window))
        snapshot.wait_for_applied(
            replace_once(
                baseline,
                f"        {property_name}: {original}px;".encode(),
                f"        {property_name}: {snapped}px;".encode(),
            ),
            source_file.name,
        )


@pytest.mark.parametrize("axes", ["X", "Y", "XY"])
def test_move_axes_snap_independently_inside_square_tolerance(
    snapping_scene, axes: str
) -> None:
    targets = (rectangle("horizontal", 205.5, 200, 20, 30) if "X" in axes else "") + (
        rectangle("vertical", 400, 185.5, 20, 20) if "Y" in axes else ""
    )
    with snapping_scene(targets) as (window, source):
        baseline = source.read_bytes()
        assert not active_guides(window)
        start = press_handle(window)
        end = move_pointer(window, start, 20, 20)
        expected = (
            125.5 if "X" in axes else 120,
            125.5 if "Y" in axes else 120,
            80,
            60,
        )
        assert document_frame(window) == pytest.approx(expected, abs=0.02)
        assert guide_axes(window) == list(axes)
        actual = finish_drag(window, end, source, baseline)
        assert tuple(actual.values()) == expected


@pytest.mark.parametrize("operation", ["move", "resize"])
@pytest.mark.parametrize(
    ("percent", "screen_gap", "snaps"),
    [(100, 5.5, True), (100, 6.5, False), (200, 5.5, True), (200, 6.5, False)],
)
def test_tolerance_is_six_screen_pixels(
    snapping_scene, operation: str, percent: int, screen_gap: float, snaps: bool
) -> None:
    zoom = percent / 100
    target_x = 200 + screen_gap / zoom
    with snapping_scene(rectangle("target", target_x, 200, 20, 30)) as (window, source):
        baseline = source.read_bytes()
        center_canvas_selection(window)
        zoom_canvas(window, percent)
        start = press_handle(
            window, "move handle" if operation == "move" else "resize right"
        )
        end = move_pointer(window, start, 20 * zoom, 0)
        correction = screen_gap / zoom if snaps else 0
        expected = (
            (120 + correction, 100, 80, 60)
            if operation == "move"
            else (100, 100, 100 + correction, 60)
        )
        assert document_frame(window, zoom) == pytest.approx(expected, abs=0.02)
        assert guide_axes(window) == (["X"] if snaps else [])
        actual = finish_drag(window, end, source, baseline)
        assert tuple(actual.values()) == expected


def test_proximity_prefers_near_finite_extent_over_smaller_axis_gap(
    snapping_scene,
) -> None:
    targets = rectangle("near", 205, 170, 20, 20) + rectangle("far", 200.5, 600, 20, 20)
    with snapping_scene(targets) as (window, source):
        baseline = source.read_bytes()
        end = move_pointer(window, press_handle(window), 20, 0)
        assert document_frame(window) == pytest.approx((125, 100, 80, 60), abs=0.02)
        assert guide_axes(window) == ["X"]
        assert finish_drag(window, end, source, baseline)["x"] == 125


@pytest.mark.parametrize(
    ("target_x", "target_width", "expected_x"), [(205.5, 20, 125.5), (144.5, 40, 124.5)]
)
def test_move_aligns_edges_and_centers(
    snapping_scene, target_x: float, target_width: float, expected_x: float
) -> None:
    with snapping_scene(rectangle("target", target_x, 200, target_width, 30)) as (
        window,
        source,
    ):
        baseline = source.read_bytes()
        end = move_pointer(window, press_handle(window), 20, 0)
        assert document_frame(window)[0] == pytest.approx(expected_x, abs=0.02)
        assert guide_axes(window) == ["X"]
        assert finish_drag(window, end, source, baseline)["x"] == expected_x


OPPOSITE_HANDLES = {
    "top": "bottom",
    "right": "left",
    "bottom": "top",
    "left": "right",
    "top-left": "bottom-right",
    "top-right": "bottom-left",
    "bottom-right": "top-left",
    "bottom-left": "top-right",
}


@pytest.mark.parametrize("handle", list(OPPOSITE_HANDLES))
def test_resize_handles_snap_moving_features_and_keep_opposite_anchor(
    snapping_scene, handle: str
) -> None:
    horizontal = handle not in ("top", "bottom")
    vertical = handle not in ("left", "right")
    left = "left" in handle
    top = "top" in handle
    target_x = 174.5 if left else 305.5
    target_y = 174.5 if top else 285.5
    targets = (rectangle("horizontal", target_x, 400, 20, 20) if horizontal else "") + (
        rectangle("vertical", 400, target_y, 20, 20) if vertical else ""
    )
    with snapping_scene(targets, rectangle("moving", 200, 200, 80, 60)) as (
        window,
        source,
    ):
        baseline = source.read_bytes()
        fixed_label = f"Rectangle resize {OPPOSITE_HANDLES[handle]}"
        fixed = center(element(window, fixed_label))
        start = press_handle(window, f"resize {handle}")
        end = move_pointer(
            window,
            start,
            (-20 if left else 20) if horizontal else 0,
            (-20 if top else 20) if vertical else 0,
        )
        expected = (
            target_x if horizontal and left else 200,
            target_y if vertical and top else 200,
            105.5 if horizontal else 80,
            85.5 if vertical else 60,
        )
        assert document_frame(window) == pytest.approx(expected, abs=0.03)
        assert position_distance(fixed, center(element(window, fixed_label))) < 0.03
        assert guide_axes(window) == (["X"] if horizontal else []) + (
            ["Y"] if vertical else []
        )
        actual = finish_drag(window, end, source, baseline)
        assert tuple(actual.values()) == pytest.approx(expected, abs=0.03)


def test_free_corner_satisfies_both_independent_winners(snapping_scene) -> None:
    targets = rectangle("horizontal", 204, 400, 20, 20) + rectangle(
        "vertical", 400, 186, 20, 20
    )
    with snapping_scene(targets) as (window, source):
        baseline = source.read_bytes()
        fixed = center(element(window, "Rectangle resize top-left"))
        end = move_pointer(window, press_handle(window, "resize bottom-right"), 20, 20)
        assert document_frame(window) == pytest.approx((100, 100, 104, 86), abs=0.02)
        assert guide_axes(window) == ["X", "Y"]
        assert (
            position_distance(
                fixed, center(element(window, "Rectangle resize top-left"))
            )
            < 0.02
        )
        assert tuple(finish_drag(window, end, source, baseline).values()) == (
            100,
            100,
            104,
            86,
        )


@pytest.mark.parametrize(
    ("horizontal_gap", "vertical_gap", "axes"),
    [(4, 6, ["X"]), (6, 4, ["Y"]), (4, 4, ["X", "Y"])],
)
def test_square_resize_uses_better_conflicting_winner_or_both_compatible(
    snapping_scene, horizontal_gap: int, vertical_gap: int, axes: list[str]
) -> None:
    targets = rectangle("horizontal", 220 + horizontal_gap, 245, 20, 20) + rectangle(
        "vertical", 245, 220 + vertical_gap, 20, 20
    )
    with snapping_scene(targets, rectangle("moving", 100, 100, 100, 100)) as (
        window,
        source,
    ):
        baseline = source.read_bytes()
        fixed = center(element(window, "Rectangle resize top-left"))
        start = press_handle(window, "resize bottom-right")
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
        end = move_pointer(window, start, 20, 20)
        assert document_frame(window) == pytest.approx((100, 100, 124, 124), abs=0.02)
        assert guide_axes(window) == axes
        assert (
            position_distance(
                fixed, center(element(window, "Rectangle resize top-left"))
            )
            < 0.02
        )
        actual = finish_drag(window, end, source, baseline)
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
        assert (actual["width"], actual["height"]) == (124, 124)


def test_rotated_edge_keeps_constraint_and_only_satisfied_winner(
    snapping_scene,
) -> None:
    proposed_x = 150 + 100 / math.sqrt(2)
    proposed_y = 130 + 40 / math.sqrt(2)
    targets = rectangle(
        "horizontal", proposed_x + 5.5, proposed_y - 10, 20, 20
    ) + rectangle("vertical", proposed_x - 10, proposed_y + 5.9, 20, 20)
    moving = rectangle("moving", 100, 100, 100, 60, "transform-rotation: 45deg;")
    with snapping_scene(targets, moving) as (window, source):
        baseline = source.read_bytes()
        rotation = frame_rotation(window, "Rectangle")
        fixed = center(element(window, "Rectangle resize left"), rotation)
        start = press_handle(window, "resize right")
        end = move_pointer(window, start, 20 / math.sqrt(2), 20 / math.sqrt(2))
        frame = document_frame(window)
        assert frame[2] == pytest.approx(120 + 5.5 * math.sqrt(2), abs=0.03)
        assert frame[3] == pytest.approx(60, abs=0.02)
        assert (
            position_distance(
                fixed, center(element(window, "Rectangle resize left"), rotation)
            )
            < 0.03
        )
        assert guide_axes(window) == ["X"]
        actual = finish_drag(window, end, source, baseline)
        assert actual["width"] == pytest.approx(120 + 5.5 * math.sqrt(2), abs=0.03)
        assert actual["height"] == 60


def test_resize_rejects_non_positive_proposal_then_recovers(snapping_scene) -> None:
    with snapping_scene(rectangle("target", 205.5, 200, 20, 30)) as (window, source):
        baseline = source.read_bytes()
        start = press_handle(window, "resize right")
        move_pointer(window, start, 20, 0)
        assert document_frame(window) == pytest.approx((100, 100, 105.5, 60), abs=0.02)
        assert guide_axes(window) == ["X"]
        move_pointer(window, start, -80, 0)
        assert document_frame(window) == pytest.approx((100, 100, 105.5, 60), abs=0.02)
        assert not active_guides(window)
        end = move_pointer(window, start, 60, 0)
        assert document_frame(window) == pytest.approx((100, 100, 140, 60), abs=0.02)
        assert not active_guides(window)
        assert finish_drag(window, end, source, baseline)["width"] == 140


@pytest.mark.parametrize("gap", [0.25, 4.5, 5])
@pytest.mark.parametrize("operation", ["move", "resize"])
def test_nested_rotated_scaled_parent_preserves_fractional_target(
    snapping_scene, operation: str, gap: float, tmp_path: Path
) -> None:
    nested = (
        """    container := Rectangle {
        x: 30px; y: 90px; width: 300px; height: 240px;
        transform-rotation: 90deg;
        transform-scale: 1.25;
"""
        + rectangle("moving", 40, 50, 80, 60)
        + rectangle("target", 200 + gap, 100, 20, 40)
        + "    }\n"
    )
    with snapping_scene("", nested) as (window, source):
        baseline = source.read_bytes()
        initial = selection_frame(window, "Rectangle")
        rotation = frame_rotation(window, "Rectangle")
        fixed = center(element(window, "Rectangle resize left"), rotation)
        end = move_pointer(
            window,
            press_handle(
                window, "move handle" if operation == "move" else "resize right"
            ),
            0,
            100,
        )
        frame = selection_frame(window, "Rectangle")
        snaps = gap < 4.8
        correction = gap if snaps else 0
        rendered_delta = (80 + correction) * 1.25
        if operation == "move":
            assert frame[1] - initial[1] == pytest.approx(rendered_delta, abs=0.02)
            assert frame[2:] == pytest.approx(initial[2:], abs=0.02)
            expected = {"x": 120 + correction, "y": 50, "width": 80, "height": 60}
        else:
            assert frame[2] - initial[2] == pytest.approx(rendered_delta, abs=0.02)
            assert frame[3] == pytest.approx(initial[3], abs=0.02)
            assert (
                position_distance(
                    fixed, center(element(window, "Rectangle resize left"), rotation)
                )
                < 0.03
            )
            expected = {"x": 40, "y": 50, "width": 160 + correction, "height": 60}
        assert guide_axes(window) == (["X"] if snaps else [])
        if snaps:
            rendered = screenshot(window).convert("RGB")
            rendered.save(tmp_path / "nested-snapping-guide.png")
            assert magenta_near(window, rendered, 180, 272.5 + 1.25 * gap)
        assert finish_drag(window, end, source, baseline) == pytest.approx(
            expected, abs=0.001
        )


@pytest.mark.parametrize("percent", [100, 200])
def test_guide_pixels_bridge_finite_target_and_moving_center(
    snapping_scene, tmp_path: Path, percent: int
) -> None:
    with snapping_scene(rectangle("target", 164.5, 200, 10, 30)) as (window, source):
        baseline = source.read_bytes()
        center_canvas_selection(window)
        zoom_canvas(window, percent)
        zoom = percent / 100
        end = move_pointer(window, press_handle(window), 22 * zoom, 0)
        assert document_frame(window, zoom) == pytest.approx(
            (124.5, 100, 80, 60), abs=0.02
        )
        assert guide_axes(window) == ["X"]
        rendered = screenshot(window).convert("RGB")
        rendered.save(tmp_path / "snapping-guide.png")
        for vertical in (130, 150, 180, 200, 230):
            assert magenta_near(window, rendered, 164.5, vertical, zoom), vertical
        for vertical in (120, 240):
            assert not magenta_near(window, rendered, 164.5, vertical, zoom), vertical
        finish_drag(window, end, source, baseline)


def test_move_leaves_snap_range_and_release_hides_guides(snapping_scene) -> None:
    with snapping_scene(rectangle("target", 205.5, 200, 100, 30)) as (window, source):
        baseline = source.read_bytes()
        start = press_handle(window)
        move_pointer(window, start, 20, 0)
        assert document_frame(window)[0] == pytest.approx(125.5, abs=0.02)
        assert guide_axes(window) == ["X"]
        end = move_pointer(window, start, 40, 0)
        assert document_frame(window)[0] == pytest.approx(140, abs=0.02)
        assert not active_guides(window)
        assert finish_drag(window, end, source, baseline)["x"] == 140


@pytest.mark.parametrize("invalidate", ["selection", "reload"])
def test_selection_change_or_reload_rejects_stale_drag(
    snapping_scene, invalidate: str
) -> None:
    with snapping_scene(rectangle("target", 205.5, 200, 100, 30)) as (window, source):
        baseline = source.read_bytes()
        start = press_handle(window)
        near = move_pointer(window, start, 20, 0)
        assert guide_axes(window) == ["X"]
        if invalidate == "selection":
            select_outline_row(window, "target")
            expected = baseline
        else:
            expected = baseline.replace(b"background: white", b"background: #f5f5f5", 1)
            source.write_bytes(expected)
            wait_for_source(source, expected)
        wait_until(lambda: not active_guides(window))
        snapshot = SourceSnapshot.capture(source.parent)
        stale_position = move_pointer(window, near, 10, 0)
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                stale_position, slint_testing.PointerEventButton.Left
            )
        )
        snapshot.assert_unchanged()
        assert source.read_bytes() == expected
        assert not active_guides(window)


def test_hover_drag_control_toggle_and_release_while_held(snapping_scene) -> None:
    with snapping_scene(rectangle("target", 204, 200, 100, 30), selected=False) as (
        window,
        source,
    ):
        baseline = source.read_bytes()
        start = center(element(window, id="Snapping::moving"))
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        element(window, "Hovered Rectangle", role=slint_testing.AccessibleRole.Region)
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                start, slint_testing.PointerEventButton.Left
            )
        )
        end = move_pointer(window, start, 20, 0)
        assert document_frame(window)[0] == pytest.approx(124, abs=0.02)
        assert guide_axes(window) == ["X"]
        control = physical_control_key()
        window.dispatch_event(slint_testing.KeyPressedEvent(text=control))
        wait_until(lambda: abs(document_frame(window)[0] - 120) < 0.02)
        assert not active_guides(window)
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=control))
        wait_until(lambda: abs(document_frame(window)[0] - 124) < 0.02)
        assert guide_axes(window) == ["X"]
        window.dispatch_event(slint_testing.KeyPressedEvent(text=control))
        wait_until(lambda: abs(document_frame(window)[0] - 120) < 0.02)
        assert finish_drag(window, end, source, baseline)["x"] == 120
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=control))


def test_release_with_control_held_discards_fractional_snap(snapping_scene) -> None:
    with snapping_scene(rectangle("target", 200.25, 200, 100, 30)) as (window, source):
        baseline = source.read_bytes()
        end = move_pointer(window, press_handle(window), 20, 0)
        assert document_frame(window)[0] == pytest.approx(120.25, abs=0.02)
        control = physical_control_key()
        window.dispatch_event(slint_testing.KeyPressedEvent(text=control))
        assert not active_guides(window)
        actual = finish_drag(window, end, source, baseline)
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=control))
        assert actual["x"] == 120


def test_snapped_edit_can_be_undone_and_redone(snapping_scene) -> None:
    with snapping_scene(rectangle("target", 205.5, 200, 100, 30)) as (window, source):
        baseline = source.read_bytes()
        snapshot = SourceSnapshot.capture(source.parent)
        end = move_pointer(window, press_handle(window), 20, 0)
        assert finish_drag(window, end, source, baseline)["x"] == 125.5
        snapped = source.read_bytes()
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, source.name)
        assert source_state(source.read_bytes())[0]["x"] == 100
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(snapped, source.name)
        assert source_state(source.read_bytes())[0]["x"] == 125.5
        assert not active_guides(window)


def test_square_resize_skips_closer_target_below_minimum(snapping_scene) -> None:
    targets = rectangle("invalid", 176, 200, 50, 20) + rectangle(
        "valid", 185, 200, 50, 20
    )
    with snapping_scene(targets, rectangle("moving", 100, 100, 100, 100)) as (
        window,
        source,
    ):
        baseline = source.read_bytes()
        start = press_handle(window, "resize bottom-right")
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
        end = move_pointer(window, start, -20, -20)
        assert document_frame(window) == pytest.approx((100, 100, 85, 85), abs=0.02)
        assert guide_axes(window) == ["X"]
        actual = finish_drag(window, end, source, baseline)
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
        assert (actual["width"], actual["height"]) == (85, 85)


def test_square_conflict_uses_finite_extent_proximity_instead_of_axis_gap(
    snapping_scene,
) -> None:
    targets = rectangle("horizontal", 222, 300, 20, 20) + rectangle(
        "vertical", 245, 225, 20, 20
    )
    with snapping_scene(targets, rectangle("moving", 100, 100, 100, 100)) as (
        window,
        source,
    ):
        baseline = source.read_bytes()
        start = press_handle(window, "resize bottom-right")
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
        end = move_pointer(window, start, 20, 20)
        assert document_frame(window) == pytest.approx((100, 100, 125, 125), abs=0.02)
        assert guide_axes(window) == ["Y"]
        actual = finish_drag(window, end, source, baseline)
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
        assert (actual["width"], actual["height"]) == (125, 125)


def test_positive_resize_rejects_target_that_would_flip_width(snapping_scene) -> None:
    with snapping_scene(rectangle("target", 90, 200, 8, 20)) as (window, source):
        baseline = source.read_bytes()
        end = move_pointer(window, press_handle(window, "resize right"), -77, 0)
        assert document_frame(window) == pytest.approx((100, 100, 3, 60), abs=0.02)
        assert not active_guides(window)
        actual = finish_drag(window, end, source, baseline)
        assert (actual["width"], actual["height"]) == (3, 60)
