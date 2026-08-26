# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from source_snapshot import SourceSnapshot
from ui_driver import (
    first_window,
    launch_editor,
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


def replace_once(source: bytes, old: bytes, new: bytes) -> bytes:
    assert source.count(old) == 1
    return source.replace(old, new, 1)


def select_element(window: slint_testing.Window, kind: str) -> None:
    select_outline_row(window, ELEMENT_ROWS[kind])
    window_element_with_label(
        window, f"Selected {kind}", slint_testing.AccessibleRole.Region
    )


def inspector_field(
    window: slint_testing.Window,
    label: str,
    role: slint_testing.AccessibleRole | None = None,
) -> slint_testing.Element:
    return window_element_with_label(window, label, role)


def edit_field(
    window: slint_testing.Window,
    label: str,
    value: str,
    role: slint_testing.AccessibleRole | None = None,
) -> None:
    inspector_field(window, label, role).accessible_value = value


def wait_for_field(
    window: slint_testing.Window,
    label: str,
    value: str,
    role: slint_testing.AccessibleRole | None = None,
    timeout: float = 5,
) -> None:
    wait_until(
        lambda: (
            field
            if (field := inspector_field(window, label, role)).accessible_value == value
            else None
        ),
        timeout=timeout,
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


def scroll_to_shadow_details(window: slint_testing.Window) -> None:
    anchor = inspector_field(
        window, "Shadow distance", slint_testing.AccessibleRole.Slider
    )
    position = slint_testing.LogicalPosition(
        x=anchor.absolute_position.x + anchor.size.width / 2,
        y=anchor.absolute_position.y + anchor.size.height / 2,
    )
    window.dispatch_event(
        slint_testing.PointerScrolledEvent(position=position, delta_x=0, delta_y=-320)
    )


@pytest.mark.parametrize(
    ("label", "value", "old", "new"),
    [
        ("Position X", "44", b"        x: 32px;", b"        x: 44px;"),
        ("Position Y", "48", b"        y: 32px;", b"        y: 48px;"),
        ("Width", "176", b"        width: 160px;", b"        width: 176px;"),
        (
            "Height",
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
        select_element(window, "Rectangle")
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
        assert_rendered_element(window, "InspectorCases::inspect-rectangle")


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


@pytest.mark.skip(reason="Requires a Rust root-element property editing fix")
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
        artboard = window_element_with_label(
            window, "Artboard", slint_testing.AccessibleRole.Region
        )
        target = slint_testing.LogicalPosition(
            x=artboard.absolute_position.x + artboard.size.width - 12,
            y=artboard.absolute_position.y + artboard.size.height - 12,
        )
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(target, button))
        window.dispatch_event(slint_testing.PointerReleaseEvent(target, button))
        edit_field(
            window,
            "Root background",
            "#abcdef",
            slint_testing.AccessibleRole.TextInput,
        )
        snapshot.wait_for_exact(
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


@pytest.mark.parametrize("alignment", ("left", "center", "right"))
def test_each_horizontal_image_alignment_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    alignment: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    initial = "left" if alignment == "center" else "center"
    starting_source = replace_once(
        baseline,
        b"        horizontal-alignment: center;",
        f"        horizontal-alignment: {initial};".encode(),
    )
    source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Image")
        edit_field(
            window,
            "Image horizontal alignment",
            alignment,
            slint_testing.AccessibleRole.Combobox,
        )
        snapshot.wait_for_exact(
            replace_once(
                starting_source,
                f"        horizontal-alignment: {initial};".encode(),
                f"        horizontal-alignment: {alignment};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        assert_rendered_element(window, "InspectorCases::inspect-image")


@pytest.mark.parametrize("alignment", ("top", "center", "bottom"))
def test_each_vertical_image_alignment_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    alignment: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    initial = "top" if alignment == "center" else "center"
    starting_source = replace_once(
        baseline,
        b"        vertical-alignment: center;",
        f"        vertical-alignment: {initial};".encode(),
    )
    source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Image")
        edit_field(
            window,
            "Image vertical alignment",
            alignment,
            slint_testing.AccessibleRole.Combobox,
        )
        snapshot.wait_for_exact(
            replace_once(
                starting_source,
                f"        vertical-alignment: {initial};".encode(),
                f"        vertical-alignment: {alignment};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        assert_rendered_element(window, "InspectorCases::inspect-image")


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
            "Image",
            "Image horizontal alignment",
            ("center", "left", "right"),
            "left",
            b"        horizontal-alignment: center;",
            b"        horizontal-alignment: left;",
        ),
        (
            "Image",
            "Image vertical alignment",
            ("center", "top", "bottom"),
            "top",
            b"        vertical-alignment: center;",
            b"        vertical-alignment: top;",
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
    ids=("image-fit", "horizontal-alignment", "vertical-alignment", "font-weight"),
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


ATOMIC_SHADOW_OFFSET_REQUIRED = pytest.mark.skip(
    reason="Requires Rust support for atomic shadow-offset edits"
)
SHADOW_CONTROLS = (
    ("color", "Shadow color", "#12345678"),
    pytest.param(
        "angle",
        "Shadow angle",
        "0",
        marks=ATOMIC_SHADOW_OFFSET_REQUIRED,
    ),
    ("distance", "Shadow distance", "12"),
    ("blur", "Shadow blur", "24"),
    ("spread", "Shadow spread", "6"),
)
SHADOW_BOUNDARIES = (
    ("distance", "Shadow distance", "0"),
    ("distance", "Shadow distance", "96"),
    ("blur", "Shadow blur", "0"),
    ("blur", "Shadow blur", "128"),
    ("spread", "Shadow spread", "-64"),
    ("spread", "Shadow spread", "64"),
    pytest.param(
        "angle",
        "Shadow angle",
        "359",
        marks=ATOMIC_SHADOW_OFFSET_REQUIRED,
    ),
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


@pytest.mark.parametrize("family", ("drop", "inner"))
@pytest.mark.parametrize(
    ("control", "label", "value"),
    SHADOW_CONTROLS,
    ids=("color", "angle", "distance", "blur", "spread"),
)
def test_each_shadow_family_control_writes_exact_source(
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
    source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Rectangle")
        if control in {"blur", "spread"}:
            scroll_to_shadow_details(window)
            inspector_field(
                window,
                f"{label} value",
                slint_testing.AccessibleRole.TextInput,
            )
        edit_field(window, label, value)
        snapshot.wait_for_exact(
            shadow_expected(starting_source, family, control, value),
            relative_path=INSPECTOR_SOURCE,
        )
        assert_rendered_element(window, "InspectorCases::inspect-rectangle")


@pytest.mark.parametrize("family", ("drop", "inner"))
@pytest.mark.parametrize(
    ("control", "label", "value"),
    SHADOW_BOUNDARIES,
    ids=(
        "distance-0",
        "distance-96",
        "blur-0",
        "blur-128",
        "spread--64",
        "spread-64",
        "angle-359",
    ),
)
def test_shadow_control_boundary_writes_exact_source(
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
    source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Rectangle")
        if control in {"blur", "spread"}:
            scroll_to_shadow_details(window)
            inspector_field(
                window,
                f"{label} value",
                slint_testing.AccessibleRole.TextInput,
            )
        edit_field(window, label, value)
        snapshot.wait_for_exact(
            shadow_expected(starting_source, family, control, value),
            relative_path=INSPECTOR_SOURCE,
        )
        assert_rendered_element(window, "InspectorCases::inspect-rectangle")


@pytest.mark.skip(reason="Requires Rust support for atomic multi-property edits")
@pytest.mark.parametrize("effect", ("none", "drop", "inner"))
def test_rectangle_effect_value_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    effect: str,
) -> None:
    assert editor_binary and editor_environment and fixture_project and effect


INVALID_EDITS = (
    ("invalid-number", "Rectangle", "Position X", "invalid"),
    ("empty-number", "Rectangle", "Position X", ""),
    ("empty-family", "Text", "Font family", ""),
    ("empty-fit", "Image", "Image fit", ""),
    ("nonnumeric-y", "Rectangle", "Position Y", "invalid"),
    ("zero-width", "Rectangle", "Width", "0"),
    ("negative-width", "Rectangle", "Width", "-1"),
    ("zero-height", "Rectangle", "Height", "0"),
    ("negative-height", "Rectangle", "Height", "-1"),
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
