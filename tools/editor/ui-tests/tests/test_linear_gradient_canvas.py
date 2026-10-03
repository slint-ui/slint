# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import math
import re
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center_canvas_selection, zoom_canvas
from editor_sync import wait_for_source
from gradient_interactions import center, click, control, gesture, shifted
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once, wait_for_source_change
from ui_assertions import expect
from ui_driver import (
    element,
    elements,
    first_window,
    launch_editor,
    press_key,
    press_keys,
    press_shortcut,
    query,
    screenshot,
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
    click(window, "Rectangle background color picker")
    control(window, "Gradient start")


@pytest.mark.parametrize("text", ("ff", "12ab3"))
def test_gradient_stop_partial_hex_typing_cancels_without_source_changes(
    editor_binary, editor_environment, scene, tmp_path, text
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        field = control(window, "Stop 2 color", slint_testing.AccessibleRole.TextInput)
        field.invoke_accessible_default_action()
        press_keys(window, text)
        assert field.accessible_value == text
        original.assert_unchanged_now()
        screenshot(window).save(tmp_path / "partial-hex.png")
        press_key(window, keys.Escape)
        expect(query(window, "Close Custom")).to_be_hidden()
        original.assert_unchanged()
        open_linear(window)
        assert (
            control(
                window, "Stop 2 color", slint_testing.AccessibleRole.TextInput
            ).accessible_value
            == "264052"
        )
        assert (
            control(
                window, "Stop 2 color opacity", slint_testing.AccessibleRole.TextInput
            ).accessible_value
            == "100"
        )
        click(window, "Close Custom")
        original.assert_unchanged()


@pytest.mark.parametrize("surface", ("inline", "stop-picker", "solid-picker"))
@pytest.mark.parametrize("alpha", ("", "7f"))
def test_live_rgb_typing_preserves_edit_start_alpha(
    editor_binary, editor_environment, scene, tmp_path, surface, alpha
):
    scene.write_text(scene.read_text().replace("#264052", "#264052" + alpha))
    if surface == "solid-picker":
        scene.write_text(
            scene.read_text().replace(
                "@linear-gradient(90deg, #568fb8 0%, #264052"
                + alpha
                + " 55%, #7e3b66 100%)",
                "#264052" + alpha,
            )
        )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        click(window, "Rectangle background color picker")
        label = "Stop 2 color" if surface == "inline" else "Hex color"
        if surface == "stop-picker":
            click(window, "Edit stop 2 color")
        field = control(window, label, slint_testing.AccessibleRole.TextInput)
        field.invoke_accessible_default_action()
        press_keys(window, "123456")
        press_key(window, keys.Return)
        assert field.accessible_value == "123456"
        assert control(
            window, label + " opacity", slint_testing.AccessibleRole.TextInput
        ).accessible_value == ("50" if alpha else "100")
        original.assert_unchanged_now()
        screenshot(window).save(tmp_path / "alpha-edit.png")
        if surface == "stop-picker":
            click(window, "Close Stop color")
        click(window, "Close Custom")
        expected = replace_once(
            original.sources[Path(scene.name)],
            ("#264052" + alpha).encode(),
            ("#123456" + alpha).encode(),
        )
        original.wait_for_applied(expected, scene.name)
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(original.sources[Path(scene.name)], scene.name)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(expected, scene.name)


@pytest.mark.parametrize("boundary", ("accept", "blur"))
def test_live_rgb_typing_recaptures_alpha_for_the_next_edit(
    editor_binary, editor_environment, scene, tmp_path, boundary
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        field = control(window, "Stop 2 color", slint_testing.AccessibleRole.TextInput)
        field.invoke_accessible_default_action()
        press_keys(window, "#abcdefab")
        if boundary == "accept":
            press_key(window, keys.Return)
            press_shortcut(window, keys.Control, "a")
        else:
            control(
                window, "Stop 2 color opacity", slint_testing.AccessibleRole.TextInput
            ).invoke_accessible_default_action()
            field.invoke_accessible_default_action()
        press_keys(window, "654321")
        press_key(window, keys.Return)
        original.assert_unchanged_now()
        click(window, "Close Custom")
        expected = replace_once(
            original.sources[Path(scene.name)], b"#264052 55%", b"#654321ab 55%"
        )
        original.wait_for_applied(expected, scene.name)


@pytest.mark.parametrize("surface", ("inline", "stop-picker", "solid-picker"))
@pytest.mark.parametrize("outcome", ("click", "revert"))
def test_opacity_scrub_cancellation_preserves_exact_alpha(
    editor_binary, editor_environment, scene, tmp_path, surface, outcome
):
    scene.write_text(scene.read_text().replace("#264052", "#2640527f"))
    if surface == "solid-picker":
        scene.write_text(
            scene.read_text().replace(
                "@linear-gradient(90deg, #568fb8 0%, #2640527f 55%, #7e3b66 100%)",
                "#2640527f",
            )
        )
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        click(window, "Rectangle background color picker")
        label = "Stop 2 color" if surface == "inline" else "Hex color"
        if surface == "stop-picker":
            click(window, "Edit stop 2 color")
        scrubber = control(
            window, label + " opacity scrubber", slint_testing.AccessibleRole.Slider
        )
        start = center(scrubber)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        if outcome == "revert":
            window.dispatch_event(slint_testing.PointerMoveEvent(shifted(start, x=-20)))
            expect(
                control(
                    window, label + " opacity", slint_testing.AccessibleRole.TextInput
                )
            ).to_have_value("30")
            window.dispatch_event(slint_testing.PointerMoveEvent(start))
            expect(
                control(
                    window, label + " opacity", slint_testing.AccessibleRole.TextInput
                )
            ).to_have_value("50")
        window.dispatch_event(slint_testing.PointerReleaseEvent(start, button))
        original.assert_unchanged_now()
        if surface == "stop-picker":
            click(window, "Close Stop color")
        click(window, "Close Custom")
        original.assert_unchanged()


def test_gradient_stop_rows_edit_color_and_opacity_without_opening_stop_panel(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        color = control(window, "Stop 2 color", slint_testing.AccessibleRole.TextInput)
        opacity = control(
            window, "Stop 2 color opacity", slint_testing.AccessibleRole.TextInput
        )
        assert color.accessible_value == "264052"
        assert opacity.accessible_value == "100"
        assert not elements(window, "Close Stop color")

        color.invoke_accessible_default_action()
        press_key(window, keys.Delete)
        control(window, "Gradient stop 3", slint_testing.AccessibleRole.Slider)
        original.assert_unchanged_now()

        color.accessible_value = "12AB34"
        opacity.accessible_value = "50"
        assert color.accessible_value == "12AB34"
        assert opacity.accessible_value == "50"
        assert not elements(window, "Close Stop color")
        original.assert_unchanged_now()

        click(window, "Close Custom")
        expected = replace_once(
            original.sources[Path(scene.name)],
            b"#264052 55%",
            b"#12ab3480 55%",
        )
        original.wait_for_applied(expected, scene.name)
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(original.sources[Path(scene.name)], scene.name)


@pytest.mark.parametrize("outcome", ("commit", "revert", "cancel"))
def test_gradient_stop_opacity_scrub_keeps_capture(
    editor_binary, editor_environment, scene, tmp_path, outcome
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        scrubber = element(
            window,
            "Stop 2 color opacity scrubber",
            role=slint_testing.AccessibleRole.Slider,
            tracking=False,
        )
        start = center(scrubber)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        for delta in (-10, -35, -20):
            window.dispatch_event(
                slint_testing.PointerMoveEvent(shifted(start, x=delta))
            )
            expect(
                control(
                    window,
                    "Stop 2 color opacity",
                    slint_testing.AccessibleRole.TextInput,
                )
            ).to_have_value(str(100 + delta))
            assert scrubber.is_valid
            original.assert_unchanged_now()

        if outcome == "cancel":
            press_key(window, keys.Escape)
            window.dispatch_event(
                slint_testing.PointerReleaseEvent(shifted(start, x=-20), button)
            )
            expect(query(window, "Close Custom")).to_be_hidden()
            original.assert_unchanged()
            return

        end = shifted(start, x=-20)
        if outcome == "revert":
            end = start
            window.dispatch_event(slint_testing.PointerMoveEvent(end))
            expect(
                control(
                    window,
                    "Stop 2 color opacity",
                    slint_testing.AccessibleRole.TextInput,
                )
            ).to_have_value("100")
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        original.assert_unchanged_now()
        click(window, "Close Custom")
        if outcome == "revert":
            original.assert_unchanged()
            return

        expected = replace_once(
            original.sources[Path(scene.name)], b"#264052 55%", b"#264052cc 55%"
        )
        original.wait_for_applied(expected, scene.name)
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(original.sources[Path(scene.name)], scene.name)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(expected, scene.name)


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
        start = center(control(window, "Gradient stop 2"), rotation)

        def destination(distance):
            return shifted(
                start,
                x=distance * percent / 100 * math.cos(math.radians(rotation)),
                y=distance * percent / 100 * math.sin(math.radians(rotation)),
            )

        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        for dx in (30, 70, 100, 130, 70, -50, -120, 20):
            window.dispatch_event(slint_testing.PointerMoveEvent(destination(dx)))
            assert float(
                control(
                    window, "Gradient stop 2", slint_testing.AccessibleRole.Slider
                ).accessible_value
            ) == pytest.approx(55 + dx / 2)
            actual = center(control(window, "Gradient stop 2"), rotation)
            assert actual.x == pytest.approx(destination(dx).x, abs=0.001)
            assert actual.y == pytest.approx(destination(dx).y, abs=0.001)
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(destination(20), button)
        )
        press_key(window, keys.Delete)
        assert not elements(window, "Gradient stop 3")
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_linear_canvas_activation_and_colour(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        select_outline_row(window, "fill")
        assert not elements(window, "Gradient start")
        click(window, "Rectangle background color picker")
        start = center(control(window, "Gradient start"))
        end = center(control(window, "Gradient end"))
        assert end.x - start.x == pytest.approx(200)
        assert end.y == pytest.approx(start.y)
        assert not elements(window, "Gradient angle degrees")
        control(window, "Add gradient stop")
        control(window, "Gradient stop 2", slint_testing.AccessibleRole.Slider)
        assert not elements(window, "Hex color")
        assert not elements(window, "Close Stop color")
        click(window, "Edit stop 2 color")
        control(window, "Close Stop color")
        hex_field = control(window, "Hex color", slint_testing.AccessibleRole.TextInput)
        expect(hex_field).to_have_value("264052")
        click(window, "Gradient stop 1")
        expect(hex_field).to_have_value("568FB8")
        click(window, "Gradient stop 2")
        expect(hex_field).to_have_value("264052")
        hex_field.accessible_value = "#12ab3480"
        click(window, "Close Stop color")
        control(
            window, "Stop 2 position", slint_testing.AccessibleRole.TextInput
        ).accessible_value = "70"
        assert center(control(window, "Gradient stop 2")).x == pytest.approx(
            start.x + 140
        )
        click(window, "Gradient stop 2")
        press_key(window, keys.RightArrow)
        assert float(
            control(
                window, "Stop 2 position", slint_testing.AccessibleRole.TextInput
            ).accessible_value
        ) == pytest.approx(71)
        original.assert_unchanged_now()
        click(window, "Solid")
        assert not elements(window, "Gradient start")
        click(window, "Gradient")
        control(window, "Gradient start")
        original.assert_unchanged_now()
        press_key(window, keys.Escape)
        original.assert_unchanged()


def test_linear_endpoint_drag_and_session_history(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        start = center(control(window, "Gradient start"))
        gesture(window, start, shifted(start, x=40))
        assert center(control(window, "Gradient start")).x == pytest.approx(
            start.x + 40
        )
        control(window, "Add gradient stop")
        original.assert_unchanged_now()
        click(window, "Close Custom")
        saved = replace_once(
            original.sources[Path(scene.name)],
            b"@linear-gradient(90deg, #568fb8 0%, #264052 55%, #7e3b66 100%)",
            b"@linear-gradient(90deg, #568fb8 20%, #264052 64%, #7e3b66 100%)",
        )
        original.wait_for_applied(saved, scene.name)
        press_shortcut(window, keys.Control, "z")
        original.wait_for_applied(original.sources[Path(scene.name)], scene.name)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        original.wait_for_applied(saved, scene.name)
        open_linear(window)
        assert center(control(window, "Gradient start")).x == pytest.approx(
            start.x + 40
        )


def test_linear_double_click_and_delete(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        axis = control(window, "Gradient axis")
        axis.double_click(slint_testing.PointerEventButton.Left)
        control(window, "Gradient stop 4")
        press_key(window, keys.Delete)
        assert not elements(window, "Gradient stop 4")
        click(window, "Gradient stop 2")
        press_key(window, keys.Delete)
        assert not elements(window, "Gradient stop 3")
        press_key(window, keys.Delete)
        control(window, "Gradient stop 2")
        control(window, "Gradient end")
        original.assert_unchanged_now()
        press_key(window, keys.Escape)
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
        start = center(control(window, handle))
        end = shifted(start, x=-130, y=-50)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        press_key(window, keys.Escape)
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        restored = center(control(window, handle))
        assert restored.x == pytest.approx(start.x)
        assert restored.y == pytest.approx(start.y)
        click(window, "Close Custom")
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
        start = center(control(window, "Gradient start"), rotation)
        end = center(control(window, "Gradient end"), rotation)
        midpoint = slint_testing.LogicalPosition(
            x=(start.x + end.x) / 2, y=(start.y + end.y) / 2
        )
        gesture(window, midpoint, shifted(midpoint, x=17, y=23))
        moved_start = center(control(window, "Gradient start"), rotation)
        moved_end = center(control(window, "Gradient end"), rotation)
        assert moved_start.x == pytest.approx(start.x + 17, abs=0.02)
        assert moved_start.y == pytest.approx(start.y + 23, abs=0.02)
        assert moved_end.x == pytest.approx(end.x + 17, abs=0.02)
        assert moved_end.y == pytest.approx(end.y + 23, abs=0.02)
        original.assert_unchanged_now()
        press_key(window, keys.Escape)
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
        start = center(control(window, "Gradient start"))
        end = center(control(window, "Gradient end"))
        assert end.x - start.x == pytest.approx(200)
        click(window, "Gradient end")
        press_key(window, keys.LeftArrow)
        press_shortcut(window, keys.Shift, keys.UpArrow)
        end = center(
            control(window, "Gradient end"), math.degrees(math.atan2(-10, 199))
        )
        assert end.x - start.x == pytest.approx(199)
        assert end.y - start.y == pytest.approx(-10, abs=0.001)
        click(window, "Gradient stop 1")
        press_key(window, keys.RightArrow)
        click(window, "Close Custom")
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
        click(window, "Edit stop 1 color")
        control(
            window, "Hex color", slint_testing.AccessibleRole.TextInput
        ).accessible_value = "#123456"
        other = wait_until(
            lambda: next(iter(elements(window, id="LinearGradientScene::other")), None)
        )
        gesture(window, center(other), center(other))
        saved = wait_for_source_change(scene, original.sources[Path(scene.name)])
        original.wait_for_applied(saved, scene.name)
        assert b"#123456" in saved
        assert b"background: yellow" in saved
        assert not elements(window, "Gradient start")
        assert not elements(window, "Close Custom")


def test_linear_external_edit_cancels_stale_draft(
    editor_binary, editor_environment, scene
):

    original = scene.read_text()
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        click(window, "Edit stop 1 color")
        control(
            window, "Hex color", slint_testing.AccessibleRole.TextInput
        ).accessible_value = "#123456"
        external = original.replace("#568fb8", "#abcdef")
        scene.write_text(external)
        wait_for_source(scene, external.encode())
        assert not elements(window, "Gradient start")
        assert not elements(window, "Close Custom")
        assert scene.read_text() == external
        open_linear(window)
        click(window, "Edit stop 1 color")
        assert (
            control(
                window, "Hex color", slint_testing.AccessibleRole.TextInput
            ).accessible_value
            == "ABCDEF"
        )


def test_linear_extended_axis_round_trip(
    editor_binary, editor_environment, scene, tmp_path
):
    original = SourceSnapshot.capture(tmp_path)
    with launch_editor(editor_binary, editor_environment, scene) as editor:
        wait_for_source(scene, scene.read_bytes())
        window = first_window(editor)
        open_linear(window)
        start = center(control(window, "Gradient start"))
        gesture(window, start, shifted(start, x=-50))
        end = center(control(window, "Gradient end"))
        gesture(window, end, shifted(end, x=50))
        click(window, "Close Custom")
        saved = wait_for_source_change(scene, original.sources[Path(scene.name)])
        original.wait_for_applied(saved, scene.name)
        assert b"0% - 25%" in saved
        assert b"125%" in saved
        open_linear(window)
        assert center(control(window, "Gradient start")).x == pytest.approx(
            start.x - 50
        )
        assert center(control(window, "Gradient end")).x == pytest.approx(end.x + 50)
