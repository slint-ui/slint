# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path

from editor_sync import wait_for_source
from ui_driver import element, file_row, first_window, launch_editor, screenshot


def test_slint_files_have_a_distinct_icon_shape(
    editor_binary: Path,
    editor_environment: dict[str, str],
    fixture_project: Path,
) -> None:
    source = fixture_project / "Main.slint"
    first_code = fixture_project / "A.rs"
    second_code = fixture_project / "B.rs"
    first_code.write_text("// Ordinary code file\n")
    second_code.write_text("// Another ordinary code file\n")

    with launch_editor(editor_binary, editor_environment, source) as editor:
        wait_for_source(source, source.read_bytes())
        window = first_window(editor)

        def icon_shape(path: Path) -> tuple[tuple[int, int], tuple[bool, ...]]:
            icon = element(file_row(window, path), id="FileTreeRow::file-icon")
            bounds = icon.absolute_rect
            image = screenshot(window)
            scale = image.width / window.root_element.size.width
            crop = image.crop(
                (
                    round(bounds.x * scale),
                    round(bounds.y * scale),
                    round((bounds.x + bounds.width) * scale),
                    round((bounds.y + bounds.height) * scale),
                )
            )
            background = crop.getpixel((0, 0))
            mask = tuple(
                crop.getpixel((x, y)) != background
                for y in range(crop.height)
                for x in range(crop.width)
            )
            assert any(mask) and not all(mask)
            return crop.size, mask

        code_shape = icon_shape(first_code)
        assert icon_shape(second_code) == code_shape
        assert icon_shape(fixture_project / "Sibling.slint") != code_shape
