# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import (
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
    press_keys,
    press_shortcut,
    select_outline_row,
)

INSPECTOR_SOURCE = "InspectorCases.slint"
ROWS = {"Rectangle": "inspect-rectangle", "Text": "inspect-text"}
ANCHORS = {
    "Rectangle": b"        drop-shadow-spread: 0px;\n",
    "Text": b"        font-size: 20px;\n",
}


def select_element(window: slint_testing.Window, kind: str) -> None:
    select_outline_row(window, ROWS[kind])
    element(window, f"Selected {kind}", role=slint_testing.AccessibleRole.Region)


def with_opacity(source: bytes, kind: str, value: str) -> bytes:
    return replace_once(
        source, ANCHORS[kind], ANCHORS[kind] + f"        opacity: {value};\n".encode()
    )


@pytest.mark.parametrize("kind", ["Rectangle", "Text"])
def test_opacity_field_adds_fractional_binding(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = source_file.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_element(window, kind)
        expect(
            inspector_field(window, "Opacity", slint_testing.AccessibleRole.Slider)
        ).to_have_value("100")
        edit_field(window, "Opacity value", "50")
        changed = wait_for_source_change(source_file, baseline)
        assert changed.count(b"opacity: 0.5;") == 1
        assert (
            changed.replace(b"opacity: 0.5;", b"").replace(b"\n        \n", b"\n")
            == baseline
        )
        wait_for_field(window, "Opacity", "50", slint_testing.AccessibleRole.Slider)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)
        press_shortcut(window, keys.Control, keys.Shift, "z")
        snapshot.wait_for_applied(changed, INSPECTOR_SOURCE)


@pytest.mark.parametrize(
    ("binding", "displayed", "value", "expected"),
    [
        pytest.param("1", "100", "25", "0.25", id="integer-literal"),
        pytest.param("0.75", "75", "0", "0", id="to-transparent"),
        pytest.param("0", "0", "100", "1", id="to-opaque"),
        pytest.param("0.5", "50", "150", "1", id="clamped"),
    ],
)
def test_opacity_field_replaces_existing_binding(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    binding: str,
    displayed: str,
    value: str,
    expected: str,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = with_opacity(source_file.read_bytes(), "Text", binding)
    source_file.write_bytes(baseline)
    expected_source = with_opacity(
        source_file.read_bytes().replace(
            f"        opacity: {binding};\n".encode(), b""
        ),
        "Text",
        expected,
    )
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_element(window, "Text")
        wait_for_field(
            window, "Opacity", displayed, slint_testing.AccessibleRole.Slider
        )
        edit_field(window, "Opacity value", value)
        snapshot.wait_for_applied(expected_source, INSPECTOR_SOURCE)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, INSPECTOR_SOURCE)


def test_opacity_slider_keyboard_steps_by_one_percent(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    baseline = with_opacity(source_file.read_bytes(), "Rectangle", "0.5")
    source_file.write_bytes(baseline)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_element(window, "Rectangle")
        slider = inspector_field(window, "Opacity", slint_testing.AccessibleRole.Slider)
        expect(slider).to_have_value("50")
        position = slider_position(window, "Opacity", 0.5)
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
        press_keys(window, keys.LeftArrow)
        snapshot.wait_for_applied(
            replace_once(baseline, b"opacity: 0.5;", b"opacity: 0.49;"),
            INSPECTOR_SOURCE,
        )


def test_text_selection_shows_opacity_without_rectangle_effects(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source_file = fixture_project / INSPECTOR_SOURCE
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, source_file.read_bytes())
        window = first_window(editor)
        select_element(window, "Text")
        inspector_field(window, "Opacity", slint_testing.AccessibleRole.Slider)
        for label in ("Drop shadow", "Inner shadow", "Enable backdrop blur"):
            assert not elements(window, label)
