# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from editor_sync import wait_for_source
from inspector_interactions import edit_field, wait_for_field
from slint_testing import keys
from source_snapshot import SourceSnapshot, replace_once
from ui_driver import (
    element,
    first_window,
    launch_editor,
    press_shortcut,
    select_outline_row,
)

SCROLLBAR_POLICY_SOURCE = "ScrollBarPolicyCases.slint"
LABELS = {
    "as-needed": "As needed",
    "always-on": "Always on",
    "always-off": "Always off",
}


@pytest.mark.parametrize(
    ("row", "kind", "field", "current", "value", "old", "new"),
    [
        (
            "default-scroll-view",
            "ScrollView",
            "Horizontal Scroll Bar Policy",
            "as-needed",
            "always-off",
            b"        height: 160px;\n    }\n",
            b"        height: 160px;\n        horizontal-scrollbar-policy: always-off;\n    }\n",
        ),
        (
            "default-scroll-view",
            "ScrollView",
            "Vertical Scroll Bar Policy",
            "as-needed",
            "always-on",
            b"        y: 32px;\n",
            b"        y: 32px;\n        vertical-scrollbar-policy: always-on;\n",
        ),
        (
            "always-on-list-view",
            "ListView",
            "Horizontal Scroll Bar Policy",
            "always-on",
            "as-needed",
            b"        horizontal-scrollbar-policy: always-on;\n",
            b"        horizontal-scrollbar-policy: as-needed;\n",
        ),
        (
            "always-on-list-view",
            "ListView",
            "Vertical Scroll Bar Policy",
            "always-off",
            "always-on",
            b"        vertical-scrollbar-policy: always-off;\n",
            b"        vertical-scrollbar-policy: always-on;\n",
        ),
    ],
    ids=(
        "scroll-view-horizontal",
        "scroll-view-vertical",
        "list-view-horizontal",
        "list-view-vertical",
    ),
)
def test_scrollbar_policy_writes_exact_source(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
    row: str,
    kind: str,
    field: str,
    current: str,
    value: str,
    old: bytes,
    new: bytes,
) -> None:
    source_file = fixture_project / SCROLLBAR_POLICY_SOURCE
    baseline = source_file.read_bytes()
    expected = replace_once(baseline, old, new)
    snapshot = SourceSnapshot.capture(fixture_project)

    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        wait_for_source(source_file, baseline)
        window = first_window(editor)
        select_outline_row(window, row)
        element(window, f"Selected {kind}", role=slint_testing.AccessibleRole.Region)
        role = slint_testing.AccessibleRole.Combobox
        wait_for_field(window, field, LABELS[current], role)
        edit_field(window, field, value, role)
        snapshot.wait_for_applied(expected, relative_path=SCROLLBAR_POLICY_SOURCE)
        wait_for_field(window, field, LABELS[value], role)
        press_shortcut(window, keys.Control, "z")
        snapshot.wait_for_applied(baseline, relative_path=SCROLLBAR_POLICY_SOURCE)
        wait_for_field(window, field, LABELS[current], role)
