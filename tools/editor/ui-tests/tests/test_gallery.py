# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import os
import subprocess
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import begin_palette_drag, center, selection_frame
from gradient_interactions import gesture
from slint_testing import keys
from ui_driver import (
    elements_with_label,
    first_window,
    press_key,
    press_shortcut,
    screenshot,
    select_outline_row,
    wait_until,
    window_element_with_label,
)

PAGES = (
    "composition",
    "foundations",
    "controls",
    "inspector-controls",
    "palette",
    "picker",
    "outline",
    "files",
    "inspector",
    "canvas",
    "images",
    "welcome",
    "shell",
)


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
            "--page",
            page,
            "--scenario",
            scenario,
            "--theme",
            theme,
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
        yield window


@pytest.mark.parametrize("page", PAGES)
@pytest.mark.parametrize("theme", ["light", "dark"])
def test_gallery_scenarios_render(
    gallery_binary, editor_environment, tmp_path, page, theme
):
    listing = subprocess.run(
        [str(gallery_binary), "--list"], check=True, capture_output=True, text=True
    ).stdout
    scenarios = dict(line.split(": ", 1) for line in listing.splitlines())[page].split(
        ", "
    )
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
            image = screenshot(window)
            assert image.width >= 800 and image.height >= 600
            assert len(image.resize((80, 60)).getcolors(4801)) > 10
            image.save(
                destination / f"{page}-{scenario.lower().replace(' ', '-')}-{theme}.png"
            )


def test_gallery_palette_drop_and_reset(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "palette") as window:
        canvas = window_element_with_label(window, "Artboard")
        target = slint_testing.LogicalPosition(
            x=canvas.absolute_position.x + 180, y=canvas.absolute_position.y + 470
        )
        begin_palette_drag(window, "Rectangle", target)
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                target, slint_testing.PointerEventButton.Left
            )
        )
        window_element_with_label(window, "Dropped element")
        window_element_with_label(window, "Select rectangle-6")
        window_element_with_label(
            window, "Reset example"
        ).invoke_accessible_default_action()
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "Select rectangle-6")
                else None
            )
        )
        assert not elements_with_label(window.root_element, "Rectangle drag preview")


def test_gallery_outline_selection_reparent_and_reset(
    gallery_binary, editor_environment
):
    with gallery(gallery_binary, editor_environment, "outline") as window:
        title = select_outline_row(window, "title")
        card = window_element_with_label(
            window, "card", slint_testing.AccessibleRole.ListItem
        )
        window.drag_and_drop(center(title), center(card))
        row = window_element_with_label(
            window, "title", slint_testing.AccessibleRole.ListItem
        )
        wait_until(
            lambda: row if row.accessible_description == "Hierarchy level 3" else None
        )
        window_element_with_label(
            window, "Reset example"
        ).invoke_accessible_default_action()
        wait_until(
            lambda: (
                True
                if window_element_with_label(
                    window, "title", slint_testing.AccessibleRole.ListItem
                ).accessible_description
                == "Hierarchy level 2"
                else None
            )
        )


def test_gallery_picker_cancel_and_commit(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "picker") as window:

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
        window_element_with_label(window, "Committed fill")
        assert open_picker().accessible_value.lower().lstrip("#") == "00ff00"


def test_gallery_invalid_launch_is_reported(gallery_binary):
    result = subprocess.run(
        [str(gallery_binary), "--page", "missing"],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode != 0
    assert "Unknown page" in result.stderr


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


def test_gallery_canvas_resize_and_reset(gallery_binary, editor_environment):
    with gallery(gallery_binary, editor_environment, "canvas") as window:
        original = selection_frame(window, "Rectangle")
        handle = window_element_with_label(window, "Rectangle resize bottom-right")
        start = center(handle)
        gesture(
            window, start, slint_testing.LogicalPosition(x=start.x + 40, y=start.y + 25)
        )
        wait_until(
            lambda: (
                True
                if selection_frame(window, "Rectangle")[2] > original[2] + 30
                else None
            )
        )
        changed = selection_frame(window, "Rectangle")
        assert changed[2] == pytest.approx(original[2] + 40, abs=2)
        assert changed[3] == pytest.approx(original[3] + 25, abs=2)
        window_element_with_label(window, "Committed geometry")
        press_shortcut(window, keys.Control, "z")
        wait_until(
            lambda: True if selection_frame(window, "Rectangle") == original else None
        )
        press_shortcut(window, keys.Control, keys.Shift, "z")
        wait_until(
            lambda: True if selection_frame(window, "Rectangle") == changed else None
        )
        window_element_with_label(
            window, "Reset example"
        ).invoke_accessible_default_action()
        wait_until(
            lambda: True if selection_frame(window, "Rectangle") == original else None
        )
