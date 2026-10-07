# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# cspell:ignore tobytes

import json
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from inspector_interactions import inspector_field
from slint_testing import keys
from ui_driver import (
    element,
    first_window,
    launch_editor,
    screenshot,
    select_outline_row,
    wait_until,
)


def wait_for_pane_settings(
    environment: dict[str, str], expected: dict[str, int | None]
) -> None:
    directory = Path(environment["HOME"]).parent
    settings_path: Path | None = None
    last_settings: dict | None = None

    def saved_heights() -> bool | None:
        nonlocal settings_path, last_settings
        paths = list(directory.rglob("visual-editor-user-settings.json"))
        assert len(paths) <= 1, paths
        if not paths:
            return None
        settings_path = paths[0]
        last_settings = json.loads(settings_path.read_text())
        if all(last_settings.get(key) == value for key, value in expected.items()):
            return True
        return None

    try:
        wait_until(saved_heights)
    except AssertionError as error:
        raise AssertionError(
            f"Pane settings did not match {expected!r}; "
            f"file: {settings_path or directory / '**/visual-editor-user-settings.json'}; "
            f"last observed settings: {last_settings!r}"
        ) from error


def drag_divider(
    window: slint_testing.Window,
    element: slint_testing.Element,
    delta: float,
    *,
    horizontal: bool = False,
) -> None:
    start = center(element)
    end = slint_testing.LogicalPosition(
        x=start.x + (delta if horizontal else 0),
        y=start.y + (0 if horizontal else delta),
    )
    button = slint_testing.PointerEventButton.Left
    window.dispatch_event(slint_testing.PointerPressEvent(start, button))
    window.dispatch_event(slint_testing.PointerMoveEvent(end))
    window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))


def double_click(window: slint_testing.Window, element: slint_testing.Element) -> None:
    position = center(element)
    button = slint_testing.PointerEventButton.Left
    for _ in range(2):
        window.dispatch_event(slint_testing.PointerPressEvent(position, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(position, button))


@pytest.mark.parametrize(
    ("label", "pane_label", "setting", "direction"),
    [
        ("Project pane resize", "Project and elements", "left_pane_width", 1),
        (
            "Inspector pane resize",
            "Inspector and outline",
            "inspector_pane_width",
            -1,
        ),
    ],
)
def test_pane_width_drag_limits_reset_and_persistence(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    label: str,
    pane_label: str,
    setting: str,
    direction: int,
) -> None:
    source_file = fixture_project / "Main.slint"
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        divider = element(window, label)
        pane = element(window, pane_label)
        minimum = int(divider.accessible_value_minimum)
        maximum = divider.accessible_value_maximum
        delta = int((maximum - minimum) / 2)
        step = divider.accessible_value_step
        assert pane.size.width == pytest.approx(minimum)
        drag_divider(window, divider, direction * delta, horizontal=True)
        assert pane.size.width == pytest.approx(minimum + delta, abs=1)
        wait_for_pane_settings(editor_environment, {setting: minimum + delta})
        element(window, "Collapse sidebars").invoke_accessible_default_action()
        element(window, "Expand sidebars").invoke_accessible_default_action()
        assert pane.size.width == pytest.approx(minimum + delta, abs=1)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        divider = element(window, label)
        pane = element(window, pane_label)
        assert pane.size.width == pytest.approx(minimum + delta, abs=1)
        divider.invoke_accessible_increment_action()
        assert pane.size.width == pytest.approx(minimum + delta + step, abs=1)
        divider.invoke_accessible_decrement_action()
        assert pane.size.width == pytest.approx(minimum + delta, abs=1)
        divider.accessible_value = "10000"
        assert pane.size.width == pytest.approx(maximum, abs=1)
        divider.accessible_value = "0"
        assert pane.size.width == pytest.approx(minimum, abs=1)
        divider.accessible_value = str(minimum + delta)
        double_click(window, divider)
        assert pane.size.width == pytest.approx(minimum, abs=1)
        wait_for_pane_settings(editor_environment, {setting: None})
        drag_divider(window, divider, direction * 2000, horizontal=True)
        assert pane.size.width == pytest.approx(maximum, abs=1)
        drag_divider(window, divider, -direction * 2000, horizontal=True)
        assert pane.size.width == pytest.approx(minimum, abs=1)


@pytest.mark.parametrize(
    ("row", "field", "role"),
    [
        (
            "inspect-rectangle",
            "Rectangle effect",
            slint_testing.AccessibleRole.Combobox,
        ),
        ("inspect-text", "Font family", slint_testing.AccessibleRole.TextInput),
        ("inspect-image", "Image fit", slint_testing.AccessibleRole.Combobox),
    ],
)
def test_wide_inspector_fields_follow_pane_width(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    field: str,
    role: slint_testing.AccessibleRole,
) -> None:
    source_file = fixture_project / "InspectorCases.slint"
    source = source_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_outline_row(window, row)
        control = inspector_field(window, field, role)
        initial_width = control.size.width
        divider = element(window, "Inspector pane resize")
        delta = int(
            (divider.accessible_value_maximum - divider.accessible_value_minimum) / 2
        )
        drag_divider(window, divider, -delta, horizontal=True)
        assert control.size.width == pytest.approx(initial_width + delta, abs=1)
        assert source_file.read_bytes() == source


def test_pane_resize_handles_preserve_idle_edges(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        left = element(window, "Project and elements")
        right = element(window, "Inspector and outline")
        canvas = element(window, "Editor canvas")
        assert canvas.absolute_position.x == left.absolute_position.x + left.size.width
        assert (
            right.absolute_position.x == canvas.absolute_position.x + canvas.size.width
        )
        away = slint_testing.LogicalPosition(10, left.absolute_position.y - 10)
        window.dispatch_event(slint_testing.PointerMoveEvent(away))
        idle = screenshot(window)
        scale = idle.width / window.root_element.size.width
        y = round((left.absolute_position.y + 10) * scale)
        for label, border_x, interior_x in [
            ("Project pane resize", left.size.width - 1, left.size.width - 2),
            (
                "Inspector pane resize",
                right.absolute_position.x,
                right.absolute_position.x + 1,
            ),
        ]:
            assert idle.getpixel((round(border_x * scale), y)) != idle.getpixel(
                (round(interior_x * scale), y)
            )
            bounds = (
                round((border_x - 3) * scale),
                y,
                round((border_x + 4) * scale),
                y + round(10 * scale),
            )
            idle_edge = idle.crop(bounds).tobytes()
            divider = element(window, label)
            assert divider.absolute_position.y == canvas.absolute_position.y
            assert divider.size.height == canvas.size.height
            if label == "Project pane resize":
                assert (
                    divider.absolute_position.x + divider.size.width
                    <= canvas.absolute_position.x
                )
            else:
                assert (
                    divider.absolute_position.x
                    >= canvas.absolute_position.x + canvas.size.width
                )
            window.dispatch_event(
                slint_testing.PointerMoveEvent(
                    slint_testing.LogicalPosition(center(divider).x, y / scale + 5)
                )
            )
            assert screenshot(window).crop(bounds).tobytes() != idle_edge
            window.dispatch_event(slint_testing.PointerMoveEvent(away))
            assert screenshot(window).crop(bounds).tobytes() == idle_edge

        for x, delta in [
            (canvas.absolute_position.x + 1, 10),
            (canvas.absolute_position.x + canvas.size.width - 1, -10),
        ]:
            artboard = element(window, "Artboard")
            initial_x = artboard.absolute_position.x
            start = slint_testing.LogicalPosition(x, center(canvas).y)
            end = slint_testing.LogicalPosition(x + delta, start.y)
            button = slint_testing.PointerEventButton.Left
            window.dispatch_event(slint_testing.KeyPressedEvent(text=keys.Space))
            window.dispatch_event(slint_testing.PointerMoveEvent(start))
            window.dispatch_event(slint_testing.PointerPressEvent(start, button))
            window.dispatch_event(slint_testing.PointerMoveEvent(end))
            window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
            window.dispatch_event(slint_testing.KeyReleasedEvent(text=keys.Space))
            assert artboard.absolute_position.x == pytest.approx(initial_x + delta)


def test_pane_sizes_persist_across_relaunch(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Main.slint"

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        elements_divider = element(window, "Elements pane resize")
        outline_divider = element(window, "Outline pane resize")
        initial_elements_y = elements_divider.absolute_position.y
        initial_outline_y = outline_divider.absolute_position.y

        drag_divider(window, elements_divider, 72)
        drag_divider(window, outline_divider, -64)

        assert element(window, "FILES").accessible_label == "FILES"
        assert element(window, "ELEMENTS").accessible_label == "ELEMENTS"
        assert element(window, "OUTLINE").accessible_label == "OUTLINE"
        assert elements_divider.absolute_position.y > initial_elements_y
        assert outline_divider.absolute_position.y < initial_outline_y

        saved_elements_y = elements_divider.absolute_position.y
        saved_outline_y = outline_divider.absolute_position.y
        saved_elements_height = int(elements_divider.accessible_value.split()[0])
        saved_outline_height = int(outline_divider.accessible_value.split()[0])

        element(window, "Collapse sidebars").invoke_accessible_default_action()
        element(window, "Expand sidebars").invoke_accessible_default_action()
        elements_divider = element(window, "Elements pane resize")
        outline_divider = element(window, "Outline pane resize")
        assert elements_divider.absolute_position.y == pytest.approx(
            saved_elements_y, abs=1
        )
        assert outline_divider.absolute_position.y == pytest.approx(
            saved_outline_y, abs=1
        )
        wait_for_pane_settings(
            editor_environment,
            {
                "elements_pane_height": saved_elements_height,
                "outline_pane_height": saved_outline_height,
            },
        )

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        elements_divider = element(window, "Elements pane resize")
        outline_divider = element(window, "Outline pane resize")
        assert elements_divider.absolute_position.y == pytest.approx(
            saved_elements_y, abs=1
        )
        assert outline_divider.absolute_position.y == pytest.approx(
            saved_outline_y, abs=1
        )


def test_pane_dividers_are_accessible_and_no_results_is_visible(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        elements = element(window, "Elements pane resize")
        outline = element(window, "Outline pane resize")

        assert elements.accessible_value_minimum == 120
        assert elements.accessible_value_step == 8
        assert outline.accessible_value_minimum == 120
        assert outline.accessible_value_step == 8

        default_elements = float(elements.accessible_value.split()[0])
        default_outline = float(outline.accessible_value.split()[0])
        elements.invoke_accessible_increment_action()
        assert float(elements.accessible_value.split()[0]) == default_elements + 8
        elements.invoke_accessible_decrement_action()
        assert float(elements.accessible_value.split()[0]) == default_elements
        outline.invoke_accessible_decrement_action()
        assert float(outline.accessible_value.split()[0]) == default_outline - 8
        outline.invoke_accessible_increment_action()
        assert float(outline.accessible_value.split()[0]) == default_outline

        drag_divider(window, elements, 1000)
        drag_divider(window, outline, 1000)
        assert (
            float(elements.accessible_value.split()[0])
            == elements.accessible_value_minimum
        )
        assert (
            float(outline.accessible_value.split()[0])
            == outline.accessible_value_minimum
        )

        elements = element(window, "Elements pane resize")
        outline = element(window, "Outline pane resize")
        drag_divider(window, elements, -1000)
        drag_divider(window, outline, -1000)
        assert (
            float(elements.accessible_value.split()[0])
            == elements.accessible_value_maximum
        )
        assert (
            float(outline.accessible_value.split()[0])
            == outline.accessible_value_maximum
        )

        double_click(window, elements)
        assert float(elements.accessible_value.split()[0]) == default_elements

        double_click(window, outline)
        assert float(outline.accessible_value.split()[0]) == default_outline

        search = element(window, "Search elements")
        elements.accessible_value = "120"
        assert float(elements.accessible_value.split()[0]) == 120
        wait_for_pane_settings(editor_environment, {"elements_pane_height": 120})
        search.accessible_value = "missing"
        no_results = element(window, "No Results")
        assert no_results.size.height >= 24
        assert (
            no_results.absolute_position.y
            >= search.absolute_position.y + search.size.height
        )
        assert (
            no_results.absolute_position.y + no_results.size.height
            <= window.size.height
        )
        pane = element(window, "Project and elements")
        assert no_results.absolute_position.x >= pane.absolute_position.x + 12
        assert no_results.absolute_position.x + no_results.size.width <= (
            pane.absolute_position.x + pane.size.width - 12
        )
        double_click(window, elements)

        assert float(elements.accessible_value.split()[0]) == default_elements
        wait_for_pane_settings(editor_environment, {"elements_pane_height": None})

    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        assert (
            float(element(window, "Elements pane resize").accessible_value.split()[0])
            == default_elements
        )
        assert (
            float(element(window, "Outline pane resize").accessible_value.split()[0])
            == default_outline
        )
