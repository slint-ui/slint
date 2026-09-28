# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore getcolors

import os
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from typing import cast

import pytest
import slint_testing
from canvas_interactions import center
from gradient_interactions import gesture
from inspector_interactions import slider_track_position
from slint_test import Window, expect
from slint_testing import keys
from ui_driver import (
    first_window,
    screenshot,
    select_outline_row,
    wait_until,
)

PAGES = {
    "foundations": ("Theme and typography", ("Default",)),
    "controls": ("Basic controls", ("Default",)),
    "inspector-controls": ("Inspector controls", ("Default",)),
    "palette": ("Element palette", ("Default", "Unavailable")),
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
            "SLINT_GALLERY_BINARY", str(editor_binary.parent / "examples" / "gallery")
        )
    )
    assert binary.is_file(), (
        f"Build with cargo build -p slint-editor --example gallery --features system-testing: {binary}"
    )
    return binary


@contextmanager
def gallery(
    binary: Path,
    environment: dict[str, str],
    page: str,
    scenario: str = "Default",
    theme: str = "light",
) -> Iterator[Window]:
    with slint_testing.Application(
        [str(binary)],
        env=environment | {"SLINT_BACKEND": "headless-skia"},
        launch_timeout=30,
    ) as application:
        window = first_window(application)
        preview = window.get_by_accessible_name("Gallery preview").resolve()
        wait_until(
            lambda: (
                preview if preview.size.width > 0 and preview.size.height > 0 else None
            )
        )
        window.get_by_role(
            slint_testing.AccessibleRole.Button, name=PAGES[page][0]
        ).activate()
        for label, index in [
            ("Gallery theme", ("system", "light", "dark").index(theme)),
            ("Gallery state", PAGES[page][1].index(scenario)),
        ]:
            if label == "Gallery state" and len(PAGES[page][1]) == 1:
                continue
            combo = window.get_by_role(
                slint_testing.AccessibleRole.Combobox, name=label
            ).resolve()
            combo.invoke_accessible_expand_action()
            for _ in range(index):
                window.keyboard.press(keys.DownArrow)
            window.keyboard.press(keys.Return)
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
                window.get_by_accessible_name("Open color picker").activate()
                window.get_by_accessible_name("Close Custom").resolve()
            expected = {
                "foundations": "SEMANTIC COLORS",
                "controls": "Sample text input",
                "inspector-controls": "Sample slider",
                "palette": "ELEMENTS",
                "picker": "Sample fill",
                "outline": "OUTLINE",
            }[page]
            window.get_by_accessible_name(expected).resolve()
            sidebar = window.get_by_accessible_name("Gallery properties").resolve()
            assert (
                sidebar.absolute_position.x
                > window.get_by_accessible_name("Gallery preview")
                .resolve()
                .absolute_position.x
            )
            assert sidebar.absolute_position.x + sidebar.size.width <= 1440
            assert (
                window.get_by_accessible_name("Gallery preview")
                .resolve()
                .absolute_position.y
                < 200
            )
            image = screenshot(window)
            assert image.width >= 800 and image.height >= 600
            colors = image.resize((80, 60)).getcolors(4801)
            assert colors is not None and len(colors) > 10
            image.save(
                destination / f"{page}-{scenario.lower().replace(' ', '-')}-{theme}.png"
            )


@pytest.mark.parametrize("theme", ["light", "dark"])
def test_gallery_gradient_stop_marker_contrast(
    gallery_binary, editor_environment, theme
):
    with gallery(
        gallery_binary, editor_environment, "picker", "Linear", theme
    ) as window:
        window.get_by_accessible_name("Open color picker").activate()
        marker = window.get_by_role(
            slint_testing.AccessibleRole.Slider, name="Gradient stop 1"
        ).resolve()
        assert marker.size.width == 40
        assert marker.size.height == 40

        image = screenshot(window)
        if destination := os.environ.get("SLINT_GALLERY_SCREENSHOT_DIR"):
            output = Path(destination)
            output.mkdir(parents=True, exist_ok=True)
            image.save(output / f"gradient-stop-marker-{theme}.png")
        x = round(marker.absolute_position.x)
        y = round(marker.absolute_position.y)
        tick = cast(tuple[int, int, int], image.getpixel((x + 4, y + 23)))
        tick_keyline = cast(tuple[int, int, int], image.getpixel((x + 1, y + 23)))
        inner_ring = cast(tuple[int, int, int], image.getpixel((x + 20, y + 10)))
        pointer = cast(tuple[int, int, int], image.getpixel((x + 20, y + 4)))

        assert max(tick) < 48
        assert min(tick_keyline) > 224
        assert min(inner_ring) > 224
        assert min(pointer) > 224


def test_gallery_outline_selection_expansion_and_reset(
    gallery_binary, editor_environment
):
    with gallery(gallery_binary, editor_environment, "outline") as window:
        title = select_outline_row(window, "title")
        expect(title).to_be_selected()
        main = window.get_by_role(
            slint_testing.AccessibleRole.ListItem, name="Main"
        ).resolve()
        main.invoke_accessible_expand_action()
        expect(window.get_by_accessible_name("title")).to_be_hidden()
        window.get_by_role(
            slint_testing.AccessibleRole.ListItem, name="Main"
        ).resolve().invoke_accessible_expand_action()
        window.get_by_role(
            slint_testing.AccessibleRole.ListItem, name="title"
        ).resolve()
        window.get_by_accessible_name("Reset example").activate()
        expect(
            window.get_by_role(slint_testing.AccessibleRole.ListItem, name="Main")
        ).to_be_selected()


def test_gallery_picker_cancel_and_commit(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "picker") as window:
        window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Property fill color"
        ).set_accessible_value("#ff9900")

        def open_picker():
            window.get_by_accessible_name("Open color picker").activate()
            field = window.get_by_role(
                slint_testing.AccessibleRole.TextInput, name="Hex color"
            )
            gesture(window, field.center(), field.center())
            return field

        original = open_picker().value()
        assert original.lower().lstrip("#") == "ff9900"
        window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Hex color"
        ).set_accessible_value("#ff0000")
        window.keyboard.press(keys.Escape)
        expect(window.get_by_accessible_name("Close Custom")).to_be_hidden()
        expect(open_picker()).to_have_value(original)
        window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Hex color"
        ).set_accessible_value("#00ff00")
        window.get_by_accessible_name("Close Custom").activate()
        assert open_picker().value().lower().lstrip("#") == "00ff00"


@pytest.mark.parametrize("label", ["Sample slider", "All corner radii slider"])
def test_gallery_slider_drag_cancel_and_reset(
    gallery_binary, editor_environment, label
):
    with gallery(gallery_binary, editor_environment, "inspector-controls") as window:
        slider = window.get_by_role(
            slint_testing.AccessibleRole.Slider, name=label
        ).resolve()
        original = float(slider.accessible_value)
        target = slider_track_position(slider, 0.75)
        gesture(window, target, target)
        wait_until(lambda: True if float(slider.accessible_value) == 75 else None)
        other = window.get_by_role(
            slint_testing.AccessibleRole.Slider, name="Rotation knob"
        ).resolve()
        assert float(other.accessible_value) == 24
        window.keyboard.press(keys.RightArrow)
        assert float(slider.accessible_value) == 76
        cancel_target = slider_track_position(slider, 0.4)
        window.pointer.press_at(cancel_target)
        wait_until(lambda: True if float(slider.accessible_value) == 40 else None)
        window.keyboard.press(keys.Escape)
        window.pointer.release_at(cancel_target)
        assert float(slider.accessible_value) == 76
        window.get_by_accessible_name("Reset example").activate()
        wait_until(
            lambda: (
                True
                if float(
                    window.get_by_role(slint_testing.AccessibleRole.Slider, name=label)
                    .resolve()
                    .accessible_value
                )
                == original
                else None
            )
        )


def test_gallery_shadow_angle_drag(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "inspector-controls") as window:
        dial = window.get_by_role(
            slint_testing.AccessibleRole.Slider, name="Sample shadow angle"
        ).resolve()
        assert float(dial.accessible_value) == 90
        middle = center(dial)
        radius = dial.size.width / 3
        start = slint_testing.LogicalPosition(middle.x, middle.y + radius)
        end = slint_testing.LogicalPosition(middle.x + radius, middle.y)
        gesture(window, start, end)
        wait_until(lambda: True if float(dial.accessible_value) == 0 else None)


def test_gallery_basic_controls_pointer_targets(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "controls") as window:
        previous_right = 0
        for label in ["One", "Two", "Three"]:
            button = window.get_by_role(
                slint_testing.AccessibleRole.Button, name=label
            ).resolve()
            assert button.size.width >= 40
            assert button.absolute_position.x >= previous_right
            gesture(window, center(button), center(button))
            assert button.accessible_checked
            previous_right = button.absolute_position.x + button.size.width
        visibility = window.get_by_role(
            slint_testing.AccessibleRole.Button, name="Visibility"
        ).resolve()
        gesture(window, center(visibility), center(visibility))
        window.get_by_accessible_name("Clicked visibility").resolve()


def test_gallery_image_alignment_selection_and_reset(
    gallery_binary, editor_environment
):
    with gallery(gallery_binary, editor_environment, "inspector-controls") as window:
        center = window.get_by_role(
            slint_testing.AccessibleRole.Button, name="Align image center"
        ).resolve()
        assert center.accessible_checked
        window.get_by_role(
            slint_testing.AccessibleRole.Button, name="Align image bottom right"
        ).activate()
        assert (
            window.get_by_role(
                slint_testing.AccessibleRole.Button, name="Align image bottom right"
            )
            .resolve()
            .accessible_checked
        )
        window.get_by_role(
            slint_testing.AccessibleRole.Text, name="Bottom right"
        ).resolve()
        window.get_by_accessible_name("Reset example").activate()
        assert (
            window.get_by_role(
                slint_testing.AccessibleRole.Button, name="Align image center"
            )
            .resolve()
            .accessible_checked
        )


def test_gallery_properties_edit_component_values(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "inspector-controls") as window:
        value = window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Slider value"
        )
        value.set_accessible_value("42")
        slider = window.get_by_role(
            slint_testing.AccessibleRole.Slider, name="Sample slider"
        ).resolve()
        wait_until(lambda: True if float(slider.accessible_value) == 42 else None)
        target = slider_track_position(slider, 0.6)
        gesture(window, target, target)
        expect(value).to_have_value("60")
        assert (
            window.get_by_role(
                slint_testing.AccessibleRole.TextInput, name="Sample numeric field"
            )
            .resolve()
            .accessible_value
            == "24"
        )
        text = window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Property sample text"
        )
        text.set_accessible_value("From the sidebar")
        sample = window.get_by_role(
            slint_testing.AccessibleRole.TextInput, name="Sample editable field"
        )
        expect(sample).to_have_value("From the sidebar")
        window.get_by_accessible_name("Reset example").activate()
        wait_until(lambda: True if float(slider.accessible_value) == 24 else None)
        expect(sample).to_have_value("Hello Slint")


def test_gallery_properties_resize_preview(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "foundations") as window:
        preview = window.get_by_accessible_name("Gallery preview")
        original_bounds = preview.bounds()
        for label, value in [("Preview width", "680"), ("Preview height", "400")]:
            window.get_by_role(
                slint_testing.AccessibleRole.TextInput, name=label
            ).set_accessible_value(value)
        expect(preview).to_have_geometry(width=680, height=400)
        sidebar = window.get_by_accessible_name("Gallery properties").resolve()
        assert (
            sidebar.absolute_position.x + sidebar.size.width
            <= window.root_element.size.width
        )
        assert preview.bounds().y < 200
        window.get_by_accessible_name("Reset example").activate()
        expect(preview).to_have_geometry(
            width=original_bounds.width, height=original_bounds.height
        )
