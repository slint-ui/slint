# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import gc
import typing
import weakref
from pathlib import Path

import slint
from slint import slint as native


def test_callback_gc() -> None:
    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
        export component Test {
            out property <string> test-value: "Ok";
            callback test-callback(string) -> string;
        }
    """,
        Path(""),
    ).component("Test")
    assert compdef is not None

    instance: native.ComponentInstance | None = compdef.create()
    assert instance is not None

    class Handler:
        def __init__(self, instance: native.ComponentInstance) -> None:
            self.instance = instance

        def python_callback(self, input: str) -> str:
            return input + typing.cast(str, self.instance.get_property("test-value"))

    handler: Handler | None = Handler(instance)
    assert handler is not None
    instance.set_callback("test-callback", handler.python_callback)
    handler = None

    assert instance.invoke("test-callback", "World") == "WorldOk"

    wr = weakref.ref(instance)
    assert wr() is not None
    instance = None
    gc.collect()
    assert wr() is None


def test_struct_gc() -> None:
    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
        export struct Foo {
            data: [int]
        }
        export component Test {
            out property <Foo> test-value;
        }
    """,
        Path(""),
    ).component("Test")
    assert compdef is not None

    instance: native.ComponentInstance | None = compdef.create()
    assert instance is not None

    model: slint.ListModel[int] | None = slint.ListModel([1, 2, 3])
    assert model
    assert model.row_count() == 3

    test_value = instance.get_property("test-value")
    test_value.data = model
    model = None
    # test_value as a struct should hold a strong reference to the model field within
    gc.collect()
    assert test_value.data.row_count() == 3


def test_properties_gc() -> None:
    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
        export component Test {
            in-out property <[int]> test-value;
        }
    """,
        Path(""),
    ).component("Test")
    assert compdef is not None

    instance: native.ComponentInstance | None = compdef.create()
    assert instance is not None

    model: slint.ListModel[int] | None = slint.ListModel([1, 2, 3])
    assert model
    assert model.row_count() == 3

    instance.set_property("test-value", model)
    model = None
    gc.collect()
    assert instance.get_property("test-value").row_count() == 3


def test_model_survives_partial_gc() -> None:
    """A model only Slint still references must survive a partial collection.

    A young (partial) collection does not traverse the old component instance,
    so the wrapper of a model held in a property must not rely on that
    traversal to stay alive. It used to be collected, leaving the Rust model
    without its Python implementation ("Model implementation is lacking self
    object").
    """
    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
        export global TestGlobal {
            in-out property <[int]> test-value;
        }
        export component Test {
            in-out property <[int]> test-value;
        }
    """,
        Path(""),
    ).component("Test")
    assert compdef is not None

    instance: native.ComponentInstance | None = compdef.create()
    assert instance is not None

    # Park the instance in the old generation, like a long-running app does.
    # Young collections then no longer traverse it.
    gc.collect()

    model: slint.ListModel[int] | None = slint.ListModel([1, 2, 3])
    assert model is not None
    instance.set_property("test-value", model)
    instance.set_global_property("TestGlobal", "test-value", model)
    model = None

    # Collect only the young generations; a full collection would traverse the
    # instance and hide the bug.
    gc.collect(0)
    gc.collect(1)

    assert instance.get_property("test-value").row_count() == 3
    assert instance.get_global_property("TestGlobal", "test-value").row_count() == 3


def test_model_released_with_instance() -> None:
    """A model assigned to a property is released by reference count alone.

    When the instance dies, its properties drop the last `ModelRc`, which
    releases the wrapper. No garbage collection is needed.
    """
    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
        export component Test {
            in-out property <[int]> test-value;
        }
    """,
        Path(""),
    ).component("Test")
    assert compdef is not None

    instance: native.ComponentInstance | None = compdef.create()
    assert instance is not None

    model: slint.ListModel[int] | None = slint.ListModel([1, 2, 3])
    assert model is not None
    instance.set_property("test-value", model)

    instance_weak = weakref.ref(instance)
    model_weak = weakref.ref(model)
    model = None
    instance = None

    assert instance_weak() is None
    assert model_weak() is None


def test_unassigned_model_released_by_refcount() -> None:
    """A model that never reached a property holds no reference cycle.

    Such a model used to leak until the next full collection cleared the
    cycle between the wrapper and its shared model.
    """
    model = slint.ListModel([1, 2, 3])
    model_weak = weakref.ref(model)
    del model
    assert model_weak() is None


def test_model_reassignment_after_drop() -> None:
    """Assigning a model again after Slint dropped it brings it back.

    Python can keep a model alive after its last `ModelRc` went away (the
    replacement on a re-assignment). Handing such a model to Slint again
    wraps it in a fresh shared model, with the rows intact.
    """
    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
        export component Test {
            in-out property <[int]> test-value;
        }
    """,
        Path(""),
    ).component("Test")
    assert compdef is not None

    instance: native.ComponentInstance | None = compdef.create()
    assert instance is not None

    model = slint.ListModel([1, 2, 3])
    instance.set_property("test-value", model)
    # Replacing the property drops the last `ModelRc` of `model`.
    instance.set_property("test-value", slint.ListModel([7]))
    instance.set_property("test-value", model)

    assert instance.get_property("test-value").row_count() == 3

    # Mutations must still reach the views attached to the fresh model.
    model.append(4)
    assert instance.get_property("test-value").row_count() == 4


def test_custom_model_survives_partial_gc() -> None:
    """A user-defined Model keeps its Python state through a partial collection.

    Unlike `ListModel`, the row data of a `Model` subclass lives in the
    wrapper's Python state. The wrapper must survive partial collections
    just like `ListModel`'s, so rows are not lost while Slint still
    references the model.
    """

    class CustomModel(slint.Model[int]):
        def __init__(self) -> None:
            super().__init__()
            self._rows = [1, 2, 3]

        def row_count(self) -> int:
            return len(self._rows)

        def row_data(self, row: int) -> int | None:
            if 0 <= row < len(self._rows):
                return self._rows[row]
            return None

    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
        export component Test {
            in-out property <[int]> test-value;
        }
    """,
        Path(""),
    ).component("Test")
    assert compdef is not None

    instance: native.ComponentInstance | None = compdef.create()
    assert instance is not None

    # Park the instance in the old generation, like a long-running app does.
    gc.collect()

    model: CustomModel | None = CustomModel()
    assert model is not None
    instance.set_property("test-value", model)
    model = None

    gc.collect(0)
    gc.collect(1)

    assert instance.get_property("test-value").row_count() == 3
    assert instance.get_property("test-value").row_data(1) == 2


def test_model_in_reference_cycle_survives_gc() -> None:
    """A model only kept alive by Slint survives a full collection in a cycle.

    Slint keeps the wrapper alive through a reference invisible to the
    cyclic garbage collector. Even when the wrapper participates in a
    Python reference cycle, a full collection must not release it while
    Slint still owns the model: the wrapper has no `__clear__`, so the
    cycle cannot be broken and the model stays alive and usable.
    """

    class CustomModel(slint.Model[int]):
        def __init__(self) -> None:
            super().__init__()
            self._rows = [1, 2, 3]
            self.cycle: object | None = None

        def row_count(self) -> int:
            return len(self._rows)

        def row_data(self, row: int) -> int | None:
            if 0 <= row < len(self._rows):
                return self._rows[row]
            return None

    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
        export component Test {
            in-out property <[int]> test-value;
        }
    """,
        Path(""),
    ).component("Test")
    assert compdef is not None

    instance: native.ComponentInstance | None = compdef.create()
    assert instance is not None

    model: CustomModel | None = CustomModel()
    assert model is not None
    instance.set_property("test-value", model)
    model.cycle = model  # reference cycle through the wrapper
    model = None

    gc.collect()

    assert instance.get_property("test-value").row_count() == 3
    assert instance.get_property("test-value").row_data(1) == 2
