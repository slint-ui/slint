# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore tobytes

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center as element_center
from editor_sync import wait_for_source
from inspector_interactions import (
    FIELDS,
    edit_field,
    inspector_field,
    slider_position,
    wait_for_field,
)
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_driver import (
    first_window,
    launch_editor,
    press_keys,
    press_shortcut,
    screenshot,
    select_outline_row,
    wait_until,
    window_element_with_label,
)

INSPECTOR_SOURCE = "InspectorCases.slint"
ELEMENT_ROWS = {
    "Rectangle": "inspect-rectangle",
    "Text": "inspect-text",
    "Image": "inspect-image",
}


def select_element(window: slint_testing.Window, kind: str) -> None:
    select_outline_row(window, ELEMENT_ROWS[kind])
    window_element_with_label(
        window, f"Selected {kind}", slint_testing.AccessibleRole.Region
    )


def open_combo_and_accept(
    window: slint_testing.Window,
    label: str,
    expected_options: tuple[str, ...],
    value: str,
) -> None:
    combo = inspector_field(window, label, slint_testing.AccessibleRole.Combobox)
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
    combo.accessible_value = value


def assert_rendered_element(window: slint_testing.Window, element_id: str) -> None:
    wait_until(
        lambda: (
            element
            if (element := next(iter(window.find_elements_by_id(element_id)), None))
            else None
        )
    )


def image_alignment_button(
    window: slint_testing.Window, vertical: str, horizontal: str
) -> slint_testing.Element:
    position = (
        "center"
        if vertical == horizontal == "center"
        else f"{'middle' if vertical == 'center' else vertical} {horizontal}"
    )
    return inspector_field(
        window, f"Align image {position}", slint_testing.AccessibleRole.Button
    )


@pytest.mark.parametrize(
    ("property_name", "original_value", "value", "old", "new"),
    [
        ("x", 32, "44", b"        x: 32px;", b"        x: 44px;"),
        ("y", 32, "48", b"        y: 32px;", b"        y: 48px;"),
        ("width", 160, "176", b"        width: 160px;", b"        width: 176px;"),
        (
            "height",
            96,
            "112",
            b"        width: 160px;\n        height: 96px;",
            b"        width: 160px;\n        height: 112px;",
        ),
    ],
    ids=("x", "y", "width", "height"),
)
def test_geometry_field_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    property_name: str,
    original_value: float,
    value: str,
    old: bytes,
    new: bytes,
) -> None:
    label = FIELDS[property_name]
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Rectangle")

        def rendered_value():
            elements = window.find_elements_by_id("InspectorCases::inspect-rectangle")
            if len(elements) != 1:
                return None
            rectangle = elements[0]
            geometry = (
                rectangle.absolute_position
                if property_name in ("x", "y")
                else rectangle.size
            )
            # Preview replacement can invalidate the handle during the property read.
            return getattr(geometry, property_name) if rectangle.is_valid else None

        expected = float(value)
        if property_name in ("x", "y"):
            # Absolute positions include the preview's offset in the editor window.
            preview_offset = wait_until(rendered_value) - original_value
            expected += preview_offset

        edit_field(window, label, value, slint_testing.AccessibleRole.TextInput)
        snapshot.wait_for_exact(
            replace_once(baseline, old, new), relative_path=INSPECTOR_SOURCE
        )
        wait_for_field(
            window,
            label,
            value,
            slint_testing.AccessibleRole.TextInput,
        )

        # Source and field updates can precede preview replacement.
        # The existing rectangle must reflect the edit, not merely exist.
        def geometry_matches():
            actual = rendered_value()
            return actual if actual == pytest.approx(expected) else None

        wait_until(geometry_matches)


@pytest.mark.parametrize(
    ("kind", "label", "value", "old", "new"),
    [
        (
            "Rectangle",
            "Rectangle background",
            "#123456",
            b"        background: #2563eb;",
            b"        background: #123456;",
        ),
        (
            "Text",
            "Text color",
            "#123456",
            b"        color: #111827;",
            b"        color: #123456;",
        ),
    ],
    ids=("rectangle", "text"),
)
def test_element_color_field_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    label: str,
    value: str,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, kind)
        edit_field(window, label, value, slint_testing.AccessibleRole.TextInput)
        snapshot.wait_for_exact(
            replace_once(baseline, old, new), relative_path=INSPECTOR_SOURCE
        )
        assert_rendered_element(window, f"InspectorCases::inspect-{kind.lower()}")


def test_root_background_field_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        outline = window_element_with_label(window, "Current file outline")
        root_row = (
            outline.query_descendants()
            .match_accessible_role(slint_testing.AccessibleRole.ListItem)
            .find_all()[0]
        )
        root_row.invoke_accessible_default_action()
        wait_for_field(
            window, "Root background", "#f8fafc", slint_testing.AccessibleRole.TextInput
        )
        edit_field(
            window,
            "Root background",
            "#abcdef",
            slint_testing.AccessibleRole.TextInput,
        )
        snapshot.wait_for_applied(
            replace_once(
                baseline,
                b"    background: #f8fafc;",
                b"    background: #abcdef;",
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        wait_for_field(
            window,
            "Root background",
            "#abcdef",
            slint_testing.AccessibleRole.TextInput,
        )


@pytest.mark.parametrize("fit", ("fill", "preserve", "contain", "cover"))
def test_each_image_fit_value_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    fit: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    initial = "fill" if fit == "contain" else "contain"
    starting_source = replace_once(
        baseline,
        b"        image-fit: contain;",
        f"        image-fit: {initial};".encode(),
    )
    source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Image")
        edit_field(window, "Image fit", fit, slint_testing.AccessibleRole.Combobox)
        snapshot.wait_for_exact(
            replace_once(
                starting_source,
                f"        image-fit: {initial};".encode(),
                f"        image-fit: {fit};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        assert_rendered_element(window, "InspectorCases::inspect-image")


def test_image_alignment_grid_writes_both_properties(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Image")
        window_element_with_label(
            window, "Alignment", slint_testing.AccessibleRole.Text
        )
        assert image_alignment_button(window, "center", "center").accessible_checked
        image_alignment_button(
            window, "center", "center"
        ).invoke_accessible_default_action()
        snapshot.assert_unchanged()

        for vertical in ("top", "center", "bottom"):
            for horizontal in ("left", "center", "right"):
                button = image_alignment_button(window, vertical, horizontal)
                if vertical == "top" and horizontal == "left":
                    position = element_center(button)
                    mouse_button = slint_testing.PointerEventButton.Left
                    window.dispatch_event(
                        slint_testing.PointerPressEvent(position, mouse_button)
                    )
                    window.dispatch_event(
                        slint_testing.PointerReleaseEvent(position, mouse_button)
                    )
                else:
                    button.invoke_accessible_default_action()
                expected = replace_once(
                    baseline,
                    b"        horizontal-alignment: center;",
                    f"        horizontal-alignment: {horizontal};".encode(),
                )
                expected = replace_once(
                    expected,
                    b"        vertical-alignment: center;",
                    f"        vertical-alignment: {vertical};".encode(),
                )
                snapshot.wait_for_applied(expected, relative_path=INSPECTOR_SOURCE)
                assert image_alignment_button(
                    window, vertical, horizontal
                ).accessible_checked
                display = (
                    "Center"
                    if vertical == horizontal == "center"
                    else f"{vertical.title() if vertical != 'center' else 'Middle'} {horizontal}"
                )
                window_element_with_label(
                    window, display, slint_testing.AccessibleRole.Text
                )
                assert_rendered_element(window, "InspectorCases::inspect-image")


def test_image_alignment_grid_one_undo_restores_both_properties(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(
        baseline,
        b"        horizontal-alignment: center;",
        b"        horizontal-alignment: right;",
    )
    expected = replace_once(
        expected,
        b"        vertical-alignment: center;",
        b"        vertical-alignment: bottom;",
    )
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Image")
        image_alignment_button(
            window, "bottom", "right"
        ).invoke_accessible_default_action()
        snapshot.wait_for_applied(expected, relative_path=INSPECTOR_SOURCE)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=INSPECTOR_SOURCE)
        assert image_alignment_button(window, "center", "center").accessible_checked
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, relative_path=INSPECTOR_SOURCE)
        assert image_alignment_button(window, "bottom", "right").accessible_checked


def test_image_alignment_grid_replaces_custom_expression(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    starting_source = replace_once(
        baseline,
        b"        horizontal-alignment: center;",
        b"        horizontal-alignment: (left);",
    )
    source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Image")
        window_element_with_label(window, "Custom", slint_testing.AccessibleRole.Text)
        assert not any(
            image_alignment_button(window, vertical, horizontal).accessible_checked
            for vertical in ("top", "center", "bottom")
            for horizontal in ("left", "center", "right")
        )
        image_alignment_button(window, "top", "left").invoke_accessible_default_action()
        expected = replace_once(
            starting_source,
            b"        horizontal-alignment: (left);",
            b"        horizontal-alignment: left;",
        )
        expected = replace_once(
            expected,
            b"        vertical-alignment: center;",
            b"        vertical-alignment: top;",
        )
        snapshot.wait_for_applied(expected, relative_path=INSPECTOR_SOURCE)
        assert image_alignment_button(window, "top", "left").accessible_checked


def test_image_source_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    value = '@image-url("assets/alternate.svg")'

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Image")
        edit_field(
            window, "Image source", value, slint_testing.AccessibleRole.TextInput
        )
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                b'        source: @image-url("assets/checker.svg");',
                b'        source: @image-url("assets/alternate.svg");',
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        assert_rendered_element(window, "InspectorCases::inspect-image")


def test_font_family_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Text")
        edit_field(
            window, "Font family", "Fira Sans", slint_testing.AccessibleRole.TextInput
        )
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                b'        font-family: "Inter";',
                b'        font-family: "Fira Sans";',
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        window_element_with_label(
            window, "Inspector text", slint_testing.AccessibleRole.Text
        )


@pytest.mark.parametrize("weight", tuple(str(value) for value in range(100, 1000, 100)))
def test_each_font_weight_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    weight: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    initial = "500" if weight == "400" else "400"
    starting_source = replace_once(
        baseline,
        b"        font-weight: 400;",
        f"        font-weight: {initial};".encode(),
    )
    source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Text")
        edit_field(window, "Font weight", weight, slint_testing.AccessibleRole.Combobox)
        snapshot.wait_for_exact(
            replace_once(
                starting_source,
                f"        font-weight: {initial};".encode(),
                f"        font-weight: {weight};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        window_element_with_label(
            window, "Inspector text", slint_testing.AccessibleRole.Text
        )


@pytest.mark.parametrize(
    ("kind", "label", "options", "value", "old", "new"),
    [
        (
            "Image",
            "Image fit",
            ("fill", "preserve", "contain", "cover"),
            "cover",
            b"        image-fit: contain;",
            b"        image-fit: cover;",
        ),
        (
            "Text",
            "Font weight",
            (
                "Thin",
                "Extra Light",
                "Light",
                "Normal",
                "Medium",
                "Semi Bold",
                "Bold",
                "Extra Bold",
                "Black",
            ),
            "700",
            b"        font-weight: 400;",
            b"        font-weight: 700;",
        ),
    ],
    ids=("image-fit", "font-weight"),
)
def test_combobox_opens_options_and_accepts_accessible_choice(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    label: str,
    options: tuple[str, ...],
    value: str,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, kind)
        open_combo_and_accept(window, label, options, value)
        snapshot.wait_for_exact(
            replace_once(baseline, old, new),
            relative_path=INSPECTOR_SOURCE,
        )


@pytest.mark.parametrize(
    ("case", "value", "expected"),
    [
        ("numeric", "24", "24px"),
        ("expression", "20px * 1.5", "20px * 1.5"),
    ],
    ids=("numeric", "expression"),
)
def test_numeric_and_expression_font_sizes_write_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    case: str,
    value: str,
    expected: str,
) -> None:
    assert case
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Text")
        edit_field(window, "Font size", value, slint_testing.AccessibleRole.TextInput)
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                b"        font-size: 20px;",
                f"        font-size: {expected};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        window_element_with_label(
            window, "Inspector text", slint_testing.AccessibleRole.Text
        )


@pytest.mark.parametrize(
    ("case", "value", "expected_line", "rendered"),
    [
        ("literal", '"Literal content"', '"Literal content"', "Literal content"),
        (
            "expression",
            '"Expression " + "content"',
            '"Expression " + "content"',
            "Expression content",
        ),
    ],
    ids=("literal", "expression"),
)
def test_text_content_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    case: str,
    value: str,
    expected_line: str,
    rendered: str,
) -> None:
    assert case
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Text")
        edit_field(
            window, "Text content", value, slint_testing.AccessibleRole.TextInput
        )
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                b'        text: "Inspector text";',
                f"        text: {expected_line};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        window_element_with_label(window, rendered, slint_testing.AccessibleRole.Text)


def test_invalid_text_content_does_not_change_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Text")
        edit_field(
            window,
            "Text content",
            "unknown_identifier",
            slint_testing.AccessibleRole.TextInput,
        )
        snapshot.assert_unchanged()
        wait_for_field(
            window,
            "Text content",
            '"Inspector text"',
            slint_testing.AccessibleRole.TextInput,
        )
        window_element_with_label(
            window, "Inspector text", slint_testing.AccessibleRole.Text
        )


def test_inspector_length_fields_show_numbers_without_pixel_labels(
    editor_binary, editor_environment, fixture_project
):
    source = fixture_project / INSPECTOR_SOURCE
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)
        select_element(window, "Rectangle")
        for label in [
            "All corner radii",
            "Shadow distance value",
            "Shadow blur value",
            "Shadow spread value",
        ]:
            field = inspector_field(
                window, label, slint_testing.AccessibleRole.TextInput
            )
            texts = (
                field.query_descendants()
                .match_accessible_role(slint_testing.AccessibleRole.Text)
                .find_all()
            )
            assert not any(text.accessible_label == "px" for text in texts)
        pane = window_element_with_label(window, "Inspector and outline")
        texts = (
            pane.query_descendants()
            .match_accessible_role(slint_testing.AccessibleRole.Text)
            .find_all()
        )
        assert not any(text.accessible_label == "px" for text in texts)


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


def artboard_pixels(window: slint_testing.Window) -> bytes:
    artboard = window_element_with_label(window, "Artboard")
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


@pytest.mark.parametrize("family", ("drop", "inner"))
@pytest.mark.parametrize(
    ("control", "label", "value"),
    SHADOW_EDITS,
)
def test_shadow_control_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    family: str,
    control: str,
    label: str,
    value: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    starting_source = shadow_source(source_file.read_bytes(), family)
    if family == "inner":
        source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, starting_source)
        window = first_window(editor)
        select_element(window, "Rectangle")
        edit_field(window, label, value)
        snapshot.wait_for_applied(
            shadow_expected(starting_source, family, control, value),
            relative_path=INSPECTOR_SOURCE,
        )


@pytest.mark.parametrize("family", ("drop", "inner"))
@pytest.mark.parametrize(
    ("control", "label", "initial", "progress", "value"),
    [
        ("distance", "Shadow distance", 8 / 96, 0.5, "48"),
        ("blur", "Shadow blur", 16 / 128, 0.5, "64"),
        ("spread", "Shadow spread", 0.5, 0.25, "-32"),
    ],
)
@pytest.mark.parametrize("outcome", ("commit", "cancel", "selection", "source"))
def test_shadow_slider_previews_without_source_writes(
    editor_binary,
    editor_environment,
    fixture_project,
    family,
    control,
    label,
    initial,
    progress,
    value,
    outcome,
):
    source = fixture_project / INSPECTOR_SOURCE
    baseline = shadow_source(source.read_bytes(), family)
    source.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_element(window, "Rectangle")
        start = slider_position(window, label, initial)
        end = slider_position(window, label, progress)
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        before = artboard_pixels(window)
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                start, slint_testing.PointerEventButton.Left
            )
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(window, label + " value", value)
        assert artboard_pixels(window) != before
        snapshot.assert_unchanged()
        if outcome == "cancel":
            press_keys(window, keys.Escape)
        elif outcome == "selection":
            select_element(window, "Text")
        elif outcome == "source":
            source.write_bytes(baseline + b"\n// External edit\n")
            snapshot.wait_for_applied(
                baseline + b"\n// External edit\n", INSPECTOR_SOURCE
            )
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                end, slint_testing.PointerEventButton.Left
            )
        )
        if outcome != "commit":
            if outcome == "source":
                snapshot.wait_for_applied(
                    baseline + b"\n// External edit\n", INSPECTOR_SOURCE
                )
            else:
                snapshot.assert_unchanged()
            if outcome == "selection":
                select_element(window, "Rectangle")
            assert artboard_pixels(window) == before
        else:
            expected = shadow_expected(baseline, family, control, value)
            snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
            press_shortcut(window, keys.Control, "z")
            snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)
            press_shortcut(window, keys.Control, keys.Shift, "z")
            snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)


@pytest.mark.parametrize("family", ("drop", "inner"))
@pytest.mark.parametrize("outcome", ("commit", "cancel"))
def test_shadow_angle_previews_without_source_writes(
    editor_binary, editor_environment, fixture_project, family, outcome
):
    source = fixture_project / INSPECTOR_SOURCE
    baseline = shadow_source(source.read_bytes(), family)
    source.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_element(window, "Rectangle")
        dial = inspector_field(
            window, "Shadow angle", slint_testing.AccessibleRole.Slider
        )
        position, size = dial.absolute_position, dial.size
        center_x = position.x + size.width / 2
        center_y = position.y + size.height / 2
        start = slint_testing.LogicalPosition(center_x, center_y + size.height / 3)
        end = slint_testing.LogicalPosition(center_x + size.width / 3, center_y)
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        before = artboard_pixels(window)
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                start, slint_testing.PointerEventButton.Left
            )
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(window, "Shadow angle", "0", slint_testing.AccessibleRole.Slider)
        assert artboard_pixels(window) != before
        snapshot.assert_unchanged()
        if outcome == "cancel":
            press_keys(window, keys.Escape)
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                end, slint_testing.PointerEventButton.Left
            )
        )
        if outcome == "cancel":
            snapshot.assert_unchanged()
            assert artboard_pixels(window) == before
        else:
            snapshot.wait_for_applied(
                shadow_expected(baseline, family, "angle", "0"), INSPECTOR_SOURCE
            )


@pytest.mark.parametrize("family", ("drop", "inner"))
def test_shadow_distance_keeps_direction_through_zero(
    editor_binary, editor_environment, fixture_project, family
):
    source = fixture_project / INSPECTOR_SOURCE
    baseline = (
        shadow_source(source.read_bytes(), family)
        .replace(
            f"{family}-shadow-offset-x: 0px;".encode(),
            f"{family}-shadow-offset-x: -8px;".encode(),
        )
        .replace(
            f"{family}-shadow-offset-y: 8px;".encode(),
            f"{family}-shadow-offset-y: 0px;".encode(),
        )
    )
    source.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_element(window, "Rectangle")
        start = slider_position(window, "Shadow distance", 8 / 96)
        zero = slider_position(window, "Shadow distance", 0)
        end = slider_position(window, "Shadow distance", 0.5)
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                start, slint_testing.PointerEventButton.Left
            )
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(zero))
        wait_for_field(window, "Shadow distance value", "0")
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(window, "Shadow distance value", "48")
        snapshot.assert_unchanged()
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                end, slint_testing.PointerEventButton.Left
            )
        )
        expected = baseline.replace(
            f"{family}-shadow-offset-x: -8px;".encode(),
            f"{family}-shadow-offset-x: -48px;".encode(),
        )
        snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)


@pytest.mark.parametrize("effect", ("none", "drop", "inner"))
def test_rectangle_effect_value_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    effect: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    starting_source = shadow_source(baseline, "inner" if effect == "drop" else "drop")
    if effect == "drop":
        source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)
    expected = b"".join(
        line
        for line in baseline.splitlines(keepends=True)
        if not line.lstrip().startswith(b"drop-shadow-")
    )
    if effect != "none":
        color = "#00000040" if effect == "drop" else "#00000030"
        properties = (
            f"        {effect}-shadow-color: {color};\n"
            f"        {effect}-shadow-blur: 16px;\n"
            f"        {effect}-shadow-spread: 0px;\n"
            f"        {effect}-shadow-offset-x: 0px;\n"
            f"        {effect}-shadow-offset-y: 8px;\n"
        ).encode()
        anchor = (
            b"        height: 96px;\n        background: #2563eb;"
            if effect == "drop"
            else b"        background: #2563eb;"
        )
        expected = replace_once(expected, anchor, properties + anchor)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Rectangle")
        edit_field(
            window, "Rectangle effect", effect, slint_testing.AccessibleRole.Combobox
        )
        snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
        wait_for_field(
            window,
            "Rectangle effect",
            {"none": "None", "drop": "Drop Shadow", "inner": "Inner Shadow"}[effect],
            slint_testing.AccessibleRole.Combobox,
        )
        select_element(window, "Rectangle")
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(starting_source, INSPECTOR_SOURCE)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)


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


@pytest.mark.parametrize(
    ("case", "kind", "label", "value"),
    INVALID_EDITS,
    ids=tuple(case for case, _, _, _ in INVALID_EDITS),
)
def test_invalid_or_empty_inspector_edit_does_not_change_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    case: str,
    kind: str,
    label: str,
    value: str,
) -> None:
    assert case
    source_file = fixture_project / INSPECTOR_SOURCE
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, kind)
        role = slint_testing.AccessibleRole.Combobox if label == "Image fit" else None
        field = inspector_field(window, label, role)
        value_before = field.accessible_value
        edit_field(window, label, value, role)
        snapshot.assert_unchanged()
        wait_for_field(window, label, value_before, role)
        window_element_with_label(
            window, f"Selected {kind}", slint_testing.AccessibleRole.Region
        )


def test_invalid_rectangle_color_does_not_change_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Rectangle")
        edit_field(
            window,
            "Rectangle background",
            "not-a-color",
            slint_testing.AccessibleRole.TextInput,
        )
        snapshot.assert_unchanged()
        wait_for_field(
            window,
            "Rectangle background",
            "#2563eb",
            slint_testing.AccessibleRole.TextInput,
        )
        window_element_with_label(
            window, "Selected Rectangle", slint_testing.AccessibleRole.Region
        )
