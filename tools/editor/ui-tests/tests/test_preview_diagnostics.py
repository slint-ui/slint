# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import (
    center,
    element_frame,
    fixture_element,
    offset_position,
    same_state,
)
from gradient_interactions import gesture, open_gradient
from slint_testing import keys
from source_snapshot import SourceSnapshot
from ui_driver import (
    elements_with_label,
    file_row,
    first_window,
    launch_editor,
    palette_row,
    press_key,
    screenshot,
    select_outline_row,
    wait_until,
    window_element_with_label,
)


@pytest.mark.parametrize("relative_path", ["Main.slint", "components/Nested.slint"])
def test_broken_preview_reports_locations_blocks_edits_and_recovers(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tmp_path: Path,
    relative_path: str,
) -> None:
    source = fixture_project / "Main.slint"
    broken_file = fixture_project / relative_path
    baseline = broken_file.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        window_element_with_label(
            window, "Fixture text", slint_testing.AccessibleRole.Text
        )
        select_outline_row(window, "root-text")
        if relative_path.startswith("components/"):
            folder = file_row(window, fixture_project / "components")
            assert folder.accessible_description == ""
        broken_file.write_bytes(baseline + b"\nthis is not valid Slint\n")
        card = window_element_with_label(
            window, "Stale preview", slint_testing.AccessibleRole.Region
        )
        if relative_path.startswith("components/"):
            folder = file_row(window, fixture_project / "components")
            assert "errors" in folder.accessible_description
            folder.invoke_accessible_expand_action()
        wait_until(
            lambda: (
                True if file_row(window, broken_file).accessible_description else None
            )
        )
        assert "errors" in file_row(window, fixture_project).accessible_description
        labels = [
            element.accessible_label for element in card.query_descendants().find_all()
        ]
        assert any(label.startswith(relative_path + ":") for label in labels)
        window_element_with_label(
            window, "Editing paused until the preview is available"
        )
        assert not window_element_with_label(
            window, "Inspector and outline"
        ).accessible_enabled
        snapshot = SourceSnapshot.capture(fixture_project)
        file_row(window, broken_file)
        window_element_with_label(
            window, "root-text", slint_testing.AccessibleRole.ListItem
        ).invoke_accessible_default_action()
        press_key(window, keys.Delete)
        canvas = window_element_with_label(
            window, "Editor canvas", slint_testing.AccessibleRole.Main
        )
        target = center(canvas)
        row = palette_row(window, "Rectangle")
        assert not row.accessible_enabled
        gesture(window, center(row), target)
        snapshot.assert_unchanged()
        assert not elements_with_label(window.root_element, "Selected Text")
        screenshot(window).save(tmp_path / "stale-preview.png")
        broken_file.write_bytes(baseline)
        window_element_with_label(
            window, "Fixture text", slint_testing.AccessibleRole.Text
        )
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "Stale preview")
                else None
            )
        )
        assert file_row(window, broken_file).accessible_description == ""
        select_outline_row(window, "root-text")
        window_element_with_label(
            window, "Selected Text", slint_testing.AccessibleRole.Region
        )


@pytest.mark.parametrize("switch_from_valid", [False, True])
def test_unavailable_preview_hides_unrelated_render_and_recovers(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tmp_path: Path,
    switch_from_valid: bool,
) -> None:
    broken = fixture_project / "Broken.slint"
    broken.write_text("export component Broken inherits Window { broken }\n")
    entry = fixture_project / "Main.slint" if switch_from_valid else broken
    with launch_editor(editor_binary, editor_environment, entry) as editor:
        window = first_window(editor)
        if switch_from_valid:
            window_element_with_label(
                window, "Fixture text", slint_testing.AccessibleRole.Text
            )
            file_row(window, broken).invoke_accessible_default_action()
        window_element_with_label(
            window, "Preview unavailable", slint_testing.AccessibleRole.Region
        )
        screenshot(window).save(tmp_path / "unavailable-preview.png")
        broken.write_text(
            'export component Broken inherits Window { Text { text: "Recovered preview"; } }\n'
        )
        window_element_with_label(
            window, "Recovered preview", slint_testing.AccessibleRole.Text
        )
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "Preview unavailable")
                else None
            )
        )
        assert file_row(window, broken).accessible_description == ""


def test_warnings_show_details_without_blocking_editing(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tmp_path: Path,
) -> None:
    source = fixture_project / "Warnings.slint"
    source.write_text(
        'export Warnings := Window { width: 320px; height: 240px; label := Text { text: "Warning preview"; } }\n'
    )
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        window_element_with_label(
            window, "Warning preview", slint_testing.AccessibleRole.Text
        )
        window_element_with_label(
            window, "Preview warnings", slint_testing.AccessibleRole.Region
        )
        assert "warnings" in file_row(window, source).accessible_description
        window_element_with_label(
            window,
            "Toggle preview warning details",
            slint_testing.AccessibleRole.Button,
        ).invoke_accessible_default_action()
        labels = [
            element.accessible_label
            for element in window.root_element.query_descendants().find_all()
        ]
        assert any("deprecated" in label for label in labels)
        select_outline_row(window, "label")
        window_element_with_label(
            window, "Selected Text", slint_testing.AccessibleRole.Region
        )
        assert not elements_with_label(
            window.root_element, "Editing paused until the preview is available"
        )
        screenshot(window).save(tmp_path / "warning-preview.png")


def test_valid_global_file_has_neutral_preview_state(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    tmp_path: Path,
) -> None:
    source = fixture_project / "Globals.slint"
    source.write_text("export global Globals { out property <int> answer: 42; }\n")
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        window_element_with_label(
            window, "No previewable component", slint_testing.AccessibleRole.Region
        )
        assert file_row(window, source).accessible_description == ""
        screenshot(window).save(tmp_path / "neutral-preview.png")


def test_broken_preview_cancels_active_fill_session(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        open_gradient(window)
        window_element_with_label(
            window, "Close Custom", slint_testing.AccessibleRole.Button
        )
        source.write_bytes(baseline + b"\nthis is not valid Slint\n")
        window_element_with_label(
            window, "Stale preview", slint_testing.AccessibleRole.Region
        )
        wait_until(
            lambda: (
                True
                if not elements_with_label(window.root_element, "Close Custom")
                else None
            )
        )
        SourceSnapshot.capture(fixture_project).assert_unchanged()


def test_broken_preview_rolls_back_an_interrupted_canvas_resize(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    baseline = source.read_bytes()
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        select_outline_row(window, "root-rectangle")
        initial = element_frame(fixture_element(window, "Rectangle"))
        handle = window_element_with_label(window, "Rectangle resize right")
        start = center(handle)
        end = offset_position(start, 40, 0, 0)
        button = slint_testing.PointerEventButton.Left
        window.dispatch_event(slint_testing.PointerPressEvent(start, button))
        window.dispatch_event(slint_testing.PointerMoveEvent(end))
        wait_until(
            lambda: (
                True
                if element_frame(fixture_element(window, "Rectangle"))[2]
                > initial[2] + 10
                else None
            )
        )
        SourceSnapshot.capture(fixture_project).assert_unchanged()
        source.write_bytes(baseline + b"\nthis is not valid Slint\n")
        window_element_with_label(
            window, "Stale preview", slint_testing.AccessibleRole.Region
        )
        wait_until(
            lambda: (
                True
                if same_state(
                    element_frame(fixture_element(window, "Rectangle")), initial
                )
                else None
            )
        )
        window.dispatch_event(slint_testing.PointerReleaseEvent(end, button))
        SourceSnapshot.capture(fixture_project).assert_unchanged()
