# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from slint import Color, Brush


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
