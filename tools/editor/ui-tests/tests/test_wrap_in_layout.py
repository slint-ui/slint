# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_driver import (
    element,
    elements,
    first_window,
    launch_editor,
    press_shortcut,
    select_outline_row,
    wait_until,
)

WRAP_LAYOUT_SOURCE = "WrapLayoutCases.slint"
BUTTON = (
    b"    wrapped-button := Button {\n"
    b"        x: 32px;\n"
    b"        y: 32px;\n"
    b'        text: "Wrap me";\n'
    b"    }\n"
)


def wrapped_button(layout: str) -> bytes:
    return (
        f"    {layout} {{\n"
        "        x: 32px;\n"
        "        y: 32px;\n"
        "        wrapped-button := Button {\n"
        '            text: "Wrap me";\n'
        "        }\n"
        "    }\n"
    ).encode()


@pytest.mark.parametrize(
    ("layout", "import_line"),
    [
        ("VerticalLayout", b"import { Button }"),
        ("HorizontalBox", b"import { Button, HorizontalBox }"),
    ],
)
def test_wrap_in_layout_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    layout: str,
    import_line: bytes,
) -> None:
    editor_environment["SLINT_NO_MUDA"] = "1"
    source_file = fixture_project / WRAP_LAYOUT_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(
        replace_once(baseline, BUTTON, wrapped_button(layout)),
        b"import { Button }",
        import_line,
    )
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_outline_row(window, "wrapped-button")

        element(window, "Edit").single_click(slint_testing.PointerEventButton.Left)
        element(window, "Wrap in Layout").single_click(
            slint_testing.PointerEventButton.Left
        )

        def layout_menu_item() -> slint_testing.Element | None:
            return next(
                (
                    item
                    for item in elements(window, layout)
                    if item.accessible_role != slint_testing.AccessibleRole.ListItem
                ),
                None,
            )

        wait_until(layout_menu_item).single_click(slint_testing.PointerEventButton.Left)
        snapshot.wait_for_applied(expected, relative_path=WRAP_LAYOUT_SOURCE)
        element(window, f"Selected {layout}", role=slint_testing.AccessibleRole.Region)

        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=WRAP_LAYOUT_SOURCE)
