# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Tests for the built-in enums exposed via the slint.language submodule."""

import enum
from pathlib import Path

import slint
from slint.language import ColorScheme, PointerEventButton, PointerEventKind


def test_color_scheme_is_an_enum() -> None:
    assert issubclass(ColorScheme, enum.Enum)
    assert {m.value for m in ColorScheme} == {"unknown", "dark", "light"}


def test_pointer_event_button_variants() -> None:
    assert {m.value for m in PointerEventButton} == {
        "other",
        "left",
        "right",
        "middle",
        "back",
        "forward",
    }


def test_pointer_event_kind_variants() -> None:
    assert {m.value for m in PointerEventKind} == {"cancel", "down", "up", "move"}


def test_color_scheme_round_trip(tmp_path: Path) -> None:
    slint_file = tmp_path / "scheme.slint"
    slint_file.write_text(
        """
        export component App {
            in-out property <ColorScheme> scheme: ColorScheme.unknown;
        }
        """
    )
    module = slint.load_file(slint_file, quiet=True)
    app = module.App()

    assert app.scheme == ColorScheme.unknown
    assert isinstance(app.scheme, ColorScheme)

    app.scheme = ColorScheme.dark
    assert app.scheme == ColorScheme.dark
    assert isinstance(app.scheme, ColorScheme)
    assert app.scheme.value == "dark"
