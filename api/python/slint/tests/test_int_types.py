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
