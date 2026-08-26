# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from source_oracle import SourceSnapshot
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
) -> None:
    wait_until(
        lambda: (
            field
            if (field := inspector_field(window, label, role)).accessible_value == value
            else None
        )
    )


def assert_rendered_element(window: slint_testing.Window, element_id: str) -> None:
    wait_until(
        lambda: (
            element
            if (element := next(iter(window.find_elements_by_id(element_id)), None))
            else None
        )
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
        wait_for_field(window, label, value, slint_testing.AccessibleRole.TextInput)
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
        wait_for_field(window, label, value, slint_testing.AccessibleRole.TextInput)
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
        wait_for_field(
            window, "Image source", value, slint_testing.AccessibleRole.TextInput
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
        wait_for_field(
            window, "Font family", "Fira Sans", slint_testing.AccessibleRole.TextInput
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
