# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore tobytes

from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import (
    edit_field,
    inspector_field,
    wait_for_field,
)
from slint_test import expect
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from test_inspector import (
    INSPECTOR_SOURCE,
    SHADOW_EDITS,
    assert_rendered_element,
    image_alignment_button,
    open_combo_and_accept,
    select_element,
    shadow_expected,
    shadow_source,
)


def test_root_background_field_writes_exact_source(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with editor_factory(source_file) as editor:
        window = editor.window
        window.get_by_role("list", name="Current file outline").get_by_role(
            "list-item"
        ).nth(0).activate()
        wait_for_field(window, "Root background", "#f8fafc", "text-input")
        edit_field(
            window,
            "Root background",
            "#abcdef",
            "text-input",
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
            "text-input",
        )


@pytest.mark.parametrize("fit", ("fill", "preserve", "contain", "cover"))
def test_each_image_fit_value_writes_exact_source(
    editor_factory,
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

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Image")
        edit_field(window, "Image fit", fit, "combobox")
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
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Image")
        window.get_by_role("text", name="Alignment").wait_for()
        expect(image_alignment_button(window, "center", "center")).to_be_checked()
        image_alignment_button(window, "center", "center").activate()
        snapshot.assert_unchanged()

        for vertical in ("top", "center", "bottom"):
            for horizontal in ("left", "center", "right"):
                button = image_alignment_button(window, vertical, horizontal)
                if vertical == "top" and horizontal == "left":
                    position = button.center()
                    window.pointer.press_at(position)
                    window.pointer.release_at(position)
                else:
                    button.activate()
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
                expect(
                    image_alignment_button(window, vertical, horizontal)
                ).to_be_checked()
                display = (
                    "Center"
                    if vertical == horizontal == "center"
                    else f"{vertical.title() if vertical != 'center' else 'Middle'} {horizontal}"
                )
                window.get_by_role("text", name=display).wait_for()
                assert_rendered_element(window, "InspectorCases::inspect-image")


def test_image_alignment_grid_one_undo_restores_both_properties(
    editor_factory,
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

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Image")
        image_alignment_button(window, "bottom", "right").activate()
        snapshot.wait_for_applied(expected, relative_path=INSPECTOR_SOURCE)
        window.keyboard.shortcut(keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=INSPECTOR_SOURCE)
        expect(image_alignment_button(window, "center", "center")).to_be_checked()
        window.keyboard.shortcut(keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, relative_path=INSPECTOR_SOURCE)
        expect(image_alignment_button(window, "bottom", "right")).to_be_checked()


def test_image_alignment_grid_replaces_custom_expression(
    editor_factory,
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

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Image")
        window.get_by_role("text", name="Custom").wait_for()
        assert not any(
            image_alignment_button(window, vertical, horizontal).read(
                lambda button: button.accessible_checked
            )
            for vertical in ("top", "center", "bottom")
            for horizontal in ("left", "center", "right")
        )
        image_alignment_button(window, "top", "left").activate()
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
        expect(image_alignment_button(window, "top", "left")).to_be_checked()


def test_image_source_writes_exact_source(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    value = '@image-url("assets/alternate.svg")'

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Image")
        edit_field(window, "Image source", value, "text-input")
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
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Text")
        edit_field(window, "Font family", "Fira Sans", "text-input")
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                b'        font-family: "Inter";',
                b'        font-family: "Fira Sans";',
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        window.get_by_role("text", name="Inspector text").wait_for()


@pytest.mark.parametrize("weight", tuple(str(value) for value in range(100, 1000, 100)))
def test_each_font_weight_writes_exact_source(
    editor_factory,
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

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Text")
        edit_field(window, "Font weight", weight, "combobox")
        snapshot.wait_for_exact(
            replace_once(
                starting_source,
                f"        font-weight: {initial};".encode(),
                f"        font-weight: {weight};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        window.get_by_role("text", name="Inspector text").wait_for()


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
    editor_factory,
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

    with editor_factory(source_file) as editor:
        window = editor.window
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
    editor_factory,
    fixture_project: Path,
    case: str,
    value: str,
    expected: str,
) -> None:
    assert case
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Text")
        edit_field(window, "Font size", value, "text-input")
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                b"        font-size: 20px;",
                f"        font-size: {expected};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        window.get_by_role("text", name="Inspector text").wait_for()


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
    editor_factory,
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

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Text")
        edit_field(window, "Text content", value, "text-input")
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                b'        text: "Inspector text";',
                f"        text: {expected_line};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        window.get_by_role("text", name=rendered).wait_for()


def test_invalid_text_content_does_not_change_source(
    editor_factory,
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    snapshot = SourceSnapshot.capture(fixture_project)

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Text")
        edit_field(
            window,
            "Text content",
            "unknown_identifier",
            "text-input",
        )
        snapshot.assert_unchanged()
        wait_for_field(
            window,
            "Text content",
            '"Inspector text"',
            "text-input",
        )
        window.get_by_role("text", name="Inspector text").wait_for()


def test_inspector_length_fields_show_numbers_without_pixel_labels(
    editor_factory, fixture_project
):
    source = fixture_project / INSPECTOR_SOURCE
    with editor_factory(source) as editor:
        wait_for_source(source, source.read_bytes())
        window = editor.window
        select_element(window, "Rectangle")
        for label in [
            "All corner radii",
            "Shadow distance value",
            "Shadow blur value",
            "Shadow spread value",
        ]:
            field = inspector_field(window, label, "text-input")
            texts = (
                field.query_descendants()
                .match_accessible_role(slint_testing.AccessibleRole.Text)
                .find_all()
            )
            assert not any(text.accessible_label == "px" for text in texts)
        pane = window.get_by_accessible_name("Inspector and outline").resolve()
        texts = (
            pane.query_descendants()
            .match_accessible_role(slint_testing.AccessibleRole.Text)
            .find_all()
        )
        assert not any(text.accessible_label == "px" for text in texts)


@pytest.mark.parametrize("family", ("drop", "inner"))
@pytest.mark.parametrize(
    ("control", "label", "value"),
    SHADOW_EDITS,
)
def test_shadow_control_writes_exact_source(
    editor_factory,
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

    with editor_factory(source_file) as editor:
        wait_for_source(source_file, starting_source)
        window = editor.window
        select_element(window, "Rectangle")
        edit_field(window, label, value)
        snapshot.wait_for_applied(
            shadow_expected(starting_source, family, control, value),
            relative_path=INSPECTOR_SOURCE,
        )
