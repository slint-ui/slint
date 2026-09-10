# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

import pytest
import slint_testing
from ui_driver import (
    first_window,
    launch_editor,
    select_outline_row,
    wait_until,
    window_element_with_label,
)


@pytest.mark.parametrize("kind", ["radial", "conic"])
def test_custom_gradient_geometry_uses_layout_size(
    editor_binary: Path,
    editor_environment: dict[str, str],
    tmp_path: Path,
    kind: str,
) -> None:
    source_file = tmp_path / "LayoutGradient.slint"
    gradient = (
        "@radial-gradient(circle, red 0%, blue 100%)"
        if kind == "radial"
        else "@conic-gradient(from 0deg, red 0deg, blue 360deg)"
    )
    source = f"""export component LayoutGradient inherits Window {{
    width: 400px;
    height: 400px;
    VerticalLayout {{
        fill := Rectangle {{
            background: {gradient};
        }}
    }}
}}
"""
    source_file.write_text(source)
    with launch_editor(editor_binary, editor_environment, source_file) as editor:
        window = first_window(editor)
        select_outline_row(window, "fill")
        rectangle = wait_until(
            lambda: next(iter(window.find_elements_by_id("LayoutGradient::fill")), None)
        )
        assert rectangle.size.width == pytest.approx(400)
        assert rectangle.size.height == pytest.approx(400)

        def field(label, role):
            return window_element_with_label(window, label, role)

        field(
            "Rectangle background color picker", slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        field(
            "Gradient center", slint_testing.AccessibleRole.Combobox
        ).accessible_value = "Custom"
        for axis in ("X", "Y"):
            assert float(
                field(
                    f"Gradient center {axis}", slint_testing.AccessibleRole.TextInput
                ).accessible_value
            ) == pytest.approx(200)
        if kind == "radial":
            field(
                "Gradient radius mode", slint_testing.AccessibleRole.Combobox
            ).accessible_value = "Custom"
            assert float(
                field(
                    "Gradient radius", slint_testing.AccessibleRole.TextInput
                ).accessible_value
            ) == pytest.approx(282.8)
        assert source_file.read_text() == source
        field(
            "Close Custom", slint_testing.AccessibleRole.Button
        ).invoke_accessible_default_action()
        wait_until(
            lambda: True if "at 200px 200px" in source_file.read_text() else None
        )
        if kind == "radial":
            assert "circle 282.842" in source_file.read_text()
