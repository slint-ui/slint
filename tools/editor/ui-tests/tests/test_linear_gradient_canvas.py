# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
import re
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center_canvas_selection, zoom_canvas
from editor_sync import wait_for_source
from gradient_interactions import center, gesture, shifted
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once, wait_for_source_change
from ui_driver import (
    first_window,
    launch_editor,
    select_outline_row,
    wait_until,
)


@pytest.fixture
def scene(tmp_path):
    path = tmp_path / "LinearGradientScene.slint"
    path.write_text("""export component LinearGradientScene inherits Window {
    width: 400px;
    height: 400px;
    fill := Rectangle {
        width: 200px;
        height: 200px;
        background: @linear-gradient(90deg, #568fb8 0%, #264052 55%, #7e3b66 100%);
    }
}
""")
    return path


def open_linear(window):
    select_outline_row(window, "fill")
    window.get_by_role("button", name="Rectangle background color picker").activate()
    window.get_by_role("button", name="Gradient start").resolve()


@pytest.mark.parametrize("percent", [50, 100, 200])
@pytest.mark.parametrize("rotation", [0, 45, 90, 180])
def test_stop_drag_crosses_neighbors_without_losing_capture(
    editor_binary, editor_environment, scene, tmp_path, rotation, percent
):
    scene.write_text(
        scene.read_text().replace(
            "        width: 200px;",
            f"        transform-rotation: {rotation}deg;\n        width: 200px;",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        zoom_canvas(window, percent)
        center_canvas_selection(window)
        open_linear(window)
        start = center(
            window.get_by_role("button", name="Gradient stop 2").resolve(), rotation
        )

        def destination(distance):
            return shifted(
                start,
                x=distance * percent / 100 * math.cos(math.radians(rotation)),
                y=distance * percent / 100 * math.sin(math.radians(rotation)),
            )

        window.pointer.press_at(start)
        for dx in (30, 70, 100, 130, 70, -50, -120, 20):
            window.pointer.move_to(destination(dx))
            assert float(
                window.get_by_role(
                    slint_testing.AccessibleRole.Slider, name="Gradient stop 2"
                )
                .resolve()
                .accessible_value
            ) == pytest.approx(55 + dx / 2)
            actual = center(
                window.get_by_role("button", name="Gradient stop 2").resolve(), rotation
            )
            assert actual.x == pytest.approx(destination(dx).x, abs=0.001)
            assert actual.y == pytest.approx(destination(dx).y, abs=0.001)
        window.pointer.release_at(destination(20))
        (tmp_path / "gradient-stop-marker.png").write_bytes(window.screenshot())
        window.keyboard.press(keys.Delete)
        assert not window.get_by_accessible_name("Gradient stop 3").all()
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


def test_linear_canvas_activation_and_colour(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        assert not window.get_by_accessible_name("Gradient start").all()
        window.get_by_role(
            "button", name="Rectangle background color picker"
        ).activate()
        start = center(window.get_by_role("button", name="Gradient start").resolve())
        end = center(window.get_by_role("button", name="Gradient end").resolve())
        assert end.x - start.x == pytest.approx(200)
        assert end.y == pytest.approx(start.y)
        assert not window.get_by_accessible_name("Gradient angle degrees").all()
        window.get_by_role("button", name="Add gradient stop").resolve()
        window.get_by_role(
            slint_testing.AccessibleRole.Slider, name="Gradient stop 2"
        ).resolve()
        assert not window.get_by_accessible_name("Hex color").all()
        assert not window.get_by_accessible_name("Close Stop color").all()
        window.get_by_role("button", name="Edit stop 2 color").activate()
        window.get_by_role("button", name="Close Stop color").resolve()
        hex_field = window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Hex color"
        ).resolve()
        assert hex_field.accessible_value == "#264052"
        window.get_by_role("button", name="Gradient stop 1").activate()
        wait_until(
            lambda: hex_field if hex_field.accessible_value == "#568fb8" else None
        )
        assert hex_field.accessible_value == "#568fb8"
        window.get_by_role("button", name="Gradient stop 2").activate()
        wait_until(
            lambda: hex_field if hex_field.accessible_value == "#264052" else None
        )
        assert hex_field.accessible_value == "#264052"
        hex_field.accessible_value = "#12ab3480"
        window.get_by_role("button", name="Close Stop color").activate()
        window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Stop 2 position"
        ).resolve().accessible_value = "70"
        assert center(
            window.get_by_role("button", name="Gradient stop 2").resolve()
        ).x == pytest.approx(start.x + 140)
        window.get_by_role("button", name="Gradient stop 2").activate()
        window.keyboard.press(keys.RightArrow)
        assert float(
            window.get_by_role(
                slint_testing.AccessibleRole.TextInput, name="Stop 2 position"
            )
            .resolve()
            .accessible_value
        ) == pytest.approx(71)
        original.assert_unchanged_now()
        window.get_by_role("button", name="Solid").activate()
        assert not window.get_by_accessible_name("Gradient start").all()
        window.get_by_role("button", name="Gradient").activate()
        window.get_by_role("button", name="Gradient start").resolve()
        original.assert_unchanged_now()
        (tmp_path / "linear-gradient-editor.png").write_bytes(window.screenshot())
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


def test_linear_endpoint_drag_and_session_history(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        start = center(window.get_by_role("button", name="Gradient start").resolve())
        gesture(window, start, shifted(start, x=40))
        assert center(
            window.get_by_role("button", name="Gradient start").resolve()
        ).x == pytest.approx(start.x + 40)
        window.get_by_role("button", name="Add gradient stop").resolve()
        original.assert_unchanged_now()
        window.get_by_role("button", name="Close Custom").activate()
        saved = replace_once(
            original.sources[Path(scene.name)],
            b"@linear-gradient(90deg, #568fb8 0%, #264052 55%, #7e3b66 100%)",
            b"@linear-gradient(90deg, #568fb8 20%, #264052 64%, #7e3b66 100%)",
        )
        original.wait_for_applied(saved, scene.name)
        window.keyboard.shortcut(keys.Control, "z")
        original.wait_for_applied(original.sources[Path(scene.name)], scene.name)
        window.keyboard.shortcut(keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, scene.name)
        open_linear(window)
        assert center(
            window.get_by_role("button", name="Gradient start").resolve()
        ).x == pytest.approx(start.x + 40)


def test_linear_double_click_and_delete(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        window.get_by_role("button", name="Gradient axis").dblclick()
        window.get_by_role("button", name="Gradient stop 4").resolve()
        window.keyboard.press(keys.Delete)
        assert not window.get_by_accessible_name("Gradient stop 4").all()
        window.get_by_role("button", name="Gradient stop 2").activate()
        window.keyboard.press(keys.Delete)
        assert not window.get_by_accessible_name("Gradient stop 3").all()
        window.keyboard.press(keys.Delete)
        window.get_by_role("button", name="Gradient stop 2").resolve()
        window.get_by_role("button", name="Gradient end").resolve()
        original.assert_unchanged_now()
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


@pytest.mark.parametrize("handle", ["Gradient end", "Gradient stop 2"])
def test_linear_drag_escape_restores_gesture(
    editor_binary, editor_environment, scene, tmp_path, handle
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        start = center(window.get_by_role("button", name=handle).resolve())
        end = shifted(start, x=-130, y=-50)
        window.pointer.press_at(start)
        window.pointer.move_to(end)
        window.keyboard.press(keys.Escape)
        window.pointer.release_at(end)
        restored = center(window.get_by_role("button", name=handle).resolve())
        assert restored.x == pytest.approx(start.x)
        assert restored.y == pytest.approx(start.y)
        window.get_by_role("button", name="Close Custom").activate()
        original.assert_unchanged()


@pytest.mark.parametrize("rotation", [0, 45, 90])
def test_linear_axis_translation_tracks_rotated_rectangles(
    editor_binary, editor_environment, scene, tmp_path, rotation
):
    scene.write_text(
        scene.read_text().replace(
            "        width: 200px;",
            f"        transform-rotation: {rotation}deg;\n        width: 200px;",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        start = center(
            window.get_by_role("button", name="Gradient start").resolve(), rotation
        )
        end = center(
            window.get_by_role("button", name="Gradient end").resolve(), rotation
        )
        midpoint = slint_testing.LogicalPosition(
            x=(start.x + end.x) / 2, y=(start.y + end.y) / 2
        )
        gesture(window, midpoint, shifted(midpoint, x=17, y=23))
        moved_start = center(
            window.get_by_role("button", name="Gradient start").resolve(), rotation
        )
        moved_end = center(
            window.get_by_role("button", name="Gradient end").resolve(), rotation
        )
        assert moved_start.x == pytest.approx(start.x + 17, abs=0.02)
        assert moved_start.y == pytest.approx(start.y + 23, abs=0.02)
        assert moved_end.x == pytest.approx(end.x + 17, abs=0.02)
        assert moved_end.y == pytest.approx(end.y + 23, abs=0.02)
        original.assert_unchanged_now()
        window.keyboard.press(keys.Escape)
        original.assert_unchanged()


def test_linear_layout_size_and_keyboard(
    editor_binary, editor_environment, scene, tmp_path
):
    scene.write_text("""export component LinearGradientScene inherits Window {
    width: 400px;
    height: 400px;
    VerticalLayout {
        padding: 100px;
        fill := Rectangle { background: @linear-gradient(90deg, red, blue); }
    }
}
""")
    baseline = scene.read_bytes()
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        start = center(window.get_by_role("button", name="Gradient start").resolve())
        end = center(window.get_by_role("button", name="Gradient end").resolve())
        assert end.x - start.x == pytest.approx(200)
        window.get_by_role("button", name="Gradient end").activate()
        window.keyboard.press(keys.LeftArrow)
        window.keyboard.shortcut(keys.Shift, keys.UpArrow)
        end = center(
            window.get_by_role("button", name="Gradient end").resolve(),
            math.degrees(math.atan2(-10, 199)),
        )
        assert end.x - start.x == pytest.approx(199)
        assert end.y - start.y == pytest.approx(-10, abs=0.001)
        window.get_by_role("button", name="Gradient stop 1").activate()
        window.keyboard.press(keys.RightArrow)
        window.get_by_role("button", name="Close Custom").activate()
        pattern = re.escape(baseline).replace(
            re.escape(b"@linear-gradient(90deg, red, blue)"),
            rb"@linear-gradient\([^()\n]+\)",
        )

        def complete_source() -> bytes | None:
            saved = scene.read_bytes()
            return saved if saved != baseline and re.fullmatch(pattern, saved) else None

        saved = wait_until(complete_source)
        wait_for_source(scene, saved)
        assert b"red, blue" not in saved
        assert b"width: 200px" not in saved


@pytest.mark.parametrize("other_y", [10, 60, 300])
def test_linear_outside_click_accepts_before_selecting_another_rectangle(
    editor_binary, editor_environment, scene, tmp_path, other_y
):
    scene.write_text(
        scene.read_text().replace(
            "    fill := Rectangle {",
            f"    other := Rectangle {{ x: 10px; y: {other_y}px; width: 50px; height: 50px; background: yellow; }}\n    fill := Rectangle {{",
        )
    )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        window.get_by_role("button", name="Edit stop 1 color").activate()
        window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Hex color"
        ).resolve().accessible_value = "#123456"
        other = wait_until(
            lambda: next(
                iter(window.get_by_id("LinearGradientScene::other").all()), None
            )
        )
        gesture(window, center(other), center(other))
        saved = wait_for_source_change(scene, original.sources[Path(scene.name)])
        original.wait_for_applied(saved, scene.name)
        assert b"#123456" in saved
        assert b"background: yellow" in saved
        assert not window.get_by_accessible_name("Gradient start").all()
        assert not window.get_by_accessible_name("Close Custom").all()


def test_linear_external_edit_cancels_stale_draft(
    editor_binary, editor_environment, scene
):

    original = scene.read_text()
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        window.get_by_role("button", name="Edit stop 1 color").activate()
        window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Hex color"
        ).resolve().accessible_value = "#123456"
        external = original.replace("#568fb8", "#abcdef")
        scene.write_text(external)
        wait_for_source(scene, external.encode())
        assert not window.get_by_accessible_name("Gradient start").all()
        assert not window.get_by_accessible_name("Close Custom").all()
        assert scene.read_text() == external
        open_linear(window)
        window.get_by_role("button", name="Edit stop 1 color").activate()
        assert (
            window.get_by_role(slint_testing.AccessibleRole.TextInput, name="Hex color")
            .resolve()
            .accessible_value
            == "#abcdef"
        )


def test_linear_extended_axis_round_trip(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        start = center(window.get_by_role("button", name="Gradient start").resolve())
        gesture(window, start, shifted(start, x=-50))
        end = center(window.get_by_role("button", name="Gradient end").resolve())
        gesture(window, end, shifted(end, x=50))
        window.get_by_role("button", name="Close Custom").activate()
        saved = wait_for_source_change(scene, original.sources[Path(scene.name)])
        original.wait_for_applied(saved, scene.name)
        assert b"0% - 25%" in saved
        assert b"125%" in saved
        open_linear(window)
        assert center(
            window.get_by_role("button", name="Gradient start").resolve()
        ).x == pytest.approx(start.x - 50)
        assert center(
            window.get_by_role("button", name="Gradient end").resolve()
        ).x == pytest.approx(end.x + 50)
