# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import zoom_canvas
from editor_sync import wait_for_source
from gradient_interactions import center, click, control, gesture, shifted
from inspector_interactions import edit_field, wait_for_field
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_driver import (
    elements_with_label,
    first_window,
    launch_editor,
    outline_rows,
    press_key,
    press_keys,
    press_shortcut,
    select_outline_row,
    wait_until,
    window_element_with_label,
)


@pytest.fixture
def canvas_project(tmp_path: Path) -> Path:
    path = tmp_path / "Main.slint"
    path.write_text("""export component Main inherits Window {
    width: 100px;
    height: 80px;
    background: #f0f4ff;
    probe := Rectangle {
        width: parent.width;
        height: parent.height;
        accessible-role: region;
        accessible-label: "Canvas probe";
        background: @linear-gradient(45deg, #dce8ff, #fff2e0);
    }
    marker := Rectangle {
        x: parent.width - 80px;
        y: parent.height - 80px;
        width: 40px;
        height: 40px;
        background: #4488cc;
        accessible-role: region;
        accessible-label: "Canvas marker";
    }
}
""")
    return path


def assert_canvas(window: slint_testing.Window, width: float, height: float) -> None:
    for label in ["Artboard", "Canvas probe"]:

        def sized(label=label):
            element = window_element_with_label(window, label)
            return (
                element
                if abs(element.size.width - width) < 0.1
                and abs(element.size.height - height) < 0.1
                else None
            )

        wait_until(sized)


def clear_selection(
    window: slint_testing.Window, position: slint_testing.LogicalPosition | None = None
) -> None:
    canvas = window_element_with_label(window, "Editor canvas")
    position = position or slint_testing.LogicalPosition(
        x=canvas.absolute_position.x + 8, y=canvas.absolute_position.y + 8
    )
    window.dispatch_event(
        slint_testing.PointerPressEvent(position, slint_testing.PointerEventButton.Left)
    )
    window.dispatch_event(
        slint_testing.PointerReleaseEvent(
            position, slint_testing.PointerEventButton.Left
        )
    )
    window_element_with_label(window, "Project width")


def history(window: slint_testing.Window, redo: bool = False) -> None:
    clear_selection(window)
    modifier = keys.Control
    if redo:
        press_shortcut(window, modifier, keys.Shift, "z")
    else:
        press_shortcut(window, modifier, "z")


@pytest.mark.parametrize(
    "root_type,dimensions",
    [
        ("Window", "width: 100px;\n    height: 80px;"),
        ("Rectangle", "width: 100px;\n    height: 80px;"),
        ("Window", ""),
        ("Base", ""),
        pytest.param(
            "Window",
            "width: 100px; height: 80px; states [ active when true: { width: 120px; height: 90px; } ]",
            id="active-state",
        ),
        pytest.param(
            "Window",
            "in-out property <length> w: 100px; in-out property <length> h: 80px; width <=> w; height <=> h;",
            id="two-way-binding",
        ),
        (
            "Window",
            "min-width: 100px; max-width: 100px; min-height: 80px; max-height: 80px;",
        ),
        (
            "Rectangle",
            "in property <length> wanted: 100px;\n    width: wanted; height: wanted;",
        ),
    ],
)
def test_root_fills_project_canvas(
    editor_binary, editor_environment, canvas_project, root_type, dimensions
):
    source = (
        canvas_project.read_text()
        .replace("inherits Window", f"inherits {root_type}")
        .replace("width: 100px;\n    height: 80px;", dimensions)
    )
    if root_type == "Base":
        source = (
            "component Base inherits Window { width: 100px; height: 80px; }\n" + source
        )
    canvas_project.write_text(source)
    snapshot = SourceSnapshot.capture(canvas_project.parent)
    with launch_editor(editor_binary, editor_environment, canvas_project) as editor:
        window = first_window(editor)
        wait_for_source(canvas_project, source.encode())
        assert_canvas(window, 390, 720)
        assert not (canvas_project.parent / "slint.project.json").exists()
        edit_field(window, "Project width", "640")
        edit_field(window, "Project height", "360")
        assert_canvas(window, 640, 360)
        snapshot.assert_unchanged()
        canvas_project.write_text(source.replace("#f0f4ff", "#ffffff"))
        wait_for_source(canvas_project, canvas_project.read_bytes())
        assert_canvas(window, 640, 360)


def test_project_keyboard_commits_invalid_input_and_selection(
    editor_binary, editor_environment, canvas_project
):
    source = canvas_project.read_bytes()
    with launch_editor(editor_binary, editor_environment, canvas_project) as editor:
        window = first_window(editor)
        wait_for_source(canvas_project, source)
        field = window_element_with_label(window, "Project width")
        field.single_click(slint_testing.PointerEventButton.Left)
        press_keys(window, "500")
        press_key(window, keys.Return)
        assert_canvas(window, 500, 720)
        for value in ["0", "-1", "nan", "inf", "abc"]:
            edit_field(window, "Project width", value)
            wait_for_field(window, "Project width", "500")
            assert_canvas(window, 500, 720)
        select_outline_row(window, "marker")
        assert not elements_with_label(window.root_element, "Project width")
        clear_selection(window)
        field = window_element_with_label(window, "Project height")
        field.single_click(slint_testing.PointerEventButton.Left)
        press_keys(window, "500")
        press_key(window, keys.Tab)
        assert_canvas(window, 500, 500)
        assert canvas_project.read_bytes() == source


def test_project_and_source_history_share_order(
    editor_binary, editor_environment, canvas_project
):
    source = canvas_project.read_bytes()
    snapshot = SourceSnapshot.capture(canvas_project.parent)
    with launch_editor(editor_binary, editor_environment, canvas_project) as editor:
        window = first_window(editor)
        wait_for_source(canvas_project, source)
        edit_field(window, "Project width", "500")
        select_outline_row(window, "marker")
        edit_field(window, "Width", "60")
        changed = replace_once(source, b"width: 40px;", b"width: 60px;")
        snapshot.wait_for_applied(changed)
        clear_selection(window)
        edit_field(window, "Project height", "500")
        history(window)
        assert_canvas(window, 500, 720)
        assert canvas_project.read_bytes() == changed
        history(window)
        snapshot.wait_for_applied(source)
        assert_canvas(window, 500, 720)
        history(window)
        assert_canvas(window, 390, 720)
        history(window, redo=True)
        assert_canvas(window, 500, 720)
        history(window, redo=True)
        snapshot.wait_for_applied(changed)
        history(window, redo=True)
        assert_canvas(window, 500, 500)
        settings = json.loads(
            (canvas_project.parent / "slint.project.json").read_text()
        )
        assert settings["visual-editor"]["canvas"] == {"width": 500, "height": 500}
    with launch_editor(editor_binary, editor_environment, canvas_project) as editor:
        window = first_window(editor)
        wait_for_source(canvas_project, changed)
        assert_canvas(window, 500, 500)
        wait_for_field(window, "Project width", "500")
        wait_for_field(window, "Project height", "500")


def test_large_canvas_scroll_and_shrink(
    editor_binary, editor_environment, canvas_project
):
    source = canvas_project.read_bytes().replace(
        b"background: #4488cc;",
        b"background: @linear-gradient(90deg, #4488cc 0%, #ffffff 50%, #000000 100%);",
    )
    canvas_project.write_bytes(source)
    snapshot = SourceSnapshot.capture(canvas_project.parent)
    with launch_editor(editor_binary, editor_environment, canvas_project) as editor:
        window = first_window(editor)
        wait_for_source(canvas_project, canvas_project.read_bytes())
        edit_field(window, "Project width", "1800")
        edit_field(window, "Project height", "1400")
        assert_canvas(window, 1800, 1400)
        canvas = window_element_with_label(window, "Editor canvas")
        assert canvas.accessible_value == "100%"
        artboard = window_element_with_label(window, "Artboard")
        before = artboard.absolute_position
        target = center(canvas)
        window.dispatch_event(
            slint_testing.PointerScrolledEvent(
                target,
                delta_x=target.x - (before.x + artboard.size.width - 60),
                delta_y=target.y - (before.y + artboard.size.height - 60),
            )
        )
        wait_until(
            lambda: (
                True
                if abs(
                    center(window_element_with_label(window, "Canvas marker")).x
                    - target.x
                )
                < 0.01
                else None
            )
        )
        assert (
            window_element_with_label(window, "Artboard").absolute_position.x
            < before.x - 100
        )
        marker = window_element_with_label(window, "Canvas marker")
        marker.single_click(slint_testing.PointerEventButton.Left)
        wait_for_field(window, "Width", "40")
        edit_field(window, "Width", "50")
        wait_until(
            lambda: (
                True
                if window_element_with_label(window, "Canvas marker").size.width == 50
                else None
            )
        )
        changed = replace_once(source, b"width: 40px;", b"width: 50px;")
        snapshot.wait_for_applied(changed)
        changed_snapshot = SourceSnapshot.capture(canvas_project.parent)
        click(window, "Rectangle background color picker")
        marker = window_element_with_label(window, "Canvas marker")
        start = center(control(window, "Gradient start"))
        end = center(control(window, "Gradient end"))
        assert start.x == pytest.approx(marker.absolute_position.x)
        assert start.y == pytest.approx(marker.absolute_position.y + 20)
        assert end.x == pytest.approx(marker.absolute_position.x + 50)
        stop = center(control(window, "Gradient stop 2"))
        gesture(window, stop, shifted(stop, x=5))
        assert float(
            control(
                window, "Gradient stop 2", slint_testing.AccessibleRole.Slider
            ).accessible_value
        ) == pytest.approx(60)
        press_key(window, keys.Escape)
        changed_snapshot.assert_unchanged()
        canvas = window_element_with_label(window, "Editor canvas")
        clear_selection(
            window,
            slint_testing.LogicalPosition(
                x=canvas.absolute_position.x + canvas.size.width - 20,
                y=canvas.absolute_position.y + canvas.size.height - 20,
            ),
        )
        wait_for_field(window, "Project width", "1800")
        edit_field(window, "Project width", "300")
        edit_field(window, "Project height", "300")
        assert_canvas(window, 300, 300)
        artboard = window_element_with_label(window, "Artboard")
        assert artboard.absolute_position.x > 0
        assert artboard.absolute_position.y > 0


def test_external_settings_change_rejects_commit(
    editor_binary, editor_environment, canvas_project
):
    with launch_editor(editor_binary, editor_environment, canvas_project) as editor:
        window = first_window(editor)
        wait_for_source(canvas_project, canvas_project.read_bytes())
        path = canvas_project.parent / "slint.project.json"
        external = '{"visual-editor":{"version":1,"canvas":{"width":200,"height":200}}}'
        path.write_text(external)
        edit_field(window, "Project width", "500")
        assert_canvas(window, 390, 720)
        assert path.read_text() == external
        window_element_with_label(window, "Project settings error")


@pytest.mark.parametrize("root_type", ["Window", "Rectangle"])
def test_root_source_size_is_editable_without_resizing_canvas(
    editor_binary, editor_environment, canvas_project, root_type
):
    source = canvas_project.read_bytes().replace(
        b"inherits Window", f"inherits {root_type}".encode()
    )
    canvas_project.write_bytes(source)
    snapshot = SourceSnapshot.capture(canvas_project.parent)
    with launch_editor(editor_binary, editor_environment, canvas_project) as editor:
        window = first_window(editor)
        wait_for_source(canvas_project, source)
        select_outline_row(window, outline_rows(window)[0].accessible_label)
        wait_for_field(window, "Root width", "100px")
        assert not elements_with_label(
            window.root_element, f"{root_type} resize bottom-right"
        )
        edit_field(window, "Root width", "250px")
        snapshot.wait_for_applied(
            replace_once(source, b"width: 100px;", b"width: 250px;")
        )
        assert_canvas(window, 390, 720)
        wait_for_field(window, "Root width", "250px")
        assert not (canvas_project.parent / "slint.project.json").exists()


@pytest.mark.parametrize("percent", [50, 200])
def test_project_resize_preserves_zoom(
    editor_binary, editor_environment, canvas_project, percent
):
    snapshot = SourceSnapshot.capture(canvas_project.parent)
    with launch_editor(editor_binary, editor_environment, canvas_project) as editor:
        window = first_window(editor)
        wait_for_source(canvas_project, canvas_project.read_bytes())
        clear_selection(window)
        zoom_canvas(window, percent)
        edit_field(window, "Project width", "640")
        edit_field(window, "Project height", "360")
        assert_canvas(window, 640, 360)
        assert (
            window_element_with_label(window, "Editor canvas").accessible_value
            == f"{percent}%"
        )
        select_outline_row(window, outline_rows(window)[0].accessible_label)
        frame = window_element_with_label(window, "Selected Window")
        assert frame.size.width == pytest.approx(640 * percent / 100)
        assert frame.size.height == pytest.approx(360 * percent / 100)
        snapshot.assert_unchanged()


def test_canvas_size_survives_file_switches(
    editor_binary, editor_environment, canvas_project
):
    source = canvas_project.read_text()
    second = canvas_project.with_name("Other.slint")
    second.write_text(
        source.replace("component Main", "component Other")
        .replace("100px", "900px")
        .replace("80px", "600px")
    )
    snapshot = SourceSnapshot.capture(canvas_project.parent)
    with launch_editor(editor_binary, editor_environment, canvas_project) as editor:
        window = first_window(editor)
        wait_for_source(canvas_project, source.encode())
        edit_field(window, "Project width", "640")
        edit_field(window, "Project height", "360")
        for path in [second, canvas_project]:
            window_element_with_label(window, str(path)).single_click(
                slint_testing.PointerEventButton.Left
            )
            wait_for_source(path, path.read_bytes())
            assert_canvas(window, 640, 360)
            wait_for_field(window, "Project width", "640")
        snapshot.assert_unchanged()
