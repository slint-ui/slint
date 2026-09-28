# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
import re
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center_canvas_selection, zoom_canvas
from editor_sync import wait_for_source
from gradient_interactions import around, center, gesture, shifted
from slint_test import expect, step
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    first_window,
    launch_editor,
    select_outline_row,
)


@pytest.fixture
def conic_scene(tmp_path):
    path = tmp_path / "ConicGradientScene.slint"
    path.write_text("""export component ConicGradientScene inherits Window {
    width: 400px;
    height: 400px;
    fill := Rectangle {
        width: 200px;
        height: 200px;
        background: @conic-gradient(from 220deg, #7e3b66 0deg, #264052 198deg, #568fb8 360deg);
    }
}
""")
    return path


def open_conic(window):
    select_outline_row(window, "fill")
    window.get_by_role("button", name="Rectangle background color picker").activate()
    window.get_by_role("button", name="Gradient rotation handle").resolve()
    assert not window.get_by_accessible_name("Gradient angle degrees").all()
    assert not window.get_by_accessible_name("Gradient center").all()
    window.get_by_role("button", name="Add gradient stop").resolve()


def stop_rotation(position, start=220, rotation=0):
    return rotation + start - 90 + position + (90 if position >= 360 else -90)


def stop_center(window, index, position, start=220, rotation=0):
    return center(
        window.get_by_role("button", name=f"Gradient stop {index}").resolve(),
        stop_rotation(position, start, rotation),
    )


@pytest.mark.parametrize("kind", ["center", "rotation", "stop"])
def test_conic_escape_restores_gesture(editor_factory, conic_scene, tmp_path, kind):
    original = SourceSnapshot.capture(tmp_path)
    with editor_factory(conic_scene) as editor:
        with step(
            "Open conic gradient controls",
            layer="adapter",
            trace_coverage="group-only legacy setup",
        ):
            open_conic(editor.window)
        label = {
            "center": "Gradient center handle",
            "rotation": "Gradient rotation handle",
            "stop": "Gradient stop 2",
        }[kind]
        handle = editor.window.get_by_role("button", name=label)
        rotation = stop_rotation(198) if kind == "stop" else 130
        start = handle.center(rotation_degrees=rotation)
        with editor.window.pointer.drag_from(start) as drag:
            drag.move_by(25, -15)
            editor.window.keyboard.press("Escape")
            drag.release()
        with step("Escape restores the gradient handle", layer="assertion"):
            restored = handle.center(rotation_degrees=rotation)
            assert restored.x == pytest.approx(start.x, abs=0.001)
            assert restored.y == pytest.approx(start.y, abs=0.001)
        editor.window.get_by_role("button", name="Close Custom").activate()
        with step(
            "Closing the cancelled picker leaves source unchanged", layer="assertion"
        ):
            original.assert_unchanged()


@pytest.mark.parametrize("percent", [50, 100, 200])
def test_conic_keyboard_and_seam_neighbor(
    editor_binary, editor_environment, conic_scene, tmp_path, percent
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        wait_for_source(conic_scene, conic_scene.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        zoom_canvas(window, percent)
        center_canvas_selection(window)
        open_conic(window)
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 130
        )
        gesture(window, c, c)
        window.keyboard.press(keys.RightArrow)
        window.keyboard.shortcut(keys.Shift, keys.DownArrow)
        moved = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 130
        )
        assert moved.x == pytest.approx(c.x + percent / 100, abs=0.001)
        assert moved.y == pytest.approx(c.y + percent / 10, abs=0.001)
        r = center(
            window.get_by_role("button", name="Gradient rotation handle").resolve(), 130
        )
        gesture(window, r, r)
        window.keyboard.press(keys.RightArrow)
        window.keyboard.shortcut(keys.Shift, keys.LeftArrow)
        r = center(
            window.get_by_role("button", name="Gradient rotation handle").resolve(), 121
        )
        expected = around(moved, 126 * percent / 100, 211)
        assert r.x == pytest.approx(expected.x, abs=0.001)
        assert r.y == pytest.approx(expected.y, abs=0.001)
        p = stop_center(window, 2, 198, start=211)
        gesture(window, p, p)
        window.keyboard.press(keys.RightArrow)
        window.keyboard.shortcut(keys.Shift, keys.LeftArrow)
        assert float(
            window.get_by_role(
                slint_testing.AccessibleRole.TextInput, name="Stop 2 position"
            )
            .resolve()
            .accessible_value
        ) == pytest.approx(189)
        p = stop_center(window, 1, 0, start=211)
        gesture(window, p, p)
        window.keyboard.press(keys.Delete)
        expect(window.get_by_accessible_name("Gradient stop 3")).to_be_hidden()
        window.get_by_role("button", name="Gradient stop 2").resolve()
        window.keyboard.press(keys.LeftArrow)
        assert float(
            window.get_by_role(
                slint_testing.AccessibleRole.TextInput, name="Stop 2 position"
            )
            .resolve()
            .accessible_value
        ) == pytest.approx(359)
        window.keyboard.press(keys.Backspace)
        window.keyboard.press(keys.Delete)
        window.get_by_role("button", name="Gradient stop 2").resolve()
        assert not window.get_by_accessible_name("Gradient stop 3").all()
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


def test_conic_swatch_delete_keeps_canvas_element(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        wait_for_source(conic_scene, conic_scene.read_bytes())
        window = first_window(editor)
        open_conic(window)
        window.get_by_role("button", name="Remove stop 3").activate()
        expect(window.get_by_accessible_name("Gradient stop 3")).to_be_hidden()
        position = window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Stop 2 position"
        ).resolve()
        gesture(window, center(position), center(position))
        window.keyboard.press(keys.Tab)
        window.keyboard.press(keys.Delete)
        original.assert_unchanged_now()
        window.keyboard.press(keys.Space)
        window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Hex color"
        ).resolve()
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


def test_external_edit_invalidates_conic_session(
    editor_binary, editor_environment, conic_scene
):

    original = conic_scene.read_text()
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        wait_for_source(conic_scene, conic_scene.read_bytes())
        window = first_window(editor)
        open_conic(window)
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 130
        )
        gesture(window, c, shifted(c, x=25, y=15))
        external = original.replace("#7e3b66", "#abcdef")
        conic_scene.write_text(external)
        wait_for_source(conic_scene, external.encode())
        assert not window.get_by_accessible_name("Gradient center handle").all()


@pytest.mark.parametrize("rotation", [0, 45, 90])
def test_conic_center_translation(
    editor_binary, editor_environment, conic_scene, tmp_path, rotation
):
    conic_scene.write_text(
        conic_scene.read_text().replace(
            "        width: 200px;",
            f"        transform-rotation: {rotation}deg;\n        width: 200px;",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        wait_for_source(conic_scene, conic_scene.read_bytes())
        window = first_window(editor)
        open_conic(window)
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(),
            rotation + 130,
        )
        r = center(
            window.get_by_role("button", name="Gradient rotation handle").resolve(),
            rotation + 130,
        )
        gesture(window, c, shifted(c, x=17, y=23))
        moved = center(
            window.get_by_role("button", name="Gradient center handle").resolve(),
            rotation + 130,
        )
        end = center(
            window.get_by_role("button", name="Gradient rotation handle").resolve(),
            rotation + 130,
        )
        assert moved.x == pytest.approx(c.x + 17, abs=0.001)
        assert moved.y == pytest.approx(c.y + 23, abs=0.001)
        assert end.x == pytest.approx(r.x + 17, abs=0.001)
        assert end.y == pytest.approx(r.y + 23, abs=0.001)
        original.assert_unchanged_now()
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


def test_conic_noop_and_collapsed_rotation_do_not_write_source(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        wait_for_source(conic_scene, conic_scene.read_bytes())
        window = first_window(editor)
        open_conic(window)
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 130
        )
        r = center(
            window.get_by_role("button", name="Gradient rotation handle").resolve(), 130
        )
        gesture(window, r, c)
        actual = center(
            window.get_by_role("button", name="Gradient rotation handle").resolve(), 130
        )
        assert actual.x == pytest.approx(r.x, abs=0.001)
        assert actual.y == pytest.approx(r.y, abs=0.001)
        window.get_by_role("button", name="Close Custom").activate()
        original.assert_unchanged()
        open_conic(window)
        window.get_by_role("button", name="Close Custom").activate()
        original.assert_unchanged()


def test_conic_seam_handles_and_stop_crossing(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        wait_for_source(conic_scene, conic_scene.read_bytes())
        window = first_window(editor)
        open_conic(window)
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 130
        )
        first = stop_center(window, 1, 0)
        last = stop_center(window, 3, 360)
        assert math.hypot(first.x - c.x, first.y - c.y) == pytest.approx(152, abs=0.001)
        assert math.hypot(last.x - c.x, last.y - c.y) == pytest.approx(100, abs=0.001)
        gesture(window, first, around(c, 152, 240))
        assert float(
            window.get_by_role(
                slint_testing.AccessibleRole.TextInput, name="Stop 1 position"
            )
            .resolve()
            .accessible_value
        ) == pytest.approx(20, abs=0.01)
        gesture(window, last, around(c, 100, 200))
        assert float(
            window.get_by_role(
                slint_testing.AccessibleRole.TextInput, name="Stop 3 position"
            )
            .resolve()
            .accessible_value
        ) == pytest.approx(340, abs=0.01)
        start = stop_center(window, 2, 198)
        window.pointer.press_at(start)
        for degrees in [250, 320, 350, 280, 180, 90, 10, 60]:
            p = around(c, 152, 220 + degrees)
            window.pointer.move_to(p)
            assert float(
                window.get_by_role(
                    slint_testing.AccessibleRole.TextInput, name="Stop 2 position"
                )
                .resolve()
                .accessible_value
            ) == pytest.approx(degrees, abs=0.01)
        window.pointer.release_at(p)
        window.get_by_role("button", name="Edit stop 2 color").activate()
        assert (
            window.get_by_role(slint_testing.AccessibleRole.TextInput, name="Hex color")
            .resolve()
            .accessible_value
            == "#264052"
        )
        window.get_by_role("button", name="Close Stop color").activate()
        (tmp_path / "conic-editor.png").write_bytes(window.screenshot())
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


def test_conic_ring_insertion(editor_binary, editor_environment, conic_scene, tmp_path):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        wait_for_source(conic_scene, conic_scene.read_bytes())
        window = first_window(editor)
        open_conic(window)
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 130
        )
        p = around(c, 126, 220 + 90)
        gesture(window, p, p)
        gesture(window, p, p)
        window.get_by_role("button", name="Gradient stop 4").resolve()
        assert float(
            window.get_by_role(
                slint_testing.AccessibleRole.TextInput, name="Stop 2 position"
            )
            .resolve()
            .accessible_value
        ) == pytest.approx(90, abs=0.01)
        window.keyboard.press(keys.Delete)
        assert not window.get_by_accessible_name("Gradient stop 4").all()
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


def test_conic_insertion_samples_straight_alpha(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    conic_scene.write_text(
        conic_scene.read_text().replace(
            "#7e3b66 0deg, #264052 198deg, #568fb8 360deg",
            "#ff000000 0deg, #0000ff 360deg",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        wait_for_source(conic_scene, conic_scene.read_bytes())
        window = first_window(editor)
        open_conic(window)
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 130
        )
        p = around(c, 126, 400)
        gesture(window, p, p)
        gesture(window, p, p)
        window.get_by_role("button", name="Edit stop 2 color").activate()
        value = (
            window.get_by_role(slint_testing.AccessibleRole.TextInput, name="Hex color")
            .resolve()
            .accessible_value
        )
        assert value in ("#80008080", "#7f008080", "#80007f7f", "#7f00807f")
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


def test_conic_coincident_stops_keep_keyboard_focus(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    conic_scene.write_text(
        conic_scene.read_text().replace("#264052 198deg", "#264052 0deg")
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        wait_for_source(conic_scene, conic_scene.read_bytes())
        window = first_window(editor)
        open_conic(window)
        window.get_by_role("button", name="Gradient stop 1").activate()
        window.keyboard.press(keys.Tab)
        window.keyboard.press(keys.RightArrow)
        assert float(
            window.get_by_role(
                slint_testing.AccessibleRole.TextInput, name="Stop 2 position"
            )
            .resolve()
            .accessible_value
        ) == pytest.approx(1)
        assert float(
            window.get_by_role(
                slint_testing.AccessibleRole.TextInput, name="Stop 1 position"
            )
            .resolve()
            .accessible_value
        ) == pytest.approx(0)
        window.get_by_role("button", name="Edit stop 2 color").activate()
        field = window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Hex color"
        ).resolve()
        p = center(field)
        gesture(window, p, p)
        window.keyboard.shortcut(keys.Control, "a")
        window.keyboard.press(keys.Backspace)
        window.get_by_role("button", name="Gradient stop 3").resolve()
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


def test_conic_picker_and_canvas_share_selection_and_color(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        wait_for_source(conic_scene, conic_scene.read_bytes())
        window = first_window(editor)
        open_conic(window)
        (tmp_path / "conic-picker-and-canvas.png").write_bytes(window.screenshot())
        window.get_by_role(
            slint_testing.AccessibleRole.Slider, name="Gradient stop 2"
        ).resolve()
        assert not window.get_by_accessible_name("Hex color").all()
        window.get_by_role("button", name="Edit stop 2 color").activate()
        field = window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Hex color"
        )
        expect(field).to_have_value("#264052")
        window.get_by_role("button", name="Gradient stop 1").activate()
        expect(field).to_have_value("#7e3b66")
        window.get_by_role("button", name="Gradient stop 2").activate()
        expect(field).to_have_value("#264052")
        field.set_accessible_value("#abcdef80")
        expect(field).to_have_value("#abcdef80")
        window.get_by_role("button", name="Close Stop color").activate()
        window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Stop 2 position"
        ).set_accessible_value("162")
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 130
        )
        actual = stop_center(window, 2, 162)
        expected = around(c, 152, 220 + 162)
        assert actual.x == pytest.approx(expected.x, abs=0.001)
        assert actual.y == pytest.approx(expected.y, abs=0.001)
        window.get_by_role("button", name="Edit stop 2 color").activate()
        assert (
            window.get_by_role(slint_testing.AccessibleRole.TextInput, name="Hex color")
            .resolve()
            .accessible_value
            == "#abcdef80"
        )
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


def test_conic_activation_from_solid(
    editor_binary, editor_environment, conic_scene, tmp_path
):
    conic_scene.write_text(
        re.sub(r"@conic-gradient\([^;]+\)", "#7e3b66", conic_scene.read_text())
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, conic_scene) as editor:
        wait_for_source(conic_scene, conic_scene.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        window.get_by_role(
            "button", name="Rectangle background color picker"
        ).activate()
        assert not window.get_by_accessible_name("Gradient rotation handle").all()
        window.get_by_role("button", name="Gradient").activate()
        window.get_by_role(
            slint_testing.AccessibleRole.Combobox, name="Gradient type"
        ).set_accessible_value("Conic")
        window.get_by_role("button", name="Gradient rotation handle").resolve()
        assert not window.get_by_accessible_name("Gradient angle degrees").all()
        window.get_by_role("button", name="Edit stop 1 color").resolve()
        window.get_by_role("button", name="Solid").activate()
        assert not window.get_by_accessible_name("Gradient rotation handle").all()
        window.get_by_role("button", name="Gradient").activate()
        window.get_by_role("button", name="Gradient rotation handle").resolve()
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


@pytest.mark.parametrize("handle", ["endpoint", "ray"])
def test_conic_rotation_crosses_the_seam(editor_factory, conic_scene, tmp_path, handle):
    from slint_test import Point, step
    from source_snapshot import wait_for_source_change

    conic_scene.write_text(
        conic_scene.read_text().replace("from 220deg", "from 350deg")
    )
    original = SourceSnapshot.capture(tmp_path)
    baseline = original.sources[Path(conic_scene.name)]
    with editor_factory(conic_scene) as editor:
        with step(
            "Open conic picker",
            layer="adapter",
            trace_coverage="group-only legacy helper",
        ):
            open_conic(editor.window)
        center_handle = editor.window.get_by_accessible_name("Gradient center handle")
        rotation = editor.window.get_by_accessible_name("Gradient rotation handle")
        c = center_handle.center(rotation_degrees=260)
        radius = 126 if handle == "endpoint" else 70
        start = around(c, radius, 350)
        with editor.window.pointer.drag_from(Point(start.x, start.y)) as drag:
            for angle in [355, 359, 1, 7]:
                position = around(c, radius, angle)
                drag.move_to(Point(position.x, position.y))
                actual = rotation.center(rotation_degrees=angle - 90)
                expected = around(c, 126, angle)
                assert actual.x == pytest.approx(expected.x, abs=0.01)
                assert actual.y == pytest.approx(expected.y, abs=0.01)
            drag.release()
        original.assert_unchanged_now()
        editor.window.get_by_role("button", name="Close Custom").activate()
        saved = wait_for_source_change(conic_scene, baseline)
        original.wait_for_applied(saved, conic_scene.name)
        angle = re.search(rb"from ([0-9.]+)deg", saved)
        assert angle is not None
        assert float(angle.group(1)) == pytest.approx(367, abs=0.001)
        assert b" at " not in saved
        editor.undo()
        original.wait_for_applied(baseline, conic_scene.name)
        editor.redo()
        original.wait_for_applied(saved, conic_scene.name)
        with step(
            "Reopen conic picker",
            layer="adapter",
            trace_coverage="group-only legacy helper",
        ):
            open_conic(editor.window)
        actual = rotation.center(rotation_degrees=277)
        expected = around(c, 126, 367)
        assert actual.x == pytest.approx(expected.x, abs=0.01)
        assert actual.y == pytest.approx(expected.y, abs=0.01)
        editor.window.get_by_role("button", name="Close Custom").activate()
        assert conic_scene.read_bytes() == saved
