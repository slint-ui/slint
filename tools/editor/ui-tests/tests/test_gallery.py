# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore getcolors

import os
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import begin_palette_drag, center
from gradient_interactions import gesture
from inspector_interactions import slider_track_position
from slint_testing import keys
from ui_driver import (
    elements_with_label,
    first_window,
    press_key,
    screenshot,
    select_outline_row,
    wait_until,
    window_element_with_label,
)

PAGES = {
    "foundations": ("Theme and typography", ("Default",)),
    "controls": ("Basic controls", ("Default",)),
    "inspector-controls": ("Inspector controls", ("Default",)),
    "palette": ("Element palette", ("Default", "Unavailable", "Dragging disabled")),
    "picker": (
        "Color and gradients",
        ("Default", "Transparent", "Linear", "Radial", "Conic", "Unsupported"),
    ),
    "outline": (
        "Outline",
        ("Default", "Collapsed", "Long names", "Empty", "Unavailable"),
    ),
}


@pytest.fixture
def gallery_binary(editor_binary: Path) -> Path:
    binary = Path(
        os.environ.get(
            "SLINT_GALLERY_BINARY", str(editor_binary.with_name("slint-editor-gallery"))
        )
    )
    assert binary.is_file(), (
        f"Build slint-editor with --features gallery,system-testing: {binary}"
    )
    return binary


@contextmanager
def gallery(
    binary: Path,
    environment: dict[str, str],
    page: str,
    scenario: str = "Default",
    theme: str = "light",
    width: int = 1440,
    height: int = 1000,
    scale: int = 1,
) -> Iterator[slint_testing.Window]:
    with slint_testing.Application(
        [
            str(binary),
            "--width",
            str(width),
            "--height",
            str(height),
            "--scale-factor",
            str(scale),
        ],
        env=environment | {"SLINT_BACKEND": "headless-skia"},
        launch_timeout=30,
    ) as application:
        window = first_window(application)
        preview = window_element_with_label(window, "Gallery preview")
        wait_until(
            lambda: (
                preview if preview.size.width > 0 and preview.size.height > 0 else None
            )
        )
        window_element_with_label(
            window, PAGES[page][0], slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        for label, index in [
            ("Gallery theme", ("system", "light", "dark").index(theme)),
            ("Gallery state", PAGES[page][1].index(scenario)),
        ]:
            if label == "Gallery state" and len(PAGES[page][1]) == 1:
                continue
            combo = window_element_with_label(
                window, label, slint_testing.AccessibleRole.Combobox
            )
            combo.invoke_accessible_expand_action()
            for _ in range(index):
                press_key(window, keys.DownArrow)
            press_key(window, keys.Return)
            expected = (
                ("System", "Light", "Dark")[index]
                if label == "Gallery theme"
                else scenario
            )
            wait_until(
                lambda combo=combo, expected=expected: (
                    True if combo.accessible_value == expected else None
                )
            )
        yield window


@pytest.mark.parametrize("page", PAGES)
@pytest.mark.parametrize("theme", ["light", "dark"])
def test_gallery_scenarios_render(
    gallery_binary, editor_environment, tmp_path, page, theme
):
    scenarios = PAGES[page][1]
    destination = Path(os.environ.get("SLINT_GALLERY_SCREENSHOT_DIR", str(tmp_path)))
    destination.mkdir(parents=True, exist_ok=True)
    for scenario in scenarios:
        with gallery(
            gallery_binary, editor_environment, page, scenario, theme
        ) as window:
            if page == "picker":
                window_element_with_label(
                    window, "Open color picker"
                ).invoke_accessible_default_action()
                window_element_with_label(window, "Close Custom")
            expected = {
                "foundations": "SEMANTIC COLORS",
                "controls": "Sample text input",
                "inspector-controls": "Sample slider",
                "palette": "Sample drop target",
                "picker": "Sample fill",
                "outline": "OUTLINE",
            }[page]
            window_element_with_label(window, expected)
            sidebar = window_element_with_label(window, "Gallery properties")
            assert (
                sidebar.absolute_position.x
                > window_element_with_label(
                    window, "Gallery preview"
                ).absolute_position.x
            )
            assert sidebar.absolute_position.x + sidebar.size.width <= 1440
            assert (
                window_element_with_label(window, "Gallery preview").absolute_position.y
                < 200
            )
            image = screenshot(window)
            assert image.width >= 800 and image.height >= 600
            colors = image.resize((80, 60)).getcolors(4801)
            assert colors is not None and len(colors) > 10
            image.save(
                destination / f"{page}-{scenario.lower().replace(' ', '-')}-{theme}.png"
            )


def test_gallery_palette_drop_and_reset(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "palette") as window:
        target = center(window_element_with_label(window, "Sample drop target"))
        begin_palette_drag(window, "Rectangle", target)
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                target, slint_testing.PointerEventButton.Left
            )
        )
        window_element_with_label(window, "Dropped Rectangle")
        window_element_with_label(
            window, "Reset example"
        ).invoke_accessible_default_action()
        window_element_with_label(window, "Ready")
        assert not elements_with_label(window.root_element, "Rectangle drag preview")


def test_gallery_outline_selection_expansion_and_drop(
    gallery_binary, editor_environment
):
    with gallery(gallery_binary, editor_environment, "outline") as window:
        title = select_outline_row(window, "title")
        assert title.accessible_item_selected
        card = window_element_with_label(
            window, "card", slint_testing.AccessibleRole.ListItem
        )
        window.drag_and_drop(center(title), center(card))
        window_element_with_label(window, "Dropped Onto row 1")
        main = window_element_with_label(
            window, "Main", slint_testing.AccessibleRole.ListItem
        )
        main.invoke_accessible_expand_action()
        wait_until(
            lambda: (
                True if not elements_with_label(window.root_element, "title") else None
            )
        )
        window_element_with_label(
            window, "Main", slint_testing.AccessibleRole.ListItem
        ).invoke_accessible_expand_action()
        window_element_with_label(
            window, "title", slint_testing.AccessibleRole.ListItem
        )
        window_element_with_label(
            window, "Reset example"
        ).invoke_accessible_default_action()
        window_element_with_label(window, "Ready")


def test_gallery_picker_cancel_and_commit(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "picker") as window:
        window_element_with_label(
            window, "Property fill color", slint_testing.AccessibleRole.TextInput
        ).accessible_value = "#ff9900"

        def open_picker():
            window_element_with_label(
                window, "Open color picker"
            ).invoke_accessible_default_action()
            field = window_element_with_label(
                window, "Hex color", slint_testing.AccessibleRole.TextInput
            )
            gesture(window, center(field), center(field))
            return field

        original = open_picker().accessible_value
        assert original.lower().lstrip("#") == "ff9900"
        window_element_with_label(
            window, "Hex color", slint_testing.AccessibleRole.TextInput
        ).accessible_value = "#ff0000"
        press_key(window, keys.Escape)
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "Close Custom")
                else None
            )
        )
        assert open_picker().accessible_value == original
        window_element_with_label(
            window, "Hex color", slint_testing.AccessibleRole.TextInput
        ).accessible_value = "#00ff00"
        window_element_with_label(
            window, "Close Custom"
        ).invoke_accessible_default_action()
        assert open_picker().accessible_value.lower().lstrip("#") == "00ff00"


@pytest.mark.parametrize("width,height", [(1024, 768), (1440, 1000)])
@pytest.mark.parametrize("scale", [1, 2])
def test_gallery_picker_bounds_at_window_sizes(
    gallery_binary, editor_environment, tmp_path, width, height, scale
):
    with gallery(
        gallery_binary,
        editor_environment,
        "picker",
        "Linear",
        width=width,
        height=height,
        scale=scale,
    ) as window:
        window_element_with_label(
            window, "Open color picker"
        ).invoke_accessible_default_action()
        close = window_element_with_label(window, "Close Custom")
        assert close.absolute_position.x >= 0
        assert close.absolute_position.x + close.size.width <= width
        assert close.absolute_position.y + close.size.height <= height
        image = screenshot(window)
        assert image.size == (width * scale, height * scale)
        image.save(tmp_path / f"picker-{width}-{height}-{scale}.png")


@pytest.mark.parametrize("label", ["Sample slider", "All corner radii slider"])
def test_gallery_slider_drag_cancel_and_reset(
    gallery_binary, editor_environment, label
):
    with gallery(gallery_binary, editor_environment, "inspector-controls") as window:
        slider = window_element_with_label(
            window, label, slint_testing.AccessibleRole.Slider
        )
        original = float(slider.accessible_value)
        target = slider_track_position(slider, 0.75)
        gesture(window, target, target)
        wait_until(lambda: True if float(slider.accessible_value) == 75 else None)
        other = window_element_with_label(
            window, "Rotation knob", slint_testing.AccessibleRole.Slider
        )
        assert float(other.accessible_value) == 24
        press_key(window, keys.RightArrow)
        assert float(slider.accessible_value) == 76
        cancel_target = slider_track_position(slider, 0.4)
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                cancel_target, slint_testing.PointerEventButton.Left
            )
        )
        wait_until(lambda: True if float(slider.accessible_value) == 40 else None)
        press_key(window, keys.Escape)
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                cancel_target, slint_testing.PointerEventButton.Left
            )
        )
        assert float(slider.accessible_value) == 76
        window_element_with_label(
            window, "Reset example"
        ).invoke_accessible_default_action()
        wait_until(
            lambda: (
                True
                if float(
                    window_element_with_label(
                        window, label, slint_testing.AccessibleRole.Slider
                    ).accessible_value
                )
                == original
                else None
            )
        )


def test_gallery_basic_controls_pointer_targets(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "controls") as window:
        previous_right = 0
        for label in ["One", "Two", "Three"]:
            button = window_element_with_label(
                window, label, slint_testing.AccessibleRole.Button
            )
            assert button.size.width >= 40
            assert button.absolute_position.x >= previous_right
            gesture(window, center(button), center(button))
            assert button.accessible_checked
            previous_right = button.absolute_position.x + button.size.width
        visibility = window_element_with_label(
            window, "Visibility", slint_testing.AccessibleRole.Button
        )
        gesture(window, center(visibility), center(visibility))
        window_element_with_label(window, "Clicked visibility")


def test_gallery_properties_edit_component_values(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "inspector-controls") as window:
        value = window_element_with_label(
            window, "Slider value", slint_testing.AccessibleRole.TextInput
        )
        value.accessible_value = "42"
        slider = window_element_with_label(
            window, "Sample slider", slint_testing.AccessibleRole.Slider
        )
        wait_until(lambda: True if float(slider.accessible_value) == 42 else None)
        target = slider_track_position(slider, 0.6)
        gesture(window, target, target)
        wait_until(lambda: True if float(value.accessible_value) == 60 else None)
        assert (
            window_element_with_label(
                window, "Sample numeric field", slint_testing.AccessibleRole.TextInput
            ).accessible_value
            == "24"
        )
        text = window_element_with_label(
            window, "Property sample text", slint_testing.AccessibleRole.TextInput
        )
        text.accessible_value = "From the sidebar"
        wait_until(
            lambda: (
                True
                if window_element_with_label(
                    window,
                    "Sample editable field",
                    slint_testing.AccessibleRole.TextInput,
                ).accessible_value
                == "From the sidebar"
                else None
            )
        )
        window_element_with_label(
            window, "Reset example"
        ).invoke_accessible_default_action()
        wait_until(lambda: True if float(slider.accessible_value) == 24 else None)
        assert (
            window_element_with_label(
                window, "Sample editable field", slint_testing.AccessibleRole.TextInput
            ).accessible_value
            == "Hello Slint"
        )


def test_gallery_properties_resize_preview(gallery_binary, editor_environment):
    with gallery(
        gallery_binary, editor_environment, "foundations", width=1024, height=768
    ) as window:
        preview = window_element_with_label(window, "Gallery preview")
        for label, value in [("Preview width", "680"), ("Preview height", "400")]:
            window_element_with_label(
                window, label, slint_testing.AccessibleRole.TextInput
            ).accessible_value = value
        wait_until(
            lambda: (
                True
                if preview.size.width == 680 and preview.size.height == 400
                else None
            )
        )
        sidebar = window_element_with_label(window, "Gallery properties")
        assert sidebar.absolute_position.x + sidebar.size.width <= 1024
        assert preview.absolute_position.y < 200
        window_element_with_label(
            window, "Reset example"
        ).invoke_accessible_default_action()
        wait_until(
            lambda: (
                True if preview.size.width < 680 and preview.size.height > 400 else None
            )
        )
