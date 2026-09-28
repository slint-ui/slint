# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
import re
from pathlib import Path

import pytest
from canvas_interactions import center_canvas_selection, zoom_canvas
from editor_sync import wait_for_source
from gradient_interactions import center, gesture, open_radial, shifted
from slint_test import expect
from slint_testing import keys
from source_snapshot import SourceSnapshot, wait_for_source_change
from ui_driver import (
    first_window,
    launch_editor,
    select_outline_row,
)


def test_radial_activation_preserves_the_actual_picker(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    radial_scene.write_text(
        re.sub(r"@radial-gradient\([^;]+\)", "#7e3b66", radial_scene.read_text())
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        window.get_by_accessible_name("Gradient center handle").wait_for(state="hidden")
        window.get_by_role(
            "button", name="Rectangle background color picker"
        ).activate()
        window.get_by_accessible_name("Gradient center handle").wait_for(state="hidden")
        window.get_by_role("button", name="Gradient").activate()
        window.get_by_role("combobox", name="Gradient type").set_accessible_value(
            "Radial"
        )
        window.get_by_role("button", name="Gradient center handle").wait_for()
        window.get_by_role("button", name="Gradient radius handle").wait_for()
        window.get_by_role("slider", name="Gradient stop 1").wait_for()
        window.get_by_role("button", name="Edit stop 1 color").wait_for()
        window.get_by_accessible_name("Hex color").wait_for(state="hidden")
        (tmp_path / "radial-picker-and-canvas.png").write_bytes(window.screenshot())
        window.get_by_role("button", name="Solid").activate()
        window.get_by_accessible_name("Gradient center handle").wait_for(state="hidden")
        window.get_by_role("text-input", name="Hex color").wait_for()
        window.get_by_role("button", name="Gradient").activate()
        window.get_by_role("button", name="Gradient center handle").wait_for()
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


@pytest.mark.parametrize("rotation", [0, 45, 90])
@pytest.mark.parametrize("handle", ["Gradient center handle", "Gradient axis"])
def test_radial_translation(
    editor_binary, editor_environment, radial_scene, tmp_path, rotation, handle
):
    radial_scene.write_text(
        radial_scene.read_text().replace(
            "        width: 200px;",
            f"        transform-rotation: {rotation}deg;\n        width: 200px;",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(),
            rotation + 35,
        )
        r = center(
            window.get_by_role("button", name="Gradient radius handle").resolve(),
            rotation + 35,
        )
        p = (
            c
            if handle == "Gradient center handle"
            else shifted(c, x=(r.x - c.x) * 0.7, y=(r.y - c.y) * 0.7)
        )
        gesture(window, p, shifted(p, x=17, y=23))
        after = center(
            window.get_by_role("button", name="Gradient center handle").resolve(),
            rotation + 35,
        )
        end = center(
            window.get_by_role("button", name="Gradient radius handle").resolve(),
            rotation + 35,
        )
        assert after.x == pytest.approx(c.x + 17, abs=0.01)
        assert after.y == pytest.approx(c.y + 23, abs=0.01)
        assert end.x == pytest.approx(r.x + 17, abs=0.01)
        assert end.y == pytest.approx(r.y + 23, abs=0.01)
        original.assert_unchanged_now()
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


@pytest.mark.parametrize("percent", [50, 100, 200])
def test_radial_radius_save_reopen_and_history(
    editor_binary, editor_environment, radial_scene, tmp_path, percent
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        zoom_canvas(window, percent)
        center_canvas_selection(window)
        open_radial(window)
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 35
        )
        r = center(
            window.get_by_role("button", name="Gradient radius handle").resolve(), 35
        )
        gesture(window, r, shifted(c, x=percent))
        window.get_by_role("button", name="Close Custom").activate()
        saved = wait_for_source_change(
            radial_scene, original.sources[Path(radial_scene.name)]
        )
        radius = re.search(rb"circle ([0-9.]+)px", saved)
        assert radius is not None
        assert float(radius.group(1)) == pytest.approx(100, abs=0.001)
        original.wait_for_applied(saved, radial_scene.name)
        assert b" at " not in saved
        window.keyboard.shortcut(keys.Control, "z")
        original.wait_for_applied(
            original.sources[Path(radial_scene.name)], radial_scene.name
        )
        window.keyboard.shortcut(keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, radial_scene.name)
        open_radial(window)
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 35
        )
        r = center(
            window.get_by_role("button", name="Gradient radius handle").resolve(), 35
        )
        assert math.hypot(r.x - c.x, r.y - c.y) == pytest.approx(percent, abs=0.001)


def test_radial_guide_rotation_and_noop_do_not_write_source(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 35
        )
        r = center(
            window.get_by_role("button", name="Gradient radius handle").resolve(), 35
        )
        gesture(window, r, shifted(c, x=-math.hypot(r.x - c.x, r.y - c.y)))
        window.get_by_role("button", name="Close Custom").activate()
        original.assert_unchanged()
        open_radial(window)
        window.get_by_role("button", name="Close Custom").activate()
        original.assert_unchanged()


def test_radial_stops_cross_insert_delete_and_color(
    editor_binary, editor_environment, radial_scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        start = center(
            window.get_by_role("button", name="Gradient stop 2").resolve(), 35
        )
        window.pointer.press_at(start)
        for distance in [40, 100, -80, 30]:
            p = shifted(
                start,
                x=distance * math.cos(math.radians(35)),
                y=distance * math.sin(math.radians(35)),
            )
            window.pointer.move_to(p)
            actual = center(
                window.get_by_role("button", name="Gradient stop 2").resolve(), 35
            )
            assert actual.x == pytest.approx(p.x, abs=0.001)
            assert actual.y == pytest.approx(p.y, abs=0.001)
        window.pointer.release_at(p)
        window.get_by_role("button", name="Edit stop 2 color").activate()
        field = window.get_by_role("text-input", name="Hex color")
        expect(field).to_have_value("#264052")
        field.set_accessible_value("#abcdef80")
        window.get_by_role("button", name="Close Stop color").activate()
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 35
        )
        r = center(
            window.get_by_role("button", name="Gradient radius handle").resolve(), 35
        )
        p = shifted(c, x=(r.x - c.x) * 0.3, y=(r.y - c.y) * 0.3)
        gesture(window, p, p)
        gesture(window, p, p)
        window.get_by_role("button", name="Gradient stop 4").wait_for()
        window.keyboard.press(keys.Delete)
        window.get_by_accessible_name("Gradient stop 4").wait_for(state="hidden")
        window.get_by_role("button", name="Gradient stop 2").activate()
        window.keyboard.press(keys.Delete)
        window.keyboard.press(keys.Delete)
        window.get_by_role("button", name="Gradient center handle").wait_for()
        window.get_by_role("button", name="Gradient stop 2").wait_for()
        (tmp_path / "radial-gradient-editor.png").write_bytes(window.screenshot())
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()
