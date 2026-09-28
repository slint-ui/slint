# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore tobytes

from pathlib import Path

import pytest
import slint_testing
from canvas_interactions import center as element_center
from inspector_interactions import (
    FIELDS,
    edit_field,
    inspector_field,
    wait_for_field,
)
from slint_test import expect
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from test_inspector import (
    INSPECTOR_SOURCE,
    assert_rendered_element,
    select_element,
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
    editor_factory, fixture_project, property_name, original_value, value, old, new
):

    source = fixture_project / INSPECTOR_SOURCE
    baseline = source.read_bytes()
    snapshot = SourceSnapshot.capture(fixture_project)
    with editor_factory(source) as editor:
        rectangle = editor.canvas.element("inspect-rectangle")
        rectangle.select()
        expected = float(value)
        if property_name in ("x", "y"):
            expected += (
                getattr(rectangle.locator().bounds(), property_name) - original_value
            )
        field = editor.inspector.reveal(property_name)
        field.set_accessible_value(value)
        snapshot.wait_for_exact(replace_once(baseline, old, new), INSPECTOR_SOURCE)
        expect(field).to_have_value(value)
        expect(rectangle.locator()).to_have_geometry(
            {property_name: pytest.approx(expected)}
        )


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
    editor_factory,
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

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Rectangle")
        label = FIELDS[property_name]
        scrubber = inspector_field(window, label + " scrubber", "slider")
        start = element_center(scrubber)
        end = slint_testing.LogicalPosition(x=start.x + 12, y=start.y)

        window.pointer.move_to(start)
        window.pointer.press_at(start)
        window.pointer.move_to(end)
        wait_for_field(
            window,
            label,
            str(initial + 12),
            "text-input",
        )
        snapshot.assert_unchanged()

        window.pointer.release_at(end)
        snapshot.wait_for_applied(expected, relative_path=INSPECTOR_SOURCE)
        window.keyboard.shortcut(keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=INSPECTOR_SOURCE)


def test_geometry_scrub_reverts_when_commit_is_rejected(
    editor_factory,
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

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, "Rectangle")
        scrubber = inspector_field(window, "Position X scrubber", "slider")
        start = element_center(scrubber)
        end = slint_testing.LogicalPosition(x=start.x + 12, y=start.y)

        window.pointer.move_to(start)
        window.pointer.press_at(start)
        window.pointer.move_to(end)
        wait_for_field(window, "Position X", "44")
        snapshot.assert_unchanged_now()

        edit_field(
            window,
            "Rectangle background",
            "#123456",
            "text-input",
        )
        window.pointer.release_at(end)

        wait_for_field(window, "Position X", "32")
        snapshot.wait_for_applied(background_edit, relative_path=INSPECTOR_SOURCE)
        window.keyboard.shortcut(keys.Control, "z")
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
    editor_factory,
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

    with editor_factory(source_file) as editor:
        window = editor.window
        select_element(window, kind)
        edit_field(window, label, value, "text-input")
        snapshot.wait_for_exact(
            replace_once(baseline, old, new), relative_path=INSPECTOR_SOURCE
        )
        assert_rendered_element(window, f"InspectorCases::inspect-{kind.lower()}")
