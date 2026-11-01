# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore FAFC tobytes

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
from source_snapshot import SourceSnapshot, replace_once, wait_for_source_change
from ui_assertions import expect
from ui_driver import (
    element,
    elements,
    first_window,
    launch_editor,
    outline_rows,
    press_keys,
    press_shortcut,
    query,
    screenshot,
    select_outline_row,
    wait_until,
)

INSPECTOR_SOURCE = "InspectorCases.slint"
ELEMENT_ROWS = {
    "Rectangle": "inspect-rectangle",
    "Text": "inspect-text",
    "Image": "inspect-image",
    "StyledText": "inspect-styled-text",
    "Button": "inspect-button",
    "CheckBox": "inspect-checkbox",
}


def select_element(window: slint_testing.Window, kind: str) -> None:
    select_outline_row(window, ELEMENT_ROWS[kind])
    element(window, f"Selected {kind}", role=slint_testing.AccessibleRole.Region)


def open_combo_and_accept(
    window: slint_testing.Window,
    label: str,
    expected_options: tuple[str, ...],
    value: str,
) -> None:
    combo = inspector_field(window, label, slint_testing.AccessibleRole.Combobox)
    combo.invoke_accessible_expand_action()

    def menu_labels() -> tuple[str, ...] | None:
        # Linux uses list-item rows in a custom popup; macOS and Windows use MenuItem.
        items = (
            window.root_element.query_descendants()
            .match_type_name("MenuItem")
            .find_all()
        ) + (
            window.root_element.query_descendants()
            .match_id("InspectorComboBox::options-content")
            .match_descendants()
            .match_accessible_role(slint_testing.AccessibleRole.ListItem)
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
    wait_until(lambda: next(iter(elements(window, id=element_id)), None))


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
        rectangle = element(window, id="InspectorCases::inspect-rectangle")

        def rendered_value() -> float:
            return getattr(rectangle.absolute_rect, property_name)

        expected = float(value)
        if property_name in ("x", "y"):
            # Absolute positions include the preview's offset in the editor window.
            preview_offset = rendered_value() - original_value
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
        wait_until(lambda: rendered_value() == pytest.approx(expected))


@pytest.mark.parametrize(
    ("property_name", "initial", "old", "new"),
    [
        ("x", 32, b"        x: 32px;", b"        x: 44px;"),
        ("y", 32, b"        y: 32px;", b"        y: 44px;"),
        ("width", 160, b"        width: 160px;", b"        width: 172px;"),
        (
            "height",
            96,
            b"        width: 160px;\n        height: 96px;",
            b"        width: 160px;\n        height: 108px;",
        ),
    ],
    ids=("x", "y", "width", "height"),
)
def test_geometry_prefix_scrubs_with_transient_preview(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    property_name: str,
    initial: int,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Rectangle")
        label = FIELDS[property_name]
        scrubber = inspector_field(
            window, label + " scrubber", slint_testing.AccessibleRole.Slider
        )
        start = element_center(scrubber)
        end = slint_testing.LogicalPosition(x=start.x + 12, y=start.y)
        button = slint_testing.PointerEventButton.Left

        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(
            window,
            label,
            str(initial + 12),
            slint_testing.AccessibleRole.TextInput,
        )
        snapshot.assert_unchanged()

        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        snapshot.wait_for_applied(expected, relative_path=INSPECTOR_SOURCE)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=INSPECTOR_SOURCE)


def test_geometry_scrub_reverts_when_commit_is_rejected(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    background_edit = replace_once(
        baseline,
        b"        background: #2563eb;",
        b"        background: #123456;",
    )
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Rectangle")
        scrubber = inspector_field(
            window, "Position X scrubber", slint_testing.AccessibleRole.Slider
        )
        start = element_center(scrubber)
        end = slint_testing.LogicalPosition(x=start.x + 12, y=start.y)
        button = slint_testing.PointerEventButton.Left

        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(window, "Position X", "44")
        snapshot.assert_unchanged_now()

        edit_field(
            window,
            "Rectangle background",
            "#123456",
            slint_testing.AccessibleRole.TextInput,
        )
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))

        wait_for_field(window, "Position X", "32")
        snapshot.wait_for_applied(background_edit, relative_path=INSPECTOR_SOURCE)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=INSPECTOR_SOURCE)


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
        outline = element(window, "Current file outline")
        root_row = (
            outline.query_descendants()
            .match_accessible_role(slint_testing.AccessibleRole.ListItem)
            .find_all()[0]
        )
        root_row.invoke_accessible_default_action()
        wait_for_field(
            window, "Root background", "F8FAFC", slint_testing.AccessibleRole.TextInput
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
            "ABCDEF",
            slint_testing.AccessibleRole.TextInput,
        )


def test_literal_color_field_separates_rgb_and_opacity(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    source_file.write_bytes(
        replace_once(
            source_file.read_bytes(),
            b"        background: #2563eb;",
            b"        background: #1a2dac4d;",
        )
    )

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Rectangle")
        wait_for_field(
            window,
            "Rectangle background",
            "1A2DAC",
            slint_testing.AccessibleRole.TextInput,
        )
        wait_for_field(
            window,
            "Rectangle background opacity",
            "30",
            slint_testing.AccessibleRole.TextInput,
        )
        rgb = inspector_field(
            window,
            "Rectangle background",
            slint_testing.AccessibleRole.TextInput,
        )
        opacity = inspector_field(
            window,
            "Rectangle background opacity",
            slint_testing.AccessibleRole.TextInput,
        )
        fill_labels = elements(window, "Fill", role=slint_testing.AccessibleRole.Text)
        assert any(
            label.absolute_position.y < rgb.absolute_position.y for label in fill_labels
        )
        assert (
            opacity.absolute_position.x + opacity.size.width - rgb.absolute_position.x
            > 190
        )
        picker = inspector_field(
            window,
            "Rectangle background color picker",
            slint_testing.AccessibleRole.Button,
        )
        rendered = screenshot(window)
        center_x = picker.absolute_position.x + picker.size.width / 2
        center_y = picker.absolute_position.y + picker.size.height / 2
        opaque = rendered.getpixel((round(center_x - 4), round(center_y)))
        transparent = {
            rendered.getpixel((round(center_x + x), round(center_y + y)))
            for x in (2, 5)
            for y in (-5, -2, 2, 5)
        }
        assert opaque not in transparent
        assert len(transparent) >= 2

        middle_y = round(rgb.absolute_position.y + rgb.size.height / 2)
        left_edge = (round(rgb.absolute_position.x), middle_y)
        divider = (round(opacity.absolute_position.x), middle_y)
        right_edge = (
            round(opacity.absolute_position.x + opacity.size.width - 1),
            middle_y,
        )

        rgb.invoke_accessible_default_action()
        rgb_focused = screenshot(window)
        assert rgb_focused.getpixel(left_edge) != rendered.getpixel(left_edge)
        assert rgb_focused.getpixel(right_edge) != rendered.getpixel(right_edge)
        assert rgb_focused.getpixel(divider) == rendered.getpixel(divider)

        opacity.invoke_accessible_default_action()
        opacity_focused = screenshot(window)
        assert opacity_focused.getpixel(left_edge) != rendered.getpixel(left_edge)
        assert opacity_focused.getpixel(divider) == rendered.getpixel(divider)
        assert opacity_focused.getpixel(right_edge) != rendered.getpixel(right_edge)


@pytest.mark.parametrize(
    ("property_name", "label", "rgb"),
    [
        ("background", "Rectangle background", "#ABCDEF"),
        ("border-color", "Border color", "ABCDEF"),
    ],
)
def test_color_field_preserves_or_replaces_alpha(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    property_name: str,
    label: str,
    rgb: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    original = f"        {property_name}: #1a2dac4d;".encode()
    baseline = replace_once(
        source_file.read_bytes(),
        b"        background: #2563eb;",
        original
        if property_name == "background"
        else b"        background: #2563eb;\n" + original,
    )
    source_file.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Rectangle")
        edit_field(
            window,
            label,
            rgb,
            slint_testing.AccessibleRole.TextInput,
        )
        rgb_edit = replace_once(
            baseline,
            original,
            f"        {property_name}: #abcdef4d;".encode(),
        )
        snapshot.wait_for_applied(rgb_edit, relative_path=INSPECTOR_SOURCE)
        wait_for_field(
            window,
            label,
            "ABCDEF",
            slint_testing.AccessibleRole.TextInput,
        )
        wait_for_field(
            window,
            label + " opacity",
            "30",
            slint_testing.AccessibleRole.TextInput,
        )

        edit_field(
            window,
            label,
            "#12345680",
            slint_testing.AccessibleRole.TextInput,
        )
        alpha_edit = replace_once(
            baseline,
            original,
            f"        {property_name}: #12345680;".encode(),
        )
        snapshot.wait_for_applied(alpha_edit, relative_path=INSPECTOR_SOURCE)
        wait_for_field(
            window,
            label,
            "123456",
            slint_testing.AccessibleRole.TextInput,
        )
        wait_for_field(
            window,
            label + " opacity",
            "50",
            slint_testing.AccessibleRole.TextInput,
        )

        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(rgb_edit, relative_path=INSPECTOR_SOURCE)


def test_color_field_opacity_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = replace_once(
        source_file.read_bytes(),
        b"        background: #2563eb;",
        b"        background: #1a2dac4d;",
    )
    source_file.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Rectangle")
        wait_for_field(
            window,
            "Rectangle background opacity",
            "30",
            slint_testing.AccessibleRole.TextInput,
        )
        opacity_field = inspector_field(
            window,
            "Rectangle background opacity",
            slint_testing.AccessibleRole.TextInput,
        )
        suffixes = (
            opacity_field.query_descendants()
            .match_id("EditorInput::fixed-suffix")
            .find_all()
        )
        assert len(suffixes) == 1
        suffix_position = suffixes[0].absolute_position.x

        for opacity, color in (
            ("0", b"#1a2dac00"),
            ("50", b"#1a2dac80"),
            ("100", b"#1a2dac"),
            ("120", b"#1a2dac"),
        ):
            edit_field(
                window,
                "Rectangle background opacity",
                opacity,
                slint_testing.AccessibleRole.TextInput,
            )
            changed = replace_once(
                baseline,
                b"        background: #1a2dac4d;",
                b"        background: " + color + b";",
            )
            snapshot.wait_for_applied(changed, relative_path=INSPECTOR_SOURCE)
            wait_for_field(
                window,
                "Rectangle background opacity",
                str(min(100, int(opacity))),
                slint_testing.AccessibleRole.TextInput,
            )
            assert suffixes[0].absolute_position.x == suffix_position
            press_shortcut(window, keys.Control, "z")
            snapshot.wait_for_applied(baseline, relative_path=INSPECTOR_SOURCE)
            wait_for_field(
                window,
                "Rectangle background opacity",
                "30",
                slint_testing.AccessibleRole.TextInput,
            )


def test_opacity_suffix_scrubs_with_transient_preview(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = replace_once(
        source_file.read_bytes(),
        b"        background: #2563eb;",
        b"        background: #1a2dac4d;",
    )
    source_file.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Rectangle")
        rectangle = elements(window, id="InspectorCases::inspect-rectangle")[0]
        sample = (
            round(rectangle.absolute_position.x + rectangle.size.width / 2),
            round(rectangle.absolute_position.y + rectangle.size.height / 2),
        )
        before = screenshot(window).getpixel(sample)
        scrubber = inspector_field(
            window,
            "Rectangle background opacity scrubber",
            slint_testing.AccessibleRole.Slider,
        )
        start = element_center(scrubber)
        end = slint_testing.LogicalPosition(x=start.x + 12, y=start.y)
        button = slint_testing.PointerEventButton.Left

        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(
            window,
            "Rectangle background opacity",
            "42",
            slint_testing.AccessibleRole.TextInput,
        )
        snapshot.assert_unchanged_now()
        assert screenshot(window).getpixel(sample) != before

        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        expected = replace_once(
            baseline,
            b"        background: #1a2dac4d;",
            b"        background: #1a2dac6b;",
        )
        snapshot.wait_for_applied(expected, relative_path=INSPECTOR_SOURCE)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=INSPECTOR_SOURCE)

        scrubber = inspector_field(
            window,
            "Rectangle background opacity scrubber",
            slint_testing.AccessibleRole.Slider,
        )
        start = element_center(scrubber)
        end = slint_testing.LogicalPosition(x=start.x + 12, y=start.y)
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(
            window,
            "Rectangle background opacity",
            "42",
            slint_testing.AccessibleRole.TextInput,
        )
        press_shortcut(window, keys.Escape)
        wait_for_field(
            window,
            "Rectangle background opacity",
            "30",
            slint_testing.AccessibleRole.TextInput,
        )
        snapshot.assert_unchanged()


def test_color_field_uses_resolved_and_symbolic_display_modes(
    editor_binary: Path,
    editor_environment: dict[str, str],
    tmp_path: Path,
) -> None:
    source_file = tmp_path / "ColorFieldModes.slint"
    source_file.write_text("""export component ColorFieldModes inherits Window {
    in property <color> accent: #1a2dac4d;
    linear := Rectangle {
        width: 200px;
        height: 200px;
        background: @linear-gradient(90deg, red 0%, blue 100%);
    }
    radial := Rectangle {
        width: 200px;
        height: 200px;
        background: @radial-gradient(circle, red 0%, blue 100%);
    }
    conic := Rectangle {
        width: 200px;
        height: 200px;
        background: @conic-gradient(red 0deg, blue 360deg);
    }
    symbolic := Rectangle {
        width: 200px;
        height: 200px;
        background: root.accent;
    }
}
""")

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        for element, label in (
            ("linear", "Linear Gradient"),
            ("radial", "Radial Gradient"),
            ("conic", "Conic Gradient"),
        ):
            select_outline_row(window, element)
            wait_until(lambda label=label: next(iter(elements(window, label)), None))
            assert not elements(
                window,
                "Rectangle background opacity",
                role=slint_testing.AccessibleRole.TextInput,
            )
            inspector_field(
                window,
                "Rectangle background color picker",
                slint_testing.AccessibleRole.Button,
            )

        select_outline_row(window, "symbolic")
        wait_for_field(
            window,
            "Rectangle background",
            "root.accent",
            slint_testing.AccessibleRole.TextInput,
        )
        assert not elements(
            window,
            "Rectangle background opacity",
            role=slint_testing.AccessibleRole.TextInput,
        )


def test_shared_color_fields_expose_opacity_editor(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        outline = element(window, "Current file outline")
        root_row = (
            outline.query_descendants()
            .match_accessible_role(slint_testing.AccessibleRole.ListItem)
            .find_all()[0]
        )
        root_row.invoke_accessible_default_action()
        wait_for_field(
            window,
            "Root background opacity",
            "100",
            slint_testing.AccessibleRole.TextInput,
        )

        select_element(window, "Rectangle")
        wait_for_field(
            window,
            "Rectangle background opacity",
            "100",
            slint_testing.AccessibleRole.TextInput,
        )
        wait_for_field(
            window,
            "Shadow color opacity",
            "25",
            slint_testing.AccessibleRole.TextInput,
        )
        shadow_color = inspector_field(
            window,
            "Shadow color",
            slint_testing.AccessibleRole.TextInput,
        )
        shadow_opacity = inspector_field(
            window,
            "Shadow color opacity",
            slint_testing.AccessibleRole.TextInput,
        )
        shadow_blur = inspector_field(
            window,
            "Shadow blur",
            slint_testing.AccessibleRole.Slider,
        )
        assert (
            shadow_opacity.absolute_position.x
            + shadow_opacity.size.width
            - shadow_color.absolute_position.x
            > 150
        )
        assert (
            shadow_color.absolute_position.y + shadow_color.size.height
            <= shadow_blur.absolute_position.y
        )

        select_element(window, "Text")
        wait_for_field(
            window,
            "Text color opacity",
            "100",
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
        if fit == "cover":
            open_combo_and_accept(
                window, "Image fit", ("fill", "preserve", "contain", "cover"), fit
            )
        else:
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


@pytest.mark.parametrize(
    ("initial", "rendering"), (("smooth", "pixelated"), ("pixelated", "smooth"))
)
def test_each_image_rendering_value_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    initial: str,
    rendering: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    starting_source = replace_once(
        source_file.read_bytes(),
        b"        image-fit: contain;",
        f"        image-fit: contain;\n        image-rendering: {initial};".encode(),
    )
    source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Image")
        edit_field(
            window, "Image rendering", rendering, slint_testing.AccessibleRole.Combobox
        )
        snapshot.wait_for_exact(
            replace_once(
                starting_source,
                f"        image-rendering: {initial};".encode(),
                f"        image-rendering: {rendering};".encode(),
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
        assert (
            element(
                window, "Image source", role=slint_testing.AccessibleRole.Text
            ).accessible_value
            == "checker.svg"
        )
        element(window, "Choose image file", role=slint_testing.AccessibleRole.Button)
        element(window, "Alignment", role=slint_testing.AccessibleRole.Text)
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
                element(window, display, role=slint_testing.AccessibleRole.Text)
                assert_rendered_element(window, "InspectorCases::inspect-image")
                if vertical == "top" and horizontal == "left":
                    press_shortcut(window, keys.Control, "z")
                    snapshot.wait_for_applied(baseline, relative_path=INSPECTOR_SOURCE)
                    assert image_alignment_button(
                        window, "center", "center"
                    ).accessible_checked
                    press_shortcut(window, keys.Control, keys.Shift, "z")
                    snapshot.wait_for_applied(expected, relative_path=INSPECTOR_SOURCE)
                    assert image_alignment_button(
                        window, vertical, horizontal
                    ).accessible_checked


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
        element(window, "Custom", role=slint_testing.AccessibleRole.Text)
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
        element(window, "Inspector text", role=slint_testing.AccessibleRole.Text)


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
        if weight == "700":
            open_combo_and_accept(
                window,
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
                weight,
            )
        else:
            edit_field(
                window, "Font weight", weight, slint_testing.AccessibleRole.Combobox
            )
        snapshot.wait_for_exact(
            replace_once(
                starting_source,
                f"        font-weight: {initial};".encode(),
                f"        font-weight: {weight};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        element(window, "Inspector text", role=slint_testing.AccessibleRole.Text)


def test_font_italic_switch_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    def italic_switch() -> slint_testing.Element:
        return inspector_field(window, "Italic", slint_testing.AccessibleRole.Switch)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_element(window, "Text")
        expect(italic_switch()).to_be_checked(False)

        italic_switch().invoke_accessible_default_action()
        italic = wait_for_source_change(source_file, baseline)
        assert italic == replace_once(
            baseline,
            b'        font-family: "Inter";',
            b'        font-family: "Inter";\n        font-italic: true;',
        )
        expect(italic_switch()).to_be_checked()

        italic_switch().invoke_accessible_default_action()
        upright = wait_for_source_change(source_file, italic)
        assert upright == replace_once(
            italic, b"        font-italic: true;", b"        font-italic: false;"
        )
        expect(italic_switch()).to_be_checked(False)

        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(italic, INSPECTOR_SOURCE)
        expect(italic_switch()).to_be_checked()


@pytest.mark.parametrize("wrap", ("no-wrap", "word-wrap", "char-wrap"))
def test_each_text_wrap_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    wrap: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    initial = "word-wrap" if wrap == "no-wrap" else "no-wrap"
    starting_source = replace_once(
        baseline,
        b"        font-size: 20px;",
        f"        font-size: 20px;\n        wrap: {initial};".encode(),
    )
    source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Text")
        if wrap == "char-wrap":
            open_combo_and_accept(
                window,
                "Text Wrap",
                ("No wrap", "Word wrap", "Character wrap"),
                wrap,
            )
        else:
            edit_field(window, "Text Wrap", wrap, slint_testing.AccessibleRole.Combobox)
        snapshot.wait_for_exact(
            replace_once(
                starting_source,
                f"        wrap: {initial};".encode(),
                f"        wrap: {wrap};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        element(window, "Inspector text", role=slint_testing.AccessibleRole.Text)


ORIENTATION_ROWS = {
    "Slider": "inspect-slider",
    "RadioGroup": "inspect-radio-group",
    "TabWidget": "inspect-tab-widget",
}
ORIENTATION_DEFAULTS = {
    "Slider": "Horizontal",
    "RadioGroup": "Vertical",
    "TabWidget": "Horizontal",
}


def with_orientation_widgets(
    baseline: bytes, kind: str, orientation: str | None = None
) -> bytes:
    bindings = {name: b"" for name in ORIENTATION_ROWS}
    if orientation:
        bindings[kind] = f"\n        orientation: {orientation};".encode()
    source = replace_once(
        baseline,
        b"export component InspectorCases",
        b'import { RadioGroup, Slider, TabWidget } from "std-widgets.slint";\n\n'
        b"export component InspectorCases",
    )
    widgets = (
        b"\n    inspect-slider := Slider {\n"
        b"        x: 32px;\n        y: 336px;\n        width: 160px;\n        height: 32px;"
        + bindings["Slider"]
        + b"\n    }\n\n"
        b"    inspect-radio-group := RadioGroup {\n"
        b"        x: 224px;\n        y: 336px;"
        + bindings["RadioGroup"]
        + b'\n        RadioButton { text: "One"; }'
        b'\n        RadioButton { text: "Two"; }'
        b"\n    }\n\n"
        b"    inspect-tab-widget := TabWidget {\n"
        b"        x: 400px;\n        y: 336px;\n        width: 200px;\n        height: 120px;"
        + bindings["TabWidget"]
        + b'\n        Tab { title: "First"; }'
        b'\n        Tab { title: "Second"; }'
        b"\n    }\n"
    )
    assert source.endswith(b"\n}\n")
    return source[:-2] + widgets + b"}\n"


def select_orientation_widget(window: slint_testing.Window, kind: str) -> None:
    select_outline_row(window, ORIENTATION_ROWS[kind])
    element(window, f"Selected {kind}", role=slint_testing.AccessibleRole.Region)


@pytest.mark.parametrize("kind", tuple(ORIENTATION_ROWS))
def test_orientation_shows_widget_default(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    source_file.write_bytes(with_orientation_widgets(source_file.read_bytes(), kind))

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_orientation_widget(window, kind)
        wait_for_field(
            window,
            "Orientation",
            ORIENTATION_DEFAULTS[kind],
            slint_testing.AccessibleRole.Combobox,
        )


@pytest.mark.parametrize("orientation", ("horizontal", "vertical"))
@pytest.mark.parametrize("kind", tuple(ORIENTATION_ROWS))
def test_each_orientation_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    orientation: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    initial = "vertical" if orientation == "horizontal" else "horizontal"
    starting_source = with_orientation_widgets(source_file.read_bytes(), kind, initial)
    source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_orientation_widget(window, kind)
        wait_for_field(
            window,
            "Orientation",
            initial.capitalize(),
            slint_testing.AccessibleRole.Combobox,
        )
        open_combo_and_accept(
            window, "Orientation", ("Horizontal", "Vertical"), orientation
        )
        snapshot.wait_for_exact(
            replace_once(
                starting_source,
                f"        orientation: {initial};".encode(),
                f"        orientation: {orientation};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        wait_for_field(
            window,
            "Orientation",
            orientation.capitalize(),
            slint_testing.AccessibleRole.Combobox,
        )


@pytest.mark.parametrize(
    ("case", "value", "expected"),
    [
        ("numeric", "24", "24px"),
        ("fractional", "24.5", "24.5px"),
        ("expression", "20px * 1.5", "20px * 1.5"),
    ],
    ids=("numeric", "fractional", "expression"),
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
        element(window, "Inspector text", role=slint_testing.AccessibleRole.Text)


@pytest.mark.parametrize("initial", ["literal", "draft", "expression"])
def test_font_size_stepper_uses_current_value_and_undo(
    editor_binary, editor_environment, fixture_project, initial
):
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    original = b"        font-size: 20px;"
    if initial == "expression":
        expression = b"        font-size: 20px * 1.5;"
        baseline = replace_once(baseline, original, expression)
        source_file.write_bytes(baseline)
        original = expression
    snapshot = SourceSnapshot.capture(fixture_project)
    expected_size = 21 if initial == "literal" else 31
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Text")
        field = inspector_field(
            window, "Font size", slint_testing.AccessibleRole.TextInput
        )
        section = element(window, id="InspectorTypographySection::root")
        element(section, "Size", role=slint_testing.AccessibleRole.Text)
        element(section, "Weight", role=slint_testing.AccessibleRole.Text)
        if initial == "draft":
            field.single_click(slint_testing.PointerEventButton.Left)
            press_keys(window, "30")
            snapshot.assert_unchanged()
        inspector_field(
            window, "Increase Font size", slint_testing.AccessibleRole.Button
        ).single_click(slint_testing.PointerEventButton.Left)
        expected = replace_once(
            baseline, original, f"        font-size: {expected_size}px;".encode()
        )
        snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
        wait_for_field(
            window,
            "Font size",
            str(expected_size),
            slint_testing.AccessibleRole.TextInput,
        )
        press_shortcut(window, keys.Escape)
        select_element(window, "Text")
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)


@pytest.mark.parametrize(
    ("initial", "key", "shift", "expected_size"),
    [
        (20, keys.UpArrow, False, 21),
        (20, keys.DownArrow, True, 10),
        (1, keys.DownArrow, False, 1),
    ],
)
def test_font_size_keyboard_steps_and_minimum(
    editor_binary,
    editor_environment,
    fixture_project,
    initial,
    key,
    shift,
    expected_size,
):
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes().replace(
        b"font-size: 20px;", f"font-size: {initial}px;".encode()
    )
    source_file.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Text")
        inspector_field(
            window, "Font size", slint_testing.AccessibleRole.TextInput
        ).single_click(slint_testing.PointerEventButton.Left)
        press_shortcut(window, *([keys.Shift, key] if shift else [key]))
        expected = baseline.replace(
            f"font-size: {initial}px;".encode(),
            f"font-size: {expected_size}px;".encode(),
        )
        snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
        wait_for_field(
            window,
            "Font size",
            str(expected_size),
            slint_testing.AccessibleRole.TextInput,
        )


@pytest.mark.parametrize("outcome", ["commit", "escape", "selection", "source"])
def test_font_size_scrubs_with_transient_preview(
    editor_binary, editor_environment, fixture_project, outcome
):
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Text")
        scrubber = inspector_field(
            window, "Font size scrubber", slint_testing.AccessibleRole.Slider
        )
        start = element_center(scrubber)
        end = slint_testing.LogicalPosition(x=start.x + 12, y=start.y)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(
            window, "Font size", "32", slint_testing.AccessibleRole.TextInput
        )
        snapshot.assert_unchanged()
        if outcome == "escape":
            press_keys(window, keys.Escape)
        elif outcome == "selection":
            select_element(window, "Rectangle")
        elif outcome == "source":
            source_file.write_bytes(baseline + b"\n// External edit\n")
            snapshot.wait_for_applied(
                baseline + b"\n// External edit\n", INSPECTOR_SOURCE
            )
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        if outcome == "commit":
            expected = replace_once(
                baseline, b"        font-size: 20px;", b"        font-size: 32px;"
            )
            snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
            press_shortcut(window, keys.Control, "z")
            snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)
        elif outcome != "source":
            snapshot.assert_unchanged()
            if outcome == "escape":
                wait_for_field(
                    window, "Font size", "20", slint_testing.AccessibleRole.TextInput
                )


@pytest.mark.parametrize(
    ("case", "value", "expected_line", "rendered"),
    [
        ("literal", "Literal content", '"Literal content"', "Literal content"),
        (
            "escaped",
            'A "quote" and a \\ path',
            '"A \\"quote\\" and a \\\\ path"',
            'A "quote" and a \\ path',
        ),
        (
            "expression",
            '"Expression " + "content"',
            '"Expression " + "content"',
            "Expression content",
        ),
    ],
    ids=("literal", "escaped", "expression"),
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
    original = b'        text: "Inspector text";'
    if case == "expression":
        original_expression = b'        text: "Inspector " + "text";'
        baseline = replace_once(baseline, original, original_expression)
        source_file.write_bytes(baseline)
        original = original_expression
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
                original,
                f"        text: {expected_line};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        element(window, rendered, role=slint_testing.AccessibleRole.Text)


@pytest.mark.parametrize(
    ("kind", "original", "rendered", "role"),
    [
        (
            "Button",
            "Inspector button",
            "Button content",
            slint_testing.AccessibleRole.Button,
        ),
        (
            "CheckBox",
            "Inspector checkbox",
            "CheckBox content",
            slint_testing.AccessibleRole.Checkbox,
        ),
    ],
    ids=("button", "checkbox"),
)
def test_widget_text_content_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    original: str,
    rendered: str,
    role: slint_testing.AccessibleRole,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, kind)
        wait_for_field(
            window,
            "Text content",
            original,
            slint_testing.AccessibleRole.TextInput,
        )
        edit_field(
            window,
            "Text content",
            rendered,
            slint_testing.AccessibleRole.TextInput,
        )
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                f'        text: "{original}";'.encode(),
                f'        text: "{rendered}";'.encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )
        element(window, rendered, role=role)


def test_tooltip_text_content_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_outline_row(window, "inspect-tooltip")
        wait_for_field(
            window,
            "Text content",
            '@markdown("Inspector tooltip")',
            slint_testing.AccessibleRole.TextInput,
        )
        assert not elements(
            window,
            "Horizontal Alignment",
            role=slint_testing.AccessibleRole.Combobox,
        )
        edit_field(
            window,
            "Text content",
            '@markdown("**Tooltip** content")',
            slint_testing.AccessibleRole.TextInput,
        )
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                b'            text: @markdown("Inspector tooltip");',
                b'            text: @markdown("**Tooltip** content");',
            ),
            relative_path=INSPECTOR_SOURCE,
        )


def test_invalid_text_content_does_not_change_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    source_file.write_bytes(
        replace_once(
            source_file.read_bytes(),
            b'        text: "Inspector text";',
            b'        text: "Inspector " + "text";',
        )
    )
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
            '"Inspector " + "text"',
            slint_testing.AccessibleRole.TextInput,
        )
        element(window, "Inspector text", role=slint_testing.AccessibleRole.Text)


@pytest.mark.parametrize(
    "tail", ["many words " * 35, "abcdefgh" * 45], ids=["words", "long-word"]
)
@pytest.mark.parametrize("outcome", ["commit", "escape"])
def test_inline_text_wraps_without_growing_and_previews_live(
    editor_binary, editor_environment, fixture_project, tail, outcome
):
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Text")
        field = inspector_field(
            window, "Text content", slint_testing.AccessibleRole.TextInput
        )
        expand = element(window, "Edit text", role=slint_testing.AccessibleRole.Button)
        clear = element(field, "Clear text", role=slint_testing.AccessibleRole.Button)
        before = field.absolute_rect
        expand_before = expand.absolute_rect
        assert before.height >= 2 * expand_before.height
        field.single_click(slint_testing.PointerEventButton.Left)
        press_keys(window, "First line")
        press_keys(window, keys.Return)
        press_keys(window, tail)
        text = "First line\n" + tail
        expect(field).to_have_value(text)
        element(window, text, role=slint_testing.AccessibleRole.Text)
        snapshot.assert_unchanged()
        assert field.absolute_rect == before
        assert expand.absolute_rect == expand_before
        pane = element(
            window,
            "Inspector and outline",
            role=slint_testing.AccessibleRole.Complementary,
        ).absolute_rect
        assert clear.absolute_rect.x + clear.size.width <= before.x + before.width
        assert expand.absolute_rect.x + expand.size.width <= pane.x + pane.width
        input = element(field, id="EditorInput::input")
        viewport = element(field, id="EditorInput::viewport")
        assert input.absolute_position.y < viewport.absolute_position.y
        if outcome == "commit":
            press_keys(window, keys.Tab)
            expected = replace_once(
                baseline,
                b'text: "Inspector text";',
                ('text: "First line\\n' + tail + '";').encode(),
            )
            snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
            select_element(window, "Text")
            press_shortcut(window, keys.Control, "z")
            snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)
        else:
            press_keys(window, keys.Escape)
            expect(field).to_have_value("Inspector text")
            element(window, "Inspector text", role=slint_testing.AccessibleRole.Text)
            snapshot.assert_unchanged()


@pytest.mark.parametrize(
    "outcome", ["apply", "cancel", "escape", "selection", "source"]
)
def test_expanded_text_edit(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    outcome: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Text")
        text = inspector_field(
            window, "Text content", slint_testing.AccessibleRole.TextInput
        )
        color = inspector_field(
            window, "Text color", slint_testing.AccessibleRole.TextInput
        )
        assert text.absolute_position.y < color.absolute_position.y
        section = element(window, "Text", role=slint_testing.AccessibleRole.Groupbox)
        element(section, "Text content", role=slint_testing.AccessibleRole.TextInput)
        element(section, "Text color", role=slint_testing.AccessibleRole.TextInput)
        assert not elements(
            window, "APPEARANCE", role=slint_testing.AccessibleRole.Text
        )
        inspector_field(
            window, "Edit text", slint_testing.AccessibleRole.Button
        ).single_click(slint_testing.PointerEventButton.Left)
        expanded = element(
            window, "Expanded text", role=slint_testing.AccessibleRole.TextInput
        )
        expect(expanded).to_have_value("Inspector text")
        press_shortcut(window, keys.Control, "a")
        press_keys(window, 'Line one "quoted"')
        press_keys(window, keys.Return)
        press_keys(window, "C:\\notes")
        expect(expanded).to_have_value('Line one "quoted"\nC:\\notes')
        element(
            window,
            'Line one "quoted"\nC:\\notes',
            role=slint_testing.AccessibleRole.Text,
        )
        snapshot.assert_unchanged()

        if outcome == "apply":
            element(
                window, "Apply", role=slint_testing.AccessibleRole.Button
            ).invoke_accessible_default_action()
            expected = replace_once(
                baseline,
                b'        text: "Inspector text";',
                rb'        text: "Line one \"quoted\"\nC:\\notes";',
            )
            snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
            press_shortcut(window, keys.Control, "z")
            snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)
            press_shortcut(window, keys.Control, keys.Shift, "z")
            snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
        elif outcome == "cancel":
            element(
                window, "Cancel", role=slint_testing.AccessibleRole.Button
            ).invoke_accessible_default_action()
        elif outcome == "escape":
            press_keys(window, keys.Escape)
        elif outcome == "selection":
            select_element(window, "Rectangle")
        else:
            source_file.write_bytes(baseline + b"\n// External edit\n")
            snapshot.wait_for_applied(
                baseline + b"\n// External edit\n", INSPECTOR_SOURCE
            )

        expect(query(window, "Expanded text")).to_be_hidden()
        if outcome not in ("apply", "source"):
            snapshot.assert_unchanged()
            element(window, "Inspector text", role=slint_testing.AccessibleRole.Text)


@pytest.mark.parametrize("apply", [True, False], ids=["apply", "cancel"])
def test_expanding_text_transfers_inline_draft(
    editor_binary, editor_environment, fixture_project, apply
):
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "Text")
        inspector_field(
            window, "Text content", slint_testing.AccessibleRole.TextInput
        ).single_click(slint_testing.PointerEventButton.Left)
        press_keys(window, "Pending text")
        inspector_field(
            window, "Edit text", slint_testing.AccessibleRole.Button
        ).single_click(slint_testing.PointerEventButton.Left)
        expect(
            element(
                window, "Expanded text", role=slint_testing.AccessibleRole.TextInput
            )
        ).to_have_value("Pending text")
        snapshot.assert_unchanged()
        element(
            window,
            "Apply" if apply else "Cancel",
            role=slint_testing.AccessibleRole.Button,
        ).invoke_accessible_default_action()
        expect(query(window, "Expanded text")).to_be_hidden()
        if apply:
            snapshot.wait_for_applied(
                replace_once(
                    baseline, b'text: "Inspector text";', b'text: "Pending text";'
                ),
                INSPECTOR_SOURCE,
            )
        else:
            snapshot.assert_unchanged()


def test_window_title_uses_plain_text_without_expand_button(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = replace_once(
        source_file.read_bytes(),
        b"    background: #f8fafc;",
        b'    background: #f8fafc;\n    title: "Inspector title";',
    )
    source_file.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        outline_rows(window)[0].invoke_accessible_default_action()
        wait_for_field(
            window, "Title", "Inspector title", slint_testing.AccessibleRole.TextInput
        )
        assert not elements(
            window, "Edit text", role=slint_testing.AccessibleRole.Button
        )
        title = inspector_field(window, "Title", slint_testing.AccessibleRole.TextInput)
        fill = inspector_field(
            window, "Root background", slint_testing.AccessibleRole.TextInput
        )
        assert title.absolute_position.y < fill.absolute_position.y
        section = element(window, "Title", role=slint_testing.AccessibleRole.Groupbox)
        element(section, "Root background", role=slint_testing.AccessibleRole.TextInput)
        assert not elements(
            window, "APPEARANCE", role=slint_testing.AccessibleRole.Text
        )
        edit_field(
            window, "Title", 'My "window"', slint_testing.AccessibleRole.TextInput
        )
        snapshot.wait_for_applied(
            replace_once(
                baseline,
                b'    title: "Inspector title";',
                rb'    title: "My \"window\"";',
            ),
            INSPECTOR_SOURCE,
        )


@pytest.mark.parametrize("kind", ["Text", "Window"])
def test_string_field_clear_is_undoable(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    if kind == "Window":
        baseline = replace_once(
            baseline,
            b"    background: #f8fafc;",
            b'    background: #f8fafc;\n    title: "Inspector title";',
        )
        source_file.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    label = "Text content" if kind == "Text" else "Title"
    clear_label = "Clear text" if kind == "Text" else "Clear title"
    old = (
        b'        text: "Inspector text";'
        if kind == "Text"
        else b'    title: "Inspector title";'
    )
    new = b'        text: "";' if kind == "Text" else b'    title: "";'
    expected = replace_once(baseline, old, new)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        if kind == "Text":
            select_element(window, "Text")
        else:
            outline_rows(window)[0].invoke_accessible_default_action()
        field = inspector_field(window, label, slint_testing.AccessibleRole.TextInput)
        original_text = "Inspector text" if kind == "Text" else "Inspector title"
        field.single_click(slint_testing.PointerEventButton.Left)
        press_keys(window, keys.RightArrow)
        press_keys(window, " draft")
        expect(field).to_have_value(original_text + " draft")
        snapshot.assert_unchanged()
        press_shortcut(window, keys.Control, "z")
        expect(field).to_have_value(original_text)
        snapshot.assert_unchanged()
        press_shortcut(window, keys.Control, keys.Shift, "z")
        expect(field).to_have_value(original_text + " draft")
        inspector_field(
            window, clear_label, slint_testing.AccessibleRole.Button
        ).single_click(slint_testing.PointerEventButton.Left)
        snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
        wait_for_field(window, label, "", slint_testing.AccessibleRole.TextInput)
        expect(
            query(window, clear_label, role=slint_testing.AccessibleRole.Button)
        ).to_be_hidden()
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)


@pytest.mark.parametrize(
    ("label", "property_name", "initial", "value"),
    [
        ("Horizontal Alignment", "horizontal-alignment", "start", "end"),
        ("Vertical Alignment", "vertical-alignment", "top", "bottom"),
    ],
    ids=("horizontal", "vertical"),
)
def test_styled_text_alignment_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    label: str,
    property_name: str,
    initial: str,
    value: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_element(window, "StyledText")
        wait_for_field(
            window, FIELDS["width"], "152", slint_testing.AccessibleRole.TextInput
        )
        inspector_field(window, "Text content", slint_testing.AccessibleRole.TextInput)
        edit_field(window, label, value, slint_testing.AccessibleRole.Combobox)
        snapshot.wait_for_exact(
            replace_once(
                baseline,
                f"        {property_name}: {initial};".encode(),
                f"        {property_name}: {value};".encode(),
            ),
            relative_path=INSPECTOR_SOURCE,
        )


def test_styled_text_content_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / "Palette.slint"
    source = replace_once(
        source_file.read_bytes(),
        b"    background: #f8fafc;\n",
        b"    background: #f8fafc;\n"
        b"    styled := StyledText {\n"
        b"        x: 32px;\n"
        b"        y: 32px;\n"
        b"        width: 200px;\n"
        b"        height: 48px;\n"
        b"    }\n",
    )
    source_file.write_bytes(source)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source)
        window = first_window(editor)
        select_outline_row(window, "styled")
        element(window, "Selected StyledText", role=slint_testing.AccessibleRole.Region)
        wait_for_field(
            window,
            "Text content",
            '@markdown("")',
            slint_testing.AccessibleRole.TextInput,
        )
        edit_field(
            window,
            "Text content",
            '@markdown("**Bold**")',
            slint_testing.AccessibleRole.TextInput,
        )
        snapshot.wait_for_exact(
            replace_once(
                source,
                b"        y: 32px;\n",
                b'        y: 32px;\n        text: @markdown("**Bold**");\n',
            ),
            relative_path="Palette.slint",
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
            "Shadow blur value",
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


SHADOW_EDITS = (
    pytest.param("color", "Shadow color", "#12345678", id="color"),
    pytest.param("blur", "Shadow blur", "24", id="blur"),
    pytest.param("blur", "Shadow blur", "0", id="blur-0"),
    pytest.param("blur", "Shadow blur", "128", id="blur-128"),
)


def effect_source(baseline: bytes, family: str) -> bytes:
    if family == "backdrop":
        return replace_once(
            replace_once(baseline, b"background: #2563eb;", b"background: #2563eb40;"),
            b"    inspect-rectangle := Rectangle {",
            b'    Image { source: @image-url("assets/checker.svg"); width: 220px; height: 160px; image-fit: fill; }\n'
            b"    inspect-rectangle := Rectangle {\n        backdrop-blur: 16px;",
        )
    return (
        baseline
        if family == "drop"
        else baseline.replace(b"drop-shadow-", b"inner-shadow-")
    )


def effect_expected(source: bytes, family: str, control: str, value: str) -> bytes:
    prefix = (
        b"        backdrop-"
        if family == "backdrop"
        else f"        {family}-shadow-".encode()
    )
    if control == "color":
        return replace_once(
            source,
            prefix + b"color: #00000040;",
            prefix + f"color: {value};".encode(),
        )
    if control == "blur":
        return replace_once(
            source,
            prefix + b"blur: 16px;",
            prefix + f"blur: {value}px;".encode(),
        )
    raise ValueError(f"Unknown effect control: {control}")


def artboard_pixels(window: slint_testing.Window) -> bytes:
    artboard = element(window, "Artboard")
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


@pytest.mark.parametrize("blur", ["0", "16"])
def test_backdrop_blur_toggle_preserves_shadows(
    editor_binary, editor_environment, fixture_project, blur
):
    source = fixture_project / INSPECTOR_SOURCE
    baseline = replace_once(
        effect_source(source.read_bytes(), "backdrop"),
        b"        drop-shadow-spread: 0px;",
        b"        drop-shadow-spread: 0px;\n        inner-shadow-color: #00000030;",
    )
    baseline = effect_expected(baseline, "backdrop", "blur", blur)
    source.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_element(window, "Rectangle")
        inspector_field(
            window, "Drop shadow", slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        expect(query(window, "Close Drop shadow")).to_be_visible()
        inspector_field(
            window, "Enable backdrop blur", slint_testing.AccessibleRole.Switch
        ).invoke_accessible_default_action()
        expect(query(window, "Close Drop shadow")).to_be_hidden()
        disabled = wait_for_source_change(source, baseline)
        assert disabled == replace_once(
            baseline, f"        backdrop-blur: {blur}px;\n".encode(), b""
        )
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(disabled, INSPECTOR_SOURCE)
        inspector_field(
            window, "Enable backdrop blur", slint_testing.AccessibleRole.Switch
        ).invoke_accessible_default_action()
        enabled = wait_for_source_change(source, disabled)
        assert b"backdrop-blur: 16px;" in enabled
        assert replace_once(enabled, b"        backdrop-blur: 16px;\n", b"") == disabled


@pytest.mark.parametrize(
    ("family", "control", "label", "value"),
    [
        pytest.param(family, *case.values, id=f"{family}-{case.id}")
        for family in ("drop", "inner")
        for case in SHADOW_EDITS
    ]
    + [
        pytest.param(
            "backdrop",
            "blur",
            "Backdrop blur amount value",
            value,
            id=f"backdrop-blur-{value}",
        )
        for value in ("0", "24", "128", "200")
    ],
)
def test_effect_control_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    family: str,
    control: str,
    label: str,
    value: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    starting_source = effect_source(source_file.read_bytes(), family)
    source_file.write_bytes(starting_source)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, starting_source)
        window = first_window(editor)
        select_element(window, "Rectangle")
        edit_field(window, label, value)
        snapshot.wait_for_applied(
            effect_expected(starting_source, family, control, value),
            relative_path=INSPECTOR_SOURCE,
        )
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(starting_source, relative_path=INSPECTOR_SOURCE)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(
            effect_expected(starting_source, family, control, value),
            relative_path=INSPECTOR_SOURCE,
        )


@pytest.mark.parametrize(
    ("binding", "displayed", "key", "value"),
    [
        ("128px", "128", keys.RightArrow, "129"),
        ("200px", "200", keys.RightArrow, "201"),
        ("200px", "200", keys.LeftArrow, "199"),
        ("root.glass-blur", "36", keys.RightArrow, "37"),
    ],
)
def test_backdrop_blur_keyboard_uses_resolved_value(
    editor_binary, editor_environment, fixture_project, binding, displayed, key, value
):
    source = fixture_project / INSPECTOR_SOURCE
    baseline = replace_once(
        effect_source(source.read_bytes(), "backdrop"),
        b"    width: 640px;",
        b"    property <length> glass-blur: 36px;\n    width: 640px;",
    )
    baseline = replace_once(
        baseline, b"backdrop-blur: 16px;", f"backdrop-blur: {binding};".encode()
    )
    source.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_element(window, "Rectangle")
        label = "Backdrop blur amount"
        slider = inspector_field(window, label, slint_testing.AccessibleRole.Slider)
        expect(slider).to_have_value(displayed)
        position = slider_position(window, label, min(float(displayed) / 128, 1))
        window.dispatch_event(slint_testing.PointerMoveEvent(position))
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                position, slint_testing.PointerEventButton.Left
            )
        )
        window.dispatch_event(
            slint_testing.PointerReleaseEvent(
                position, slint_testing.PointerEventButton.Left
            )
        )
        snapshot.assert_unchanged()
        press_keys(window, key)
        expected = replace_once(
            baseline,
            f"backdrop-blur: {binding};".encode(),
            f"backdrop-blur: {value}px;".encode(),
        )
        snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)


@pytest.mark.parametrize("family", ["drop", "inner", "backdrop"])
@pytest.mark.parametrize("outcome", ["commit", "cancel", "selection", "source"])
def test_effect_slider_previews_without_source_writes(
    editor_binary, editor_environment, fixture_project, family, outcome
):
    source = fixture_project / INSPECTOR_SOURCE
    baseline = effect_source(source.read_bytes(), family)
    label = "Backdrop blur amount" if family == "backdrop" else "Shadow blur"
    source.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, baseline)
        window = first_window(editor)
        select_element(window, "Rectangle")
        window.dispatch_event(
            slint_testing.PointerMoveEvent(slint_testing.LogicalPosition(0, 0))
        )
        before = artboard_pixels(window)
        start = slider_position(window, label, 16 / 128)
        end = slider_position(window, label, 0.5)
        window.dispatch_event(slint_testing.PointerMoveEvent(start))
        preview_baseline = artboard_pixels(window)
        window.dispatch_event(
            slint_testing.PointerPressEvent(
                start, slint_testing.PointerEventButton.Left
            )
        )
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_for_field(window, f"{label} value", "64")
        assert artboard_pixels(window) != preview_baseline
        snapshot.assert_unchanged()
        if outcome == "cancel":
            press_keys(window, keys.Escape)
            wait_for_field(window, f"{label} value", "16")
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
            if family != "backdrop":
                close_buttons = elements(
                    window,
                    f"Close {family.capitalize()} shadow",
                    role=slint_testing.AccessibleRole.Button,
                )
                if close_buttons:
                    close_buttons[0].invoke_accessible_default_action()
            window.dispatch_event(
                slint_testing.PointerMoveEvent(slint_testing.LogicalPosition(0, 0))
            )
            assert artboard_pixels(window) == before
        else:
            expected = effect_expected(baseline, family, "blur", "64")
            snapshot.wait_for_applied(expected, INSPECTOR_SOURCE)
            press_shortcut(window, keys.Control, "z")
            snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)
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
        role = (
            slint_testing.AccessibleRole.Combobox
            if label == "Image fit"
            else slint_testing.AccessibleRole.TextInput
        )
        field = inspector_field(window, label, role)
        value_before = field.accessible_value
        edit_field(window, label, value, role)
        snapshot.assert_unchanged()
        wait_for_field(window, label, value_before, role)
        element(window, f"Selected {kind}", role=slint_testing.AccessibleRole.Region)


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
        edit_field(
            window,
            "Rectangle background opacity",
            "not-a-number",
            slint_testing.AccessibleRole.TextInput,
        )
        snapshot.assert_unchanged()
        wait_for_field(
            window,
            "Rectangle background",
            "2563EB",
            slint_testing.AccessibleRole.TextInput,
        )
        wait_for_field(
            window,
            "Rectangle background opacity",
            "100",
            slint_testing.AccessibleRole.TextInput,
        )
        element(window, "Selected Rectangle", role=slint_testing.AccessibleRole.Region)
