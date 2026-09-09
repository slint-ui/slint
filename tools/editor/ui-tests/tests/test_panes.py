# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from editor_sync import current_editor_sync
from ui_driver import first_window, launch_editor, wait_until, window_element_with_label


def wait_for_pane_settings(directory: Path, expected: dict[str, int | None]) -> None:
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


def drag_vertically(
    window: slint_testing.Window, element: slint_testing.Element, delta: float
) -> None:
    start = center(element)
    end = slint_testing.LogicalPosition(x=start.x, y=start.y + delta)
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


def test_pane_sizes_persist_across_relaunch(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tmp_path: Path,
) -> None:
    source_file = fixture_project / "Main.slint"

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        elements_divider = window_element_with_label(window, "Elements pane resize")
        outline_divider = window_element_with_label(window, "Outline pane resize")
        initial_elements_y = elements_divider.absolute_position.y
        initial_outline_y = outline_divider.absolute_position.y

        with current_editor_sync.get().action() as resize:
            drag_vertically(window, elements_divider, 72)
            drag_vertically(window, outline_divider, -64)
        resize.wait_for_settled(outcome="completed")

        assert window_element_with_label(window, "FILES").accessible_label == "FILES"
        assert (
            window_element_with_label(window, "ELEMENTS").accessible_label == "ELEMENTS"
        )
        assert (
            window_element_with_label(window, "APPEARANCE").accessible_label
            == "APPEARANCE"
        )
        assert (
            window_element_with_label(window, "OUTLINE").accessible_label == "OUTLINE"
        )
        assert elements_divider.absolute_position.y > initial_elements_y
        assert outline_divider.absolute_position.y < initial_outline_y

        saved_elements_y = elements_divider.absolute_position.y
        saved_outline_y = outline_divider.absolute_position.y
        saved_elements_height = int(elements_divider.accessible_value.split()[0])
        saved_outline_height = int(outline_divider.accessible_value.split()[0])

        window_element_with_label(
            window, "Collapse sidebars"
        ).invoke_accessible_default_action()
        window_element_with_label(
            window, "Expand sidebars"
        ).invoke_accessible_default_action()
        elements_divider = window_element_with_label(window, "Elements pane resize")
        outline_divider = window_element_with_label(window, "Outline pane resize")
        assert elements_divider.absolute_position.y == pytest.approx(
            saved_elements_y, abs=1
        )
        assert outline_divider.absolute_position.y == pytest.approx(
            saved_outline_y, abs=1
        )
        wait_for_pane_settings(
            tmp_path,
            {
                "elements_pane_height": saved_elements_height,
                "outline_pane_height": saved_outline_height,
            },
        )

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        elements_divider = window_element_with_label(window, "Elements pane resize")
        outline_divider = window_element_with_label(window, "Outline pane resize")
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
    tmp_path: Path,
) -> None:
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        elements = window_element_with_label(window, "Elements pane resize")
        outline = window_element_with_label(window, "Outline pane resize")

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

        drag_vertically(window, elements, 1000)
        drag_vertically(window, outline, 1000)
        assert (
            float(elements.accessible_value.split()[0])
            == elements.accessible_value_minimum
        )
        assert (
            float(outline.accessible_value.split()[0])
            == outline.accessible_value_minimum
        )

        elements = window_element_with_label(window, "Elements pane resize")
        outline = window_element_with_label(window, "Outline pane resize")
        drag_vertically(window, elements, -1000)
        drag_vertically(window, outline, -1000)
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

        search = window_element_with_label(window, "Search elements")
        elements.accessible_value = "120"
        assert float(elements.accessible_value.split()[0]) == 120
        wait_for_pane_settings(tmp_path, {"elements_pane_height": 120})
        search.accessible_value = "missing"
        no_results = window_element_with_label(window, "No Results")
        assert no_results.size.height >= 24
        assert (
            no_results.absolute_position.y
            >= search.absolute_position.y + search.size.height
        )
        assert (
            no_results.absolute_position.y + no_results.size.height
            <= window.size.height
        )
        pane = window_element_with_label(window, "Project and elements")
        assert no_results.absolute_position.x + no_results.size.width <= (
            pane.absolute_position.x + pane.size.width - 14
        )
        with current_editor_sync.get().action() as reset:
            double_click(window, elements)
        reset.wait_for_settled(outcome="completed")
        assert float(elements.accessible_value.split()[0]) == default_elements
        wait_for_pane_settings(tmp_path, {"elements_pane_height": None})

    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        assert (
            float(
                window_element_with_label(
                    window, "Elements pane resize"
                ).accessible_value.split()[0]
            )
            == default_elements
        )
        assert (
            float(
                window_element_with_label(
                    window, "Outline pane resize"
                ).accessible_value.split()[0]
            )
            == default_outline
        )
