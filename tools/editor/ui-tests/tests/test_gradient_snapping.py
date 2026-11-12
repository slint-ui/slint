# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
import re
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center_canvas_selection, zoom_canvas
from editor_sync import wait_for_source
from gradient_interactions import (
    around,
    center,
    click,
    control,
    move_picker_to_files,
    shifted,
)
from slint_testing import keys
from source_snapshot import SourceSnapshot, wait_for_source_change
from ui_assertions import expect
from ui_driver import (
    first_window,
    launch_editor,
    press_key,
    press_shortcut,
    query,
    screenshot,
    select_outline_row,
)


def gradient_scene(tmp_path, kind, rotation=0):
    fixture = (
        Path(__file__).resolve().parents[1] / "fixtures" / f"{kind}-gradient.slint"
    )
    path = tmp_path / "Gradient.slint"
    path.write_text(
        fixture.read_text().replace(
            "        width: 200px;",
            f"        transform-rotation: {rotation}deg;\n        width: 200px;",
            1,
        )
    )
    return path


def open_scene(window, zoom=100):
    select_outline_row(window, "fill")
    zoom_canvas(window, zoom)
    center_canvas_selection(window)
    click(window, "Rectangle background color picker")
    move_picker_to_files(window, "main")


@pytest.mark.parametrize(
    "kind,handle",
    [("linear", "start"), ("linear", "end"), ("conic", "end")],
)
@pytest.mark.parametrize(
    "modifier,angle,rotation,zoom",
    [
        (keys.Shift, 120, 0, 100),
        (keys.ShiftR, 120, 37, 200),
        (keys.Control, 113, 37, 200),
    ],
)
def test_gradient_angle_snapping(
    editor_binary,
    editor_environment,
    tmp_path,
    kind,
    handle,
    modifier,
    angle,
    rotation,
    zoom,
):
    path = gradient_scene(tmp_path, kind, rotation)
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, path) as editor:
        wait_for_source(path, path.read_bytes())
        window = first_window(editor)
        open_scene(window, zoom)
        initial = {"linear": 90, "conic": 220}[kind]
        start_label = "Gradient start" if kind == "linear" else "Gradient center handle"
        end_label = {
            "linear": "Gradient end",
            "conic": "Gradient rotation handle",
        }[kind]
        if handle == "start":
            start_label, end_label = end_label, start_label
        offset = 180 if handle == "start" else 0
        anchor = center(control(window, start_label), rotation + initial - 90)
        start = center(control(window, end_label), rotation + initial - 90)
        radius = math.hypot(start.x - anchor.x, start.y - anchor.y)
        target = around(anchor, radius, 113.4 + rotation + offset)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(target))
        window.dispatch_event(slint_testing.KeyPressedEvent(text=modifier))
        actual = center(control(window, end_label), rotation + angle - 90)
        expected = around(anchor, radius, angle + rotation + offset)
        assert actual.x == pytest.approx(expected.x, abs=0.01)
        assert actual.y == pytest.approx(expected.y, abs=0.01)
        expect(
            control(window, "Gradient angle", slint_testing.AccessibleRole.Text)
        ).to_have_value(str(angle))
        screenshot(window).save(tmp_path / "snapped-angle.png")
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=modifier))
        actual = center(control(window, end_label), rotation + 113.4 - 90)
        assert actual.x == pytest.approx(target.x, abs=0.01)
        assert actual.y == pytest.approx(target.y, abs=0.01)
        original.assert_unchanged_now()
        press_key(window, keys.Escape)
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        expect(query(window, "Gradient angle")).to_be_hidden()
        restored = center(control(window, end_label), rotation + initial - 90)
        assert restored.x == pytest.approx(start.x, abs=0.01)
        assert restored.y == pytest.approx(start.y, abs=0.01)
        click(window, "Close Fill")
        original.assert_unchanged()


@pytest.mark.parametrize("kind", ["linear", "radial", "conic"])
@pytest.mark.parametrize("modifier", [keys.Shift, keys.ControlR])
def test_gradient_stop_snapping(
    editor_binary, editor_environment, tmp_path, kind, modifier
):
    path = gradient_scene(tmp_path, kind)
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, path) as editor:
        wait_for_source(path, path.read_bytes())
        window = first_window(editor)
        open_scene(window)
        field = control(
            window, "Stop 2 position", slint_testing.AccessibleRole.TextInput
        )
        direction = {"linear": 0, "radial": 0, "conic": 238}[kind]
        start = center(control(window, "Gradient stop 2"), direction)
        c = center(
            control(
                window,
                "Gradient start" if kind == "linear" else "Gradient center handle",
            ),
            130 if kind == "conic" else 0,
        )
        if kind == "conic":
            radius = math.hypot(start.x - c.x, start.y - c.y)
            target = around(c, radius, 220 + 206.4)
            snapped = 210 if modifier == keys.Shift else 206
        else:
            end = center(
                control(
                    window,
                    "Gradient end"
                    if kind == "linear"
                    else "Gradient radius X and rectangle rotation handle",
                ),
                direction,
            )
            position = float(field.accessible_value) / 100
            delta = 0.584 - position
            target = shifted(start, x=(end.x - c.x) * delta, y=(end.y - c.y) * delta)
            snapped = 60 if modifier == keys.Shift else 58
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.KeyPressedEvent(text=modifier))
        window.dispatch_event(slint_testing.PointerMoveEvent(target))
        expect(
            control(window, "Gradient stop position", slint_testing.AccessibleRole.Text)
        ).to_have_value(str(snapped))
        assert float(field.accessible_value) == pytest.approx(snapped, abs=0.001)
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=modifier))
        assert float(field.accessible_value) == pytest.approx(
            206.4 if kind == "conic" else 58.4, abs=0.001
        )
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        expect(query(window, "Gradient stop position")).to_be_hidden()
        press_key(window, keys.Escape)
        original.assert_unchanged()


@pytest.mark.parametrize("handle", ["endpoint", "axis"])
def test_conic_snapping_across_seam_saves_and_undoes(
    editor_binary, editor_environment, tmp_path, handle
):
    path = gradient_scene(tmp_path, "conic")
    path.write_text(path.read_text().replace("from 220deg", "from 350deg"))
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, path) as editor:
        wait_for_source(path, path.read_bytes())
        window = first_window(editor)
        open_scene(window)
        c = center(control(window, "Gradient center handle"), 260)
        radius = 126 if handle == "endpoint" else 70
        start = around(c, radius, 350)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        target = around(c, radius, 359.6)
        window.dispatch_event(slint_testing.PointerMoveEvent(target))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
        expect(
            control(window, "Gradient angle", slint_testing.AccessibleRole.Text)
        ).to_have_value("0")
        window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Shift))
        for angle in [1, 7, 17]:
            target = around(c, radius, angle)
            window.dispatch_event(slint_testing.PointerMoveEvent(target))
        expect(
            control(window, "Gradient angle", slint_testing.AccessibleRole.Text)
        ).to_have_value("15")
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Shift))
        expect(query(window, "Gradient angle")).to_be_hidden()
        original.assert_unchanged_now()
        click(window, "Close Fill")
        saved = wait_for_source_change(path, original.sources[Path(path.name)])
        angle = re.search(rb"from ([0-9.]+)deg", saved)
        assert angle is not None
        assert float(angle.group(1)) == pytest.approx(375, abs=0.001)
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(original.sources[Path(path.name)], path.name)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, path.name)
