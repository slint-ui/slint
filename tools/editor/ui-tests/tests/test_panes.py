# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import json
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center
from slint_test import Window
from ui_driver import first_window, launch_editor, wait_until


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
    window: Window, element: slint_testing.Element, delta: float
) -> None:
    start = center(element)
    end = slint_testing.LogicalPosition(x=start.x, y=start.y + delta)
    window.pointer.press_at(start)
    window.pointer.move_to(end)
    window.pointer.release_at(end)


def double_click(window: Window, element: slint_testing.Element) -> None:
    position = center(element)
    for _ in range(2):
        window.pointer.press_at(position)
        window.pointer.release_at(position)


def test_pane_sizes_persist_across_relaunch(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tmp_path: Path,
) -> None:
    editor_environment["HOME"] = str(tmp_path / "home")
    editor_environment["XDG_CONFIG_HOME"] = str(tmp_path / "config")
    source_file = fixture_project / "Main.slint"

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        elements_divider = window.get_by_accessible_name(
            "Elements pane resize"
        ).resolve()
        outline_divider = window.get_by_accessible_name("Outline pane resize").resolve()
        initial_elements_y = elements_divider.absolute_position.y
        initial_outline_y = outline_divider.absolute_position.y

        drag_vertically(window, elements_divider, 72)
        drag_vertically(window, outline_divider, -64)

        assert (
            window.get_by_accessible_name("FILES").resolve().accessible_label == "FILES"
        )
        assert (
            window.get_by_accessible_name("ELEMENTS").resolve().accessible_label
            == "ELEMENTS"
        )
        assert (
            window.get_by_accessible_name("OUTLINE").resolve().accessible_label
            == "OUTLINE"
        )
        assert elements_divider.absolute_position.y > initial_elements_y
        assert outline_divider.absolute_position.y < initial_outline_y

        saved_elements_y = elements_divider.absolute_position.y
        saved_outline_y = outline_divider.absolute_position.y
        saved_elements_height = int(elements_divider.accessible_value.split()[0])
        saved_outline_height = int(outline_divider.accessible_value.split()[0])

        window.get_by_accessible_name("Collapse sidebars").activate()
        window.get_by_accessible_name("Expand sidebars").activate()
        elements_divider = window.get_by_accessible_name(
            "Elements pane resize"
        ).resolve()
        outline_divider = window.get_by_accessible_name("Outline pane resize").resolve()
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
        elements_divider = window.get_by_accessible_name(
            "Elements pane resize"
        ).resolve()
        outline_divider = window.get_by_accessible_name("Outline pane resize").resolve()
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
    editor_environment["HOME"] = str(tmp_path / "home")
    editor_environment["XDG_CONFIG_HOME"] = str(tmp_path / "config")
    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        elements = window.get_by_accessible_name("Elements pane resize").resolve()
        outline = window.get_by_accessible_name("Outline pane resize").resolve()

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

        elements = window.get_by_accessible_name("Elements pane resize").resolve()
        outline = window.get_by_accessible_name("Outline pane resize").resolve()
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

        search = window.get_by_accessible_name("Search elements").resolve()
        elements.accessible_value = "120"
        assert float(elements.accessible_value.split()[0]) == 120
        wait_for_pane_settings(tmp_path, {"elements_pane_height": 120})
        search.accessible_value = "missing"
        no_results = window.get_by_accessible_name("No Results").resolve()
        assert no_results.size.height >= 24
        assert (
            no_results.absolute_position.y
            >= search.absolute_position.y + search.size.height
        )
        assert (
            no_results.absolute_position.y + no_results.size.height
            <= window.size.height
        )
        pane = window.get_by_accessible_name("Project and elements").resolve()
        assert no_results.absolute_position.x >= pane.absolute_position.x + 12
        assert no_results.absolute_position.x + no_results.size.width <= (
            pane.absolute_position.x + pane.size.width - 12
        )
        double_click(window, elements)

        assert float(elements.accessible_value.split()[0]) == default_elements
        wait_for_pane_settings(tmp_path, {"elements_pane_height": None})

    with launch_editor(
        editor_binary, editor_environment, fixture_project / "Main.slint"
    ) as editor:
        window = first_window(editor)
        assert (
            float(
                window.get_by_accessible_name("Elements pane resize")
                .resolve()
                .accessible_value.split()[0]
            )
            == default_elements
        )
        assert (
            float(
                window.get_by_accessible_name("Outline pane resize")
                .resolve()
                .accessible_value.split()[0]
            )
            == default_outline
        )
