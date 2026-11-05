# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from pathlib import Path
from typing import cast

from editor_sync import wait_for_source
from PIL import Image
from ui_driver import element, file_row, first_window, launch_editor, screenshot


def _icon_coverage(image: Image.Image) -> tuple[float, ...]:
    """Return pixel contrast normalized by the icon's maximum contrast."""
    image = image.convert("RGB")
    background = cast(tuple[int, int, int], image.getpixel((0, 0)))
    distances = tuple(
        max(
            abs(value - base)
            for value, base in zip(
                cast(tuple[int, int, int], image.getpixel((x, y))),
                background,
                strict=True,
            )
        )
        for y in range(image.height)
        for x in range(image.width)
    )
    contrast = max(distances)
    assert contrast > 8, "Icon must be visible above background dithering"
    return tuple(distance / contrast for distance in distances)


def _shape_similarity(first: tuple[float, ...], second: tuple[float, ...]) -> float:
    pairs = tuple(zip(first, second, strict=True))
    return sum(min(left, right) for left, right in pairs) / sum(
        max(left, right) for left, right in pairs
    )


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

        def icon_shape(path: Path) -> tuple[tuple[int, int], tuple[float, ...]]:
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
            return crop.size, _icon_coverage(crop)

        code_shape = icon_shape(first_code)
        second_size, second_coverage = icon_shape(second_code)
        assert second_size == code_shape[0]
        assert _shape_similarity(code_shape[1], second_coverage) > 0.9
        slint_size, slint_coverage = icon_shape(fixture_project / "Sibling.slint")
        assert slint_size == code_shape[0]
        assert _shape_similarity(code_shape[1], slint_coverage) < 0.5
