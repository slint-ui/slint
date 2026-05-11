# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""
Tests pinning down that values declared as `int` in .slint stay as Python
`int` across the boundary, while `float` declarations stay as `float`.

The interpreter represents every number as `Value::Number(f64)`; the bindings
must consult the declared `Type` (Int32 vs Float32) and convert accordingly.
"""

import typing
from pathlib import Path
from slint import slint as native
from slint import models


def _build(source: str, name: str = "Test") -> native.ComponentInstance:
    compdef = native.Compiler().build_from_source(source, Path("")).component(name)
    assert compdef is not None
    instance = compdef.create()
    assert instance is not None
    return instance


def test_callback_int_arg_is_int_in_python() -> None:
    """The original bug: a Slint callback declared with `int` args reaches
    the Python handler as a float."""
    instance = _build(
        """
        export component Test {
            callback ping(int, float);
        }
        """
    )
    received: list[object] = []

    def handler(a: object, b: object) -> None:
        received.append(a)
        received.append(b)

    instance.set_callback("ping", handler)
    instance.invoke("ping", 3, 1.5)
    assert len(received) == 2
    assert type(received[0]) is int, f"expected int, got {type(received[0]).__name__}"
    assert received[0] == 3
    assert type(received[1]) is float
    assert received[1] == 1.5


def test_callback_int_return_round_trips_as_int() -> None:
    """A callback declared `-> int` whose Python implementation returns an int
    should land back in Python as an int when invoked."""
    instance = _build(
        """
        export component Test {
            callback compute() -> int;
        }
        """
    )
    instance.set_callback("compute", lambda: 42)
    result = instance.invoke("compute")
    assert type(result) is int
    assert result == 42


def test_invoke_function_returning_int() -> None:
    """A pure-Slint function declared `-> int` must return Python int from
    invoke()."""
    instance = _build(
        """
        export component Test {
            public function answer() -> int { 42 }
            public function ratio() -> float { 0.5 }
        }
        """
    )
    intval = instance.invoke("answer")
    floatval = instance.invoke("ratio")
    assert type(intval) is int
    assert intval == 42
    assert type(floatval) is float
    assert floatval == 0.5


def test_struct_int_field_in_callback_arg() -> None:
    instance = _build(
        """
        export struct Item { count: int }
        export component Test {
            callback got(Item);
        }
        """
    )
    received: list[object] = []
    instance.set_callback("got", lambda item: received.append(item))
    instance.invoke("got", {"count": 9})
    assert len(received) == 1
    item = typing.cast(typing.Any, received[0])
    assert type(item.count) is int
    assert item.count == 9


def test_global_callback_int_arg_is_int() -> None:
    instance = _build(
        """
        export global G {
            callback ping(int);
        }
        export component Test { }
        """
    )
    received: list[object] = []
    instance.set_global_callback("G", "ping", lambda x: received.append(x))
    instance.invoke_global("G", "ping", 11)
    assert len(received) == 1
    assert type(received[0]) is int
    assert received[0] == 11


def test_int_property_get_returns_int() -> None:
    instance = _build(
        """
        export component Test {
            in-out property <int> intprop: 42;
            in-out property <float> floatprop: 1.5;
        }
        """
    )
    intval = instance.get_property("intprop")
    floatval = instance.get_property("floatprop")
    assert type(intval) is int, f"expected int, got {type(intval).__name__}"
    assert intval == 42
    assert type(floatval) is float, f"expected float, got {type(floatval).__name__}"
    assert floatval == 1.5


def test_int_property_set_then_get_returns_int() -> None:
    instance = _build(
        """
        export component Test {
            in-out property <int> intprop;
        }
        """
    )
    instance.set_property("intprop", 7)
    val = instance.get_property("intprop")
    assert type(val) is int
    assert val == 7


def test_struct_int_field_preserves_type() -> None:
    instance = _build(
        """
        export struct Item { count: int, ratio: float }
        export component Test {
            in-out property <Item> item: { count: 7, ratio: 0.25 };
        }
        """
    )
    item = instance.get_property("item")
    assert type(item.count) is int, f"expected int, got {type(item.count).__name__}"
    assert item.count == 7
    assert type(item.ratio) is float
    assert item.ratio == 0.25


def test_int_model_iteration_yields_ints() -> None:
    instance = _build(
        """
        export component Test {
            in-out property <[int]> data: [1, 2, 3];
        }
        """
    )
    rows = list(instance.get_property("data"))
    assert all(type(r) is int for r in rows), [type(r).__name__ for r in rows]
    assert rows == [1, 2, 3]


def test_struct_model_int_field_in_iteration() -> None:
    instance = _build(
        """
        export struct Item { count: int }
        export component Test {
            in-out property <[Item]> items: [{count: 1}, {count: 2}];
        }
        """
    )
    rows = list(instance.get_property("items"))
    assert len(rows) == 2
    for i, row in enumerate(rows, start=1):
        assert type(row.count) is int
        assert row.count == i


def test_set_row_data_from_slint_preserves_int() -> None:
    """Writing an int row from Slint into a Python Model must reach the
    Python `set_row_data` as `int`, not `float`."""
    received: list[object] = []

    class Capturing(models.ListModel[int]):
        def set_row_data(self, row: int, value: int) -> None:
            received.append(value)
            super().set_row_data(row, value)

    instance = _build(
        """
        export component Test {
            in-out property <[int]> m;
            public function write(row: int, val: int) { m[row] = val; }
        }
        """
    )
    instance.set_property("m", Capturing([0, 0, 0]))
    instance.invoke("write", 1, 7)
    assert len(received) == 1
    assert type(received[0]) is int, f"expected int, got {type(received[0]).__name__}"
    assert received[0] == 7


def test_global_int_property() -> None:
    instance = _build(
        """
        export global G {
            in-out property <int> n: 5;
        }
        export component Test { }
        """
    )
    val = instance.get_global_property("G", "n")
    assert type(val) is int
    assert val == 5
