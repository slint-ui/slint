# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path
from textwrap import indent

import pytest
import slint_testing
from editor_sync import wait_for_source
from slint_testing import keys
from source_snapshot import SourceSnapshot
from test_outline import drop_position, outline_row, outline_rows
from test_palette import begin_palette_drag, canvas_drop_position, release_palette_drag
from test_undo_redo import shortcut
from ui_driver import (
    PALETTE_KINDS,
    elements_with_label,
    first_window,
    launch_editor,
    press_key,
    wait_until,
    window_element_with_label,
)

ELEMENTS = {
    "Rectangle": """Rectangle {
    width: 160px;
    height: 64px;
    background: #ffffff;
    border-radius: 12px;
    border-color: #d0d7de;
    border-width: 1px;
}
""",
    "Text": """Text {
    text: "Text";
    width: 220px;
    height: 40px;
    color: #1f2328;
    font-size: 24px;
    vertical-alignment: center;
}
""",
    "Image": """Image {
    source: @image-url("EDIT_ME.png");
    width: 160px;
    height: 96px;
    image-fit: contain;
}
""",
    "TouchArea": """TouchArea {
    width: 160px;
    height: 96px;
}
""",
}


@pytest.mark.parametrize("kind", PALETTE_KINDS)
@pytest.mark.parametrize(
    "target,location",
    [
        ("container", "onto"),
        ("sibling-a", "before"),
        ("sibling-a", "after"),
        ("<component-root>", "onto"),
        ("<outline-root>", "onto"),
    ],
)
def test_palette_outline_insertion(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
    target: str,
    location: str,
) -> None:
    source = fixture_project / "OutlineCases.slint"
    baseline = source.read_text()
    snapshot = SourceSnapshot.capture(fixture_project)
    if target == "container":
        expected = baseline.replace(
            "    }\n\n    sibling-a",
            indent(ELEMENTS[kind], "        ") + "    }\n\n    sibling-a",
        )
        level = "Hierarchy level 3"
    elif target == "sibling-a":
        marker = "    sibling-a" if location == "before" else "    sibling-b"
        expected = baseline.replace(marker, indent(ELEMENTS[kind], "    ") + marker)
        level = "Hierarchy level 2"
    else:
        expected = baseline[:-2] + indent(ELEMENTS[kind], "    ") + "}\n"
        level = "Hierarchy level 2"

    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, baseline.encode())
        if target == "container":
            outline_row(window, target).invoke_accessible_expand_action()
        position = drop_position(window, target, location)
        begin_palette_drag(window, kind, position)
        window_element_with_label(window, "Outline drag preview")
        assert not elements_with_label(window.root_element, f"{kind} drag preview")
        snapshot.assert_unchanged_now()
        release_palette_drag(window, position)
        snapshot.wait_for_applied(expected.encode(), source.name)
        inserted = wait_until(
            lambda: next(
                (
                    row
                    for row in outline_rows(window)
                    if row.accessible_label.strip() == kind
                ),
                None,
            )
        )
        assert inserted.accessible_description == level
        wait_until(lambda: True if inserted.accessible_item_selected else None)
        shortcut(window, redo=False)
        snapshot.wait_for_applied(baseline.encode(), source.name)
        shortcut(window, redo=True)
        snapshot.wait_for_applied(expected.encode(), source.name)


@pytest.mark.parametrize("kind", PALETTE_KINDS)
def test_palette_preview_switches_between_canvas_and_outline_and_cancels(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source = fixture_project / "OutlineCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, source.read_bytes())
        canvas = canvas_drop_position(window)
        outline = drop_position(window, "container", "onto")
        begin_palette_drag(window, kind, canvas)
        window_element_with_label(window, f"{kind} drag preview")
        window.dispatch_event(slint_testing.PointerMoveEvent(outline))
        ghost = window_element_with_label(window, "Outline drag preview")
        assert not elements_with_label(window.root_element, f"{kind} drag preview")
        assert ghost.size.height == 32
        window.dispatch_event(slint_testing.PointerMoveEvent(canvas))
        window_element_with_label(window, f"{kind} drag preview")
        assert not elements_with_label(window.root_element, "Outline drag preview")
        window.dispatch_event(slint_testing.PointerMoveEvent(outline))
        press_key(window, keys.Escape)
        release_palette_drag(window, outline)
        assert not elements_with_label(window.root_element, "Outline drag preview")
        snapshot.assert_unchanged()


@pytest.mark.parametrize("kind", PALETTE_KINDS)
def test_palette_outline_layout_controls_geometry(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source = fixture_project / "CanvasCases.slint"
    baseline = source.read_text()
    element = (
        "\n".join(
            line
            for line in ELEMENTS[kind].splitlines()
            if not line.strip().startswith(("width:", "height:"))
        )
        + "\n"
    )
    if kind == "TouchArea":
        element = "TouchArea { }\n"
    expected = baseline.replace(
        "    }\n\n    HorizontalLayout",
        indent(element, "        ") + "    }\n\n    HorizontalLayout",
    )
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, baseline.encode())
        target = drop_position(window, "prohibited-layout", "onto")
        begin_palette_drag(window, kind, target)
        release_palette_drag(window, target)
        snapshot.wait_for_applied(expected.encode(), source.name)


@pytest.mark.parametrize("kind", PALETTE_KINDS)
def test_palette_outline_rejects_insertion_before_component_root(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    kind: str,
) -> None:
    source = fixture_project / "OutlineCases.slint"
    snapshot = SourceSnapshot.capture(fixture_project)
    with launch_editor(editor_binary, editor_environment, source) as editor:
        window = first_window(editor)
        wait_for_source(source, source.read_bytes())
        target = drop_position(window, "<component-root>", "before")
        begin_palette_drag(window, kind, target)
        release_palette_drag(window, target)
        snapshot.assert_unchanged()
