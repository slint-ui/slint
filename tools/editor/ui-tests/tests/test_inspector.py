# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# ruff: noqa: I001

# cspell:ignore tobytes


import pytest
import slint_testing
from inspector_interactions import (
    FIELDS,
    inspector_field,
)
from slint_test import Locator, Window
from source_snapshot import replace_once
from ui_driver import (
    screenshot,
    select_outline_row,
    wait_until,
)

INSPECTOR_SOURCE = "InspectorCases.slint"
ELEMENT_ROWS = {
    "Rectangle": "inspect-rectangle",
    "Text": "inspect-text",
    "Image": "inspect-image",
}


def select_element(window: Window, kind: str) -> None:
    select_outline_row(window, ELEMENT_ROWS[kind])
    window.get_by_role("region", name=f"Selected {kind}").wait_for()


def open_combo_and_accept(
    window: Window,
    label: str,
    expected_options: tuple[str, ...],
    value: str,
) -> None:
    combo = inspector_field(window, label, "combobox")
    combo.invoke_accessible_expand_action()

    def menu_labels() -> tuple[str, ...] | None:
        items = (
            window.root_element.query_descendants()
            .match_type_name("MenuItem")
            .find_all()
        )
        labels = tuple(
            text.accessible_label
            for item in items
            for text in item.query_descendants()
            .match_accessible_role(slint_testing.AccessibleRole.Text)
            .find_all()
            if text.accessible_label and text.accessible_label != "✓"
        )
        return labels if labels == expected_options else None

    wait_until(menu_labels)
    window.get_by_role("combobox", name=label).set_accessible_value(value)


def assert_rendered_element(window: Window, element_id: str) -> None:
    window.get_by_id(element_id).wait_for()


def image_alignment_button(window: Window, vertical: str, horizontal: str) -> Locator:
    position = (
        "center"
        if vertical == horizontal == "center"
        else f"{'middle' if vertical == 'center' else vertical} {horizontal}"
    )
    return window.get_by_role(
        "complementary", name="Inspector and outline"
    ).get_by_role("button", name=f"Align image {position}")


SHADOW_EDITS = (
    pytest.param("color", "Shadow color", "#12345678", id="color"),
    pytest.param("angle", "Shadow angle", "0", id="angle"),
    pytest.param("distance", "Shadow distance", "12", id="distance"),
    pytest.param("blur", "Shadow blur", "24", id="blur"),
    pytest.param("spread", "Shadow spread", "6", id="spread"),
    pytest.param("distance", "Shadow distance", "0", id="distance-0"),
    pytest.param("distance", "Shadow distance", "96", id="distance-96"),
    pytest.param("blur", "Shadow blur", "0", id="blur-0"),
    pytest.param("blur", "Shadow blur", "128", id="blur-128"),
    pytest.param("spread", "Shadow spread", "-64", id="spread--64"),
    pytest.param("spread", "Shadow spread", "64", id="spread-64"),
    pytest.param("angle", "Shadow angle", "359", id="angle-359"),
)


def shadow_source(baseline: bytes, family: str) -> bytes:
    return (
        baseline
        if family == "drop"
        else baseline.replace(b"drop-shadow-", b"inner-shadow-")
    )


def shadow_expected(source: bytes, family: str, control: str, value: str) -> bytes:
    prefix = f"        {family}-shadow-".encode()
    if control == "color":
        return replace_once(
            source,
            prefix + b"color: #00000040;",
            prefix + f"color: {value};".encode(),
        )
    if control == "angle":
        return replace_once(
            source,
            prefix + b"offset-x: 0px;\n" + prefix + b"offset-y: 8px;",
            prefix + b"offset-x: 8px;\n" + prefix + b"offset-y: 0px;",
        )
    if control == "distance":
        return replace_once(
            source,
            prefix + b"offset-y: 8px;",
            prefix + f"offset-y: {value}px;".encode(),
        )
    old_value = "16" if control == "blur" else "0"
    return replace_once(
        source,
        prefix + f"{control}: {old_value}px;".encode(),
        prefix + f"{control}: {value}px;".encode(),
    )


def artboard_pixels(window: Window) -> bytes:
    artboard = window.get_by_accessible_name("Artboard").resolve()
    image = screenshot(window)
    scale = image.width / window.root_element.size.width
    x, y = artboard.absolute_position.x, artboard.absolute_position.y
    return image.crop(
        (
            round(x * scale),
            round(y * scale),
            round((x + artboard.size.width) * scale),
            round((y + artboard.size.height) * scale),
        )
    ).tobytes()


INVALID_EDITS = (
    ("invalid-number", "Rectangle", FIELDS["x"], "invalid"),
    ("empty-number", "Rectangle", FIELDS["x"], ""),
    ("empty-family", "Text", "Font family", ""),
    ("empty-fit", "Image", "Image fit", ""),
    ("nonnumeric-y", "Rectangle", FIELDS["y"], "invalid"),
    ("zero-width", "Rectangle", FIELDS["width"], "0"),
    ("negative-width", "Rectangle", FIELDS["width"], "-1"),
    ("zero-height", "Rectangle", FIELDS["height"], "0"),
    ("negative-height", "Rectangle", FIELDS["height"], "-1"),
)

from inspector_geometry_cases import (  # noqa: F401
    test_geometry_field_writes_exact_source,
    test_geometry_prefix_scrubs_with_transient_preview,
    test_geometry_scrub_reverts_when_commit_is_rejected,
    test_element_color_field_writes_exact_source,
)

from inspector_property_cases import (  # noqa: F401
    test_root_background_field_writes_exact_source,
    test_each_image_fit_value_writes_exact_source,
    test_image_alignment_grid_writes_both_properties,
    test_image_alignment_grid_one_undo_restores_both_properties,
    test_image_alignment_grid_replaces_custom_expression,
    test_image_source_writes_exact_source,
    test_font_family_writes_exact_source,
    test_each_font_weight_writes_exact_source,
    test_combobox_opens_options_and_accepts_accessible_choice,
    test_numeric_and_expression_font_sizes_write_exact_source,
    test_text_content_writes_exact_source,
    test_invalid_text_content_does_not_change_source,
    test_inspector_length_fields_show_numbers_without_pixel_labels,
    test_shadow_control_writes_exact_source,
)

from inspector_effect_cases import (  # noqa: F401
    test_shadow_slider_previews_without_source_writes,
    test_shadow_angle_previews_without_source_writes,
    test_shadow_distance_keeps_direction_through_zero,
    test_rectangle_effect_value_writes_exact_source,
    test_invalid_or_empty_inspector_edit_does_not_change_source,
    test_invalid_rectangle_color_does_not_change_source,
)
