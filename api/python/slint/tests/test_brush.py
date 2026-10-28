# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import re

import pytest

from slint import Brush, Color


def test_col_default() -> None:
    col = Color()
    assert col.red == 0
    assert col.green == 0
    assert col.blue == 0
    assert col.alpha == 0


def test_col_from_str() -> None:
    col = Color("#123456")
    assert col.red == 0x12
    assert col.green == 0x34
    assert col.blue == 0x56
    assert col.alpha == 255
    assert str(col) == "argb(255, 18, 52, 86)"


def test_col_from_slint_color_string() -> None:
    col = Color("#33669980")
    assert (col.red, col.green, col.blue, col.alpha) == (0x33, 0x66, 0x99, 0x80)
    assert Color("#abc") == Color("#aabbcc")
    assert Color("RebeccaPurple") == Color("#663399")
    assert Color("transparent").alpha == 0
    for string in ["not a color", "rgb(255, 0, 0)"]:
        with pytest.raises(
            RuntimeError, match=re.escape(f"Could not convert {string} to Color")
        ):
            Color(string)


def test_col_from_rgb_dict() -> None:
    coldict = {"red": 0x12, "green": 0x34, "blue": 0x56}
    col = Color(coldict)
    assert col.red == 0x12
    assert col.green == 0x34
    assert col.blue == 0x56
    assert col.alpha == 255


def test_col_from_rgba_dict() -> None:
    coldict = {"red": 0x12, "green": 0x34, "blue": 0x56, "alpha": 128}
    col = Color(coldict)
    assert col.red == 0x12
    assert col.green == 0x34
    assert col.blue == 0x56
    assert col.alpha == 128


def test_from_col() -> None:
    col = Color("#123456")
    brush = Brush(col)
    assert brush.color == col
