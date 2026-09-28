# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math

import pytest
from editor_sync import wait_for_source
from gradient_interactions import center, gesture, open_radial, shifted
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    first_window,
    launch_editor,
)


@pytest.mark.parametrize(
    "label", ["Gradient center handle", "Gradient radius handle", "Gradient stop 2"]
)
def test_escape_restores_radial_gesture(
    editor_binary, editor_environment, radial_scene, tmp_path, label
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        start = center(window.get_by_role("button", name=label).resolve(), 35)
        end = shifted(start, x=25, y=-15)
        window.pointer.press_at(start)
        window.pointer.move_to(end)
        window.keyboard.press(keys.Escape)
        window.pointer.release_at(end)
        restored = center(window.get_by_role("button", name=label).resolve(), 35)
        assert restored.x == pytest.approx(start.x, abs=0.001)
        assert restored.y == pytest.approx(start.y, abs=0.001)
        window.get_by_role("button", name="Close Custom").wait_for()
        window.get_by_role("button", name="Close Custom").activate()
        original.assert_unchanged()


def test_radial_keyboard_and_collapsed_radius(
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
        gesture(window, r, c)
        unchanged = center(
            window.get_by_role("button", name="Gradient radius handle").resolve(), 35
        )
        assert unchanged.x == pytest.approx(r.x, abs=0.001)
        assert unchanged.y == pytest.approx(r.y, abs=0.001)
        window.get_by_role("button", name="Gradient center handle").activate()
        window.keyboard.press(keys.RightArrow)
        window.keyboard.shortcut(keys.Shift, keys.DownArrow)
        after = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 35
        )
        end = center(
            window.get_by_role("button", name="Gradient radius handle").resolve(), 35
        )
        assert after.x == pytest.approx(c.x + 1, abs=0.001)
        assert after.y == pytest.approx(c.y + 10, abs=0.001)
        assert math.hypot(end.x - after.x, end.y - after.y) == pytest.approx(
            math.hypot(r.x - c.x, r.y - c.y), abs=0.001
        )
        window.get_by_role("button", name="Gradient stop 2").activate()
        window.keyboard.press(keys.RightArrow)
        window.keyboard.shortcut(keys.Shift, keys.LeftArrow)
        assert float(
            window.get_by_role("text-input", name="Stop 2 position")
            .resolve()
            .accessible_value
        ) == pytest.approx(36)
        window.keyboard.press(keys.Backspace)
        window.keyboard.press(keys.Backspace)
        window.get_by_role("button", name="Gradient stop 2").wait_for()
        window.get_by_accessible_name("Gradient stop 3").wait_for(state="hidden")
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


def test_external_edit_invalidates_radial_session(
    editor_binary, editor_environment, radial_scene
):

    original = radial_scene.read_text()
    with launch_editor(editor_binary, editor_environment, radial_scene) as editor:
        wait_for_source(radial_scene, radial_scene.read_bytes())
        window = first_window(editor)
        open_radial(window)
        c = center(
            window.get_by_role("button", name="Gradient center handle").resolve(), 35
        )
        gesture(window, c, shifted(c, x=25, y=15))
        external = original.replace("#7e3b66", "#abcdef")
        radial_scene.write_text(external)
        wait_for_source(radial_scene, external.encode())
        window.get_by_accessible_name("Gradient center handle").wait_for(state="hidden")
        window.get_by_accessible_name("Close Custom").wait_for(state="hidden")
        assert radial_scene.read_text() == external
        open_radial(window)
        window.get_by_role("button", name="Edit stop 1 color").activate()
        assert (
            window.get_by_role("text-input", name="Hex color")
            .resolve()
            .accessible_value
            == "#abcdef"
        )
