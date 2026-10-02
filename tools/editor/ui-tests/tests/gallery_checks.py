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
from slint_testing import keys
from ui_assertions import expect
from ui_driver import (
    element,
    first_window,
    press_key,
    query,
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
) -> Iterator[slint_testing.Window]:
    with slint_testing.Application(
        [str(binary)],
        env=environment | {"SLINT_BACKEND": "headless-skia"},
        launch_timeout=30,
    ) as application:
        window = first_window(application)
        preview = element(window, "Gallery preview")
        wait_until(
            lambda: (
                preview if preview.size.width > 0 and preview.size.height > 0 else None
            )
        )
        element(
            window, PAGES[page][0], role=slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        for label, index in [
            ("Gallery theme", ("system", "light", "dark").index(theme)),
            ("Gallery state", PAGES[page][1].index(scenario)),
        ]:
            if label == "Gallery state" and len(PAGES[page][1]) == 1:
                continue
            combo = element(window, label, role=slint_testing.AccessibleRole.Combobox)
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
                    combo.accessible_value == expected
                )
            )
        yield window


@pytest.mark.parametrize("theme", ["light", "dark"])
def test_gallery_gradient_stop_marker_contrast(
    gallery_binary, editor_environment, theme
):
    with gallery(
        gallery_binary, editor_environment, "picker", "Linear", theme
    ) as window:
        element(window, "Open color picker").invoke_accessible_default_action()
        marker = element(
            window, "Gradient stop 1", role=slint_testing.AccessibleRole.Slider
        )
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
        main = element(window, "Main", role=slint_testing.AccessibleRole.ListItem)
        main.invoke_accessible_expand_action()
        expect(query(window, "title")).to_be_hidden()
        element(
            window, "Main", role=slint_testing.AccessibleRole.ListItem
        ).invoke_accessible_expand_action()
        element(window, "title", role=slint_testing.AccessibleRole.ListItem)
        element(window, "Reset example").invoke_accessible_default_action()
        expect(
            query(window, "Main", role=slint_testing.AccessibleRole.ListItem)
        ).to_be_selected()


def test_gallery_picker_cancel_and_commit(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "picker") as window:
        element(
            window, "Property fill color", role=slint_testing.AccessibleRole.TextInput
        ).accessible_value = "#ff9900"

        def open_picker():
            element(window, "Open color picker").invoke_accessible_default_action()
            field = element(
                window, "Hex color", role=slint_testing.AccessibleRole.TextInput
            )
            gesture(window, center(field), center(field))
            return field

        original = open_picker().accessible_value
        assert original.lower().lstrip("#") == "ff9900"
        element(
            window, "Hex color", role=slint_testing.AccessibleRole.TextInput
        ).accessible_value = "#ff0000"
        press_key(window, keys.Escape)
        expect(query(window, "Close Custom")).to_be_hidden()
        expect(open_picker()).to_have_value(original)
        element(
            window, "Hex color", role=slint_testing.AccessibleRole.TextInput
        ).accessible_value = "#00ff00"
        element(window, "Close Custom").invoke_accessible_default_action()
        assert open_picker().accessible_value.lower().lstrip("#") == "00ff00"


@pytest.mark.parametrize("label", ["Sample slider", "All corner radii slider"])
def test_gallery_slider_drag_cancel_and_reset(
    gallery_binary, editor_environment, label
):
    with gallery(gallery_binary, editor_environment, "inspector-controls") as window:
        slider = element(window, label, role=slint_testing.AccessibleRole.Slider)
        original = float(slider.accessible_value)
        target = slider_track_position(slider, 0.75)
        gesture(window, target, target)
        expect.poll(
            lambda: float(slider.accessible_value), message=f"{label} value"
        ).to_equal(75)
        other = element(
            window, "Rotation knob", role=slint_testing.AccessibleRole.Slider
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
        expect.poll(
            lambda: float(slider.accessible_value), message=f"{label} drag value"
        ).to_equal(40)
        press_key(window, keys.Escape)
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                cancel_target, slint_testing.PointerEventButton.Left
            )
        )
        assert float(slider.accessible_value) == 76
        element(window, "Reset example").invoke_accessible_default_action()
        expect.poll(
            lambda: float(
                element(
                    window, label, role=slint_testing.AccessibleRole.Slider
                ).accessible_value
            ),
            message=f"reset {label} value",
        ).to_equal(original)


def test_gallery_shadow_angle_drag(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "inspector-controls") as window:
        dial = element(
            window, "Sample shadow angle", role=slint_testing.AccessibleRole.Slider
        )
        assert float(dial.accessible_value) == 90
        middle = center(dial)
        radius = dial.size.width / 3
        start = slint_testing.LogicalPosition(middle.x, middle.y + radius)
        end = slint_testing.LogicalPosition(middle.x + radius, middle.y)
        gesture(window, start, end)
        expect.poll(
            lambda: float(dial.accessible_value), message="shadow angle"
        ).to_equal(0)


def test_gallery_basic_controls_pointer_targets(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "controls") as window:
        previous_right = 0
        for label in ["One", "Two", "Three"]:
            button = element(window, label, role=slint_testing.AccessibleRole.Button)
            assert button.size.width >= 40
            assert button.absolute_position.x >= previous_right
            gesture(window, center(button), center(button))
            assert button.accessible_checked
            previous_right = button.absolute_position.x + button.size.width
        visibility = element(
            window, "Visibility", role=slint_testing.AccessibleRole.Button
        )
        gesture(window, center(visibility), center(visibility))
        element(window, "Clicked visibility")


def test_gallery_image_alignment_selection_and_reset(
    gallery_binary, editor_environment
):
    with gallery(gallery_binary, editor_environment, "inspector-controls") as window:
        center = element(
            window, "Align image center", role=slint_testing.AccessibleRole.Button
        )
        assert center.accessible_checked
        element(
            window, "Align image bottom right", role=slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        assert element(
            window, "Align image bottom right", role=slint_testing.AccessibleRole.Button
        ).accessible_checked
        element(window, "Bottom right", role=slint_testing.AccessibleRole.Text)
        element(window, "Reset example").invoke_accessible_default_action()
        assert element(
            window, "Align image center", role=slint_testing.AccessibleRole.Button
        ).accessible_checked


def test_gallery_properties_edit_component_values(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "inspector-controls") as window:
        value = element(
            window, "Slider value", role=slint_testing.AccessibleRole.TextInput
        )
        value.accessible_value = "42"
        slider = element(
            window, "Sample slider", role=slint_testing.AccessibleRole.Slider
        )
        wait_until(lambda: float(slider.accessible_value) == 42)
        target = slider_track_position(slider, 0.6)
        gesture(window, target, target)
        expect(value).to_have_value("60")
        assert (
            element(
                window,
                "Sample numeric field",
                role=slint_testing.AccessibleRole.TextInput,
            ).accessible_value
            == "24"
        )
        text = element(
            window, "Property sample text", role=slint_testing.AccessibleRole.TextInput
        )
        text.accessible_value = "From the sidebar"
        sample = element(
            window, "Sample editable field", role=slint_testing.AccessibleRole.TextInput
        )
        expect(sample).to_have_value("From the sidebar")
        element(window, "Reset example").invoke_accessible_default_action()
        wait_until(lambda: float(slider.accessible_value) == 24)
        expect(sample).to_have_value("Hello Slint")


def test_gallery_properties_resize_preview(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "foundations") as window:
        preview = element(window, "Gallery preview")
        original_size = preview.size
        for label, value in [("Preview width", "680"), ("Preview height", "400")]:
            element(
                window, label, role=slint_testing.AccessibleRole.TextInput
            ).accessible_value = value
        expect(preview).to_have_geometry(width=680, height=400)
        sidebar = element(window, "Gallery properties")
        assert (
            sidebar.absolute_position.x + sidebar.size.width
            <= window.root_element.size.width
        )
        assert preview.absolute_position.y < 200
        element(window, "Reset example").invoke_accessible_default_action()
        expect(preview).to_have_geometry(
            width=original_size.width, height=original_size.height
        )
