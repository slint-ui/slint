# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import wait_for_source
from inspector_interactions import edit_field, inspector_field, wait_for_field
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once, wait_for_source_change
from ui_assertions import expect
from ui_driver import (
    element,
    first_window,
    launch_editor,
    press_shortcut,
    query,
    select_outline_row,
)


@pytest.fixture
def layout_scene(fixture_project: Path) -> Path:
    source = fixture_project / "LayoutSpacing.slint"
    source.write_text("""export component LayoutSpacing inherits Window {
    width: 640px;
    height: 480px;
    layout := HorizontalLayout {
        x: 32px;
        y: 32px;
        width: 400px;
        height: 200px;
        alignment: start;
        padding: 8px;
        spacing: 6px;
        first := Rectangle { width: 80px; background: blue; }
        second := Rectangle { width: 80px; background: green; }
    }
}
""")
    return source


@pytest.mark.parametrize("name,initial", [("padding", 8), ("spacing", 6)])
@pytest.mark.parametrize("outcome", ["commit", "cancel", "source"])
@pytest.mark.parametrize("bound", [True, False], ids=["bound", "default"])
def test_layout_scrub_previews_and_reverts(
    editor_binary,
    editor_environment,
    fixture_project,
    layout_scene,
    name,
    initial,
    outcome,
    bound,
):
    baseline = layout_scene.read_bytes()
    if not bound:
        baseline = replace_once(
            baseline, f"        {name}: {initial}px;\n".encode(), b""
        )
        layout_scene.write_bytes(baseline)
        initial = 0
    expected = (
        replace_once(
            baseline,
            f"{name}: {initial}px;".encode(),
            f"{name}: {initial + 12}px;".encode(),
        )
        if bound
        else None
    )
    external = baseline + b"\n"
    snapshot = SourceSnapshot.capture(fixture_project)
    label = f"Layout {name}"
    with launch_editor(editor_binary, editor_environment, layout_scene) as editor:
        window = first_window(editor)
        select_outline_row(window, "layout")
        child = element(window, id="LayoutSpacing::second")
        before = child.absolute_position.x
        scrubber = inspector_field(
            window, label + " scrubber", slint_testing.AccessibleRole.Slider
        )
        start = center(scrubber)
        end = slint_testing.LogicalPosition(x=start.x + 12, y=start.y)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(
            window, label, str(initial + 12), slint_testing.AccessibleRole.TextInput
        )
        expect.poll(lambda: child.absolute_position.x > before).to_equal(True)
        snapshot.assert_unchanged()
        if outcome == "cancel":
            press_shortcut(window, keys.Escape)
        elif outcome == "source":
            layout_scene.write_bytes(external)
            wait_for_source(layout_scene, external)
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        if outcome == "commit":
            if expected is None:
                expected = wait_for_source_change(layout_scene, baseline)
                assert f"{name}: {initial + 12}px;".encode() in expected
            snapshot.wait_for_applied(expected, layout_scene.name)
            press_shortcut(window, keys.Control, "z")
            snapshot.wait_for_applied(baseline, layout_scene.name)
            expect.poll(lambda: child.absolute_position.x).to_equal(before)
            press_shortcut(window, keys.Control, keys.Shift, "z")
            snapshot.wait_for_applied(expected, layout_scene.name)
        else:
            snapshot.wait_for_applied(
                external if outcome == "source" else baseline, layout_scene.name
            )
            expect.poll(lambda: child.absolute_position.x).to_equal(before)


@pytest.mark.parametrize("name,initial", [("padding", 8), ("spacing", 6)])
@pytest.mark.parametrize(
    "expression",
    [
        "root.base-value",
        "(root.base-value)",
        "root.width / 40",
        "8px + 4px",
        "1cm",
        "{ return root.base-value; }",
    ],
)
def test_layout_bindings_are_editable_without_scrubbing(
    editor_binary,
    editor_environment,
    fixture_project,
    layout_scene,
    name,
    initial,
    expression,
):
    baseline = replace_once(
        layout_scene.read_bytes(),
        b"    height: 480px;\n",
        b"    height: 480px;\n    in-out property <length> base-value: 18px;\n",
    )
    binding = expression if expression.startswith("{") else expression + ";"
    baseline = replace_once(
        baseline, f"{name}: {initial}px;".encode(), f"{name}: {binding}".encode()
    )
    layout_scene.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, layout_scene) as editor:
        window = first_window(editor)
        select_outline_row(window, "layout")
        label = f"Layout {name}"
        field = inspector_field(window, label, slint_testing.AccessibleRole.TextInput)
        expect(field).to_have_value(expression)
        expect(
            query(window, label + " scrubber", role=slint_testing.AccessibleRole.Slider)
        ).to_be_hidden()
        child = element(window, id="LayoutSpacing::second")
        before = child.absolute_position.x
        start = center(field)
        end = slint_testing.LogicalPosition(x=start.x + 12, y=start.y)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        snapshot.assert_unchanged()
        assert child.absolute_position.x == before

        replacement = (
            "{ return root.base-value + 5px; }"
            if expression.startswith("{")
            else "root.base-value + 5px"
        )
        replacement_binding = (
            replacement if replacement.startswith("{") else replacement + ";"
        )
        expected = replace_once(
            baseline,
            f"{name}: {binding}".encode(),
            f"{name}: {replacement_binding}".encode(),
        )
        edit_field(window, label, replacement, slint_testing.AccessibleRole.TextInput)
        snapshot.wait_for_applied(expected, layout_scene.name)
        select_outline_row(window, "layout").single_click(button)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, layout_scene.name)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, layout_scene.name)


@pytest.mark.parametrize("name,initial", [("padding", 8), ("spacing", 6)])
@pytest.mark.parametrize("value", ["0", "2.5", "-1"])
def test_layout_numeric_input(
    editor_binary,
    editor_environment,
    fixture_project,
    layout_scene,
    name,
    initial,
    value,
):
    baseline = layout_scene.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, layout_scene) as editor:
        window = first_window(editor)
        select_outline_row(window, "layout")
        field = inspector_field(
            window, f"Layout {name}", slint_testing.AccessibleRole.TextInput
        )
        assert (
            not field.query_descendants()
            .match_id("EditorInput::fixed-suffix")
            .find_all()
        )
        edit_field(
            window, f"Layout {name}", value, slint_testing.AccessibleRole.TextInput
        )
        if value == "-1":
            snapshot.assert_unchanged()
        else:
            expected = replace_once(
                baseline,
                f"{name}: {initial}px;".encode(),
                f"{name}: {value}px;".encode(),
            )
            snapshot.wait_for_applied(expected, layout_scene.name)
