# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Tests for the LogicalPosition and LogicalSize value classes."""

import typing
from pathlib import Path

import pytest

import slint


def test_logical_position_construct_and_access() -> None:
    p = slint.LogicalPosition(10.0, 20.0)
    assert p.x == 10.0
    assert p.y == 20.0
    assert isinstance(p, slint.LogicalPosition)


def test_logical_position_defaults_to_zero() -> None:
    p = slint.LogicalPosition()
    assert p.x == 0.0
    assert p.y == 0.0


def test_logical_position_equality_and_hash() -> None:
    a = slint.LogicalPosition(1.5, 2.5)
    b = slint.LogicalPosition(1.5, 2.5)
    c = slint.LogicalPosition(1.5, 9.0)
    assert a == b
    assert a != c
    assert hash(a) == hash(b)


def test_logical_position_repr() -> None:
    assert repr(slint.LogicalPosition(3.0, 4.0)) == "LogicalPosition(x=3, y=4)"


def test_logical_size_construct_and_access() -> None:
    s = slint.LogicalSize(100.0, 50.0)
    assert s.width == 100.0
    assert s.height == 50.0
    assert isinstance(s, slint.LogicalSize)


def test_logical_size_defaults_to_zero() -> None:
    s = slint.LogicalSize()
    assert s.width == 0.0
    assert s.height == 0.0


def test_round_trip_through_slint_property(tmp_path: Path) -> None:
    """Setting and reading a Point/Size property round-trips through the typed classes."""
    slint_file = tmp_path / "pos.slint"
    slint_file.write_text(
        """
        export component App inherits Window {
            in-out property <Point> p: { x: 1px, y: 2px };
            in-out property <Size> s: { width: 3px, height: 4px };
        }
        """
    )
    module = slint.load_file(slint_file, quiet=True)
    app = module.App()

    # Initial values arrive as the typed classes.
    assert isinstance(app.p, slint.LogicalPosition)
    assert app.p == slint.LogicalPosition(1.0, 2.0)
    assert isinstance(app.s, slint.LogicalSize)
    assert app.s == slint.LogicalSize(3.0, 4.0)

    # Setting from Python and reading back preserves the value.
    app.p = slint.LogicalPosition(10.0, 20.0)
    assert app.p == slint.LogicalPosition(10.0, 20.0)

    app.s = slint.LogicalSize(100.0, 50.0)
    assert app.s == slint.LogicalSize(100.0, 50.0)

    # Dict writes round-trip into the pyclass on read.
    app.p = {"x": 11.0, "y": 12.0}
    assert app.p == slint.LogicalPosition(11.0, 12.0)
    app.s = {"width": 13.0, "height": 14.0}
    assert app.s == slint.LogicalSize(13.0, 14.0)

    # NamedTuple writes (with matching field names) round-trip too.
    class PointNT(typing.NamedTuple):
        x: float
        y: float

    class SizeNT(typing.NamedTuple):
        width: float
        height: float

    app.p = PointNT(21.0, 22.0)
    assert app.p == slint.LogicalPosition(21.0, 22.0)
    app.s = SizeNT(23.0, 24.0)
    assert app.s == slint.LogicalSize(23.0, 24.0)

    # Plain tuples are not accepted: the runtime requires a typed value,
    # dict, or NamedTuple. The generated `.py` annotation reflects this
    # (slint.LogicalPosition / slint.LogicalSize), so a static type checker
    # also flags such assignments.
    with pytest.raises(TypeError):
        app.p = (31.0, 32.0)  # type: ignore[assignment]
    with pytest.raises(TypeError):
        app.s = (33.0, 34.0)  # type: ignore[assignment]
