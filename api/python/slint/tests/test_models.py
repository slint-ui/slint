# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cSpell: ignore capfd

import typing
from pathlib import Path

import pytest

from slint import models
from slint import slint as native


def test_row_modification_rejection_raises() -> None:
    model = models.ListModel([1, 2, 3])
    with pytest.raises(IndexError):
        model.remove_row(3)
    with pytest.raises(IndexError):
        model.insert_row(5, 4)

    class ReadOnly(models.Model[int]):
        def row_count(self) -> int:
            return 1

        def row_data(self, row: int) -> int | None:
            return 42

    with pytest.raises(NotImplementedError):
        ReadOnly().push_row(1)


def test_rejected_modification_logs_python_class_name(
    capfd: pytest.CaptureFixture[str],
) -> None:
    compiler = native.Compiler()
    compdef = compiler.build_from_source(
        """
        export component App {
            in-out property<[int]> ints;
            public function push-one() { ints.push(4) }
        }
        """,
        Path(""),
    ).component("App")
    assert compdef is not None
    instance = compdef.create()
    assert instance is not None

    class ReadOnly(models.Model[int]):
        def row_count(self) -> int:
            return 1

        def row_data(self, row: int) -> int | None:
            return 42

    instance.set_property("ints", ReadOnly())
    instance.invoke("push_one")

    err = capfd.readouterr().err
    assert (
        "array.push(): the model ReadOnly does not support this modification" in err
    ), err


def test_model_notify() -> None:
    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
  export component App {
    width: 300px;
    height: 300px;

    out property<length> layout-height: layout.height;
    in-out property<[length]> fixed-height-model;

    VerticalLayout {
      alignment: start;

      layout := VerticalLayout {
        for fixed-height in fixed-height-model: Rectangle {
            background: blue;
            height: fixed-height;
        }
      }
    }

  }
    """,
        Path(""),
    ).component("App")
    assert compdef is not None

    instance = compdef.create()
    assert instance is not None

    model = models.ListModel([100, 0])

    instance.set_property("fixed-height-model", model)
    instance._process_pending_events()

    assert instance.get_property("layout-height") == 100
    model.set_row_data(1, 50)
    assert instance.get_property("layout-height") == 150
    model[-1] = 25
    assert instance.get_property("layout-height") == 125
    model[-1] = 50
    assert instance.get_property("layout-height") == 150
    model.push_row(75)
    instance._process_pending_events()
    assert instance.get_property("layout-height") == 225
    del model[1:]
    instance._process_pending_events()
    assert instance.get_property("layout-height") == 100

    assert isinstance(instance.get_property("fixed-height-model"), models.ListModel)


def test_model_from_list() -> None:
    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
  export component App {
    in-out property<[int]> data: [1, 2, 3, 4];
  }
    """,
        Path(""),
    ).component("App")
    assert compdef is not None

    instance = compdef.create()
    assert instance is not None

    model = instance.get_property("data")
    assert model.row_count() == 4
    assert model.row_data(2) == 3

    instance.set_property("data", models.ListModel([0]))
    instance.set_property("data", model)
    assert list(instance.get_property("data")) == [1, 2, 3, 4]


def test_python_model_sequence() -> None:
    model = models.ListModel([1, 2, 3, 4, 5])

    assert len(model) == 5
    assert list(model) == [1, 2, 3, 4, 5]
    model[0] = 100
    assert list(model) == [100, 2, 3, 4, 5]
    model[-1] = 500
    assert list(model) == [100, 2, 3, 4, 500]
    assert model[2] == 3


def test_list_model_insert() -> None:
    model = models.ListModel([1, 2, 3])
    model.insert(0, 0)
    assert list(model) == [0, 1, 2, 3]
    model.insert(2, 99)
    assert list(model) == [0, 1, 99, 2, 3]
    model.insert(len(model), 100)
    assert list(model) == [0, 1, 99, 2, 3, 100]


def test_list_model_insert_clamps() -> None:
    model = models.ListModel([1, 2, 3])
    model.insert(-5, 7)
    assert list(model) == [7, 1, 2, 3]
    model.insert(100, 8)
    assert list(model) == [7, 1, 2, 3, 8]


def test_list_model_insert_into_empty() -> None:
    model: models.ListModel[int] = models.ListModel()
    model.insert(0, 42)
    assert list(model) == [42]


def test_list_model_insert_notifies() -> None:
    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
  export component App {
    width: 300px;
    height: 300px;

    out property<length> layout-height: layout.height;
    in-out property<[length]> fixed-height-model;

    VerticalLayout {
      alignment: start;

      layout := VerticalLayout {
        for fixed-height in fixed-height-model: Rectangle {
            background: blue;
            height: fixed-height;
        }
      }
    }
  }
    """,
        Path(""),
    ).component("App")
    assert compdef is not None

    instance = compdef.create()
    assert instance is not None

    model = models.ListModel([100, 50])
    instance.set_property("fixed-height-model", model)
    instance._process_pending_events()
    assert instance.get_property("layout-height") == 150

    model.insert(0, 25)
    instance._process_pending_events()
    assert instance.get_property("layout-height") == 175

    model.insert(len(model), 10)
    instance._process_pending_events()
    assert instance.get_property("layout-height") == 185


def test_list_model_delete() -> None:
    model = models.ListModel([1, 10, 100, 1000, 10000])
    del model[-1]
    assert list(model) == [1, 10, 100, 1000]
    del model[::2]
    assert list(model) == [10, 1000]
    del model[::-1]
    assert list(model) == []


def test_list_model_delete_notifies() -> None:
    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
  export component App {
    width: 300px;
    height: 300px;

    out property<length> layout-height: layout.height;
    in-out property<[length]> fixed-height-model;

    VerticalLayout {
      alignment: start;

      layout := VerticalLayout {
        for fixed-height in fixed-height-model: Rectangle {
            background: blue;
            height: fixed-height;
        }
      }
    }
  }
    """,
        Path(""),
    ).component("App")
    assert compdef is not None

    cases: list[tuple[int | slice, list[int]]] = [
        (-1, [1, 10, 100, 1000]),
        (3, [1, 10, 100, 10000]),
        (slice(1, 3), [1, 1000, 10000]),
        (slice(None, None, 2), [10, 1000]),
        (slice(1, None, 2), [1, 100, 10000]),
        (slice(None, None, -2), [10, 1000]),
        (slice(3, None, -2), [1, 100, 10000]),
        (slice(None, None, -1), []),
    ]

    for key, expected in cases:
        instance = compdef.create()
        assert instance is not None

        model = models.ListModel([1, 10, 100, 1000, 10000])
        instance.set_property("fixed-height-model", model)
        instance._process_pending_events()
        assert instance.get_property("layout-height") == 11111

        del model[key]
        instance._process_pending_events()
        assert list(model) == expected
        assert instance.get_property("layout-height") == sum(expected)


def test_python_model_iterable() -> None:
    def test_generator(max: int) -> typing.Iterator[int]:
        i = 0
        while i < max:
            yield i
            i += 1

    model = models.ListModel(test_generator(5))

    assert len(model) == 5
    assert list(model) == [0, 1, 2, 3, 4]


def test_rust_model_sequence() -> None:
    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
  export component App {
    in-out property<[int]> data: [1, 2, 3, 4, 5];
  }
    """,
        Path(""),
    ).component("App")
    assert compdef is not None

    instance = compdef.create()
    assert instance is not None

    model = instance.get_property("data")

    assert len(model) == 5
    assert list(model) == [1, 2, 3, 4, 5]
    assert model[2] == 3


def test_model_writeback() -> None:
    compiler = native.Compiler()

    compdef = compiler.build_from_source(
        """
  export component App {
    width: 300px;
    height: 300px;

    in-out property<[int]> model;
    callback write-to-model(int, int);
    write-to-model(index, value) => {
        self.model[index] = value
    }

  }
    """,
        Path(""),
    ).component("App")
    assert compdef is not None

    instance = compdef.create()
    assert instance is not None

    model = models.ListModel([100, 0])

    instance.set_property("model", model)

    instance.invoke("write-to-model", 1, 42)
    assert list(instance.get_property("model")) == [100, 42]
    instance.invoke("write-to-model", 0, 25)
    assert list(instance.get_property("model")) == [25, 42]


def test_model_modifications() -> None:
    compiler = native.Compiler()
    compdef = compiler.build_from_source(
        """
        export component App {
            in-out property<[int]> ints;
            in-out property<[int]> empty-ints;
            public function push-one(value: int) { ints.push(value) }
            public function remove-one(index: int) { ints.remove(index) }
            public function insert-one(index: int, value: int) { ints.insert(index, value) }
            public function push-one-empty() { empty-ints.push(0) }
            public function remove-one-empty() { empty-ints.remove(0) }
            public function insert-one-empty() { empty-ints.insert(0, 0) }
        }
        """,
        Path(""),
    ).component("App")

    assert compdef is not None

    instance = compdef.create()
    assert instance is not None

    model = models.ListModel([1, 2, 3])
    instance.set_property("ints", model)

    assert len(model) == 3

    instance.invoke("push-one", 10)
    assert len(model) == 4
    assert model[3] == 10

    instance.invoke("remove-one", 1)
    assert len(model) == 3
    assert model[2] == 10

    instance.invoke("insert-one", 1, 20)
    assert len(model) == 4
    assert model[1] == 20

    instance.invoke("remove_one", -1)
    assert len(model) == 4
    instance.invoke("remove_one", 10)
    assert len(model) == 4

    instance.invoke("insert_one", -1, 30)
    assert len(model) == 4
    instance.invoke("insert_one", 10, 30)
    assert len(model) == 4

    model = instance.get_property("empty_ints")
    assert len(model) == 0
    instance.invoke("push_one_empty", 1)
    assert len(model) == 0
    instance.invoke("remove_one_empty")
    assert len(model) == 0
    instance.invoke("insert_one_empty")
    assert len(model) == 0


def test_list_model_append_alias() -> None:
    model = models.ListModel([1, 2])
    model.append(3)
    assert list(model) == [1, 2, 3]


def test_map_model() -> None:
    source = models.ListModel([1, 2, 3])
    mapped = models.MapModel(source, lambda value: value * 10)

    assert mapped.source_model is source
    assert list(mapped) == [10, 20, 30]
    assert mapped[1] == 20
    assert mapped.row_data(3) is None

    source[1] = 5
    source.append(4)
    del source[0]
    assert list(mapped) == [50, 30, 40]


def test_map_model_of_map_model() -> None:
    source = models.ListModel([1, 2])
    mapped = models.MapModel(
        models.MapModel(source, lambda value: value + 1), lambda value: str(value)
    )
    assert list(mapped) == ["2", "3"]


def test_map_model_notifies() -> None:
    compiler = native.Compiler()
    compdef = compiler.build_from_source(
        """
        export component App {
            in property<[string]> texts;
            out property<string> joined: texts[0] + texts[1] + texts[2];
            out property<int> count: texts.length;
        }
        """,
        Path(""),
    ).component("App")
    assert compdef is not None
    instance = compdef.create()
    assert instance is not None

    source = models.ListModel([1, 2, 3])
    instance.set_property("texts", models.MapModel(source, lambda value: str(value)))
    assert instance.get_property("joined") == "123"

    source[1] = 5
    assert instance.get_property("joined") == "153"
    source.insert(0, 0)
    assert instance.get_property("joined") == "015"
    assert instance.get_property("count") == 4
    del source[0]
    assert instance.get_property("count") == 3


def test_map_model_subclass_notifies() -> None:
    compiler = native.Compiler()
    compdef = compiler.build_from_source(
        """
        export component App {
            in property<[int]> values;
            out property<int> first: values[0];
        }
        """,
        Path(""),
    ).component("App")
    assert compdef is not None
    instance = compdef.create()
    assert instance is not None

    class ScaledModel(models.MapModel[int, int]):
        def __init__(self, source: models.Model[int]) -> None:
            super().__init__(source)
            self.factor = 2

        def map_row(self, row_data: int) -> int:
            return row_data * self.factor

    mapped = ScaledModel(models.ListModel([1, 2]))
    instance.set_property("values", mapped)
    assert instance.get_property("first") == 2

    mapped.factor = 3
    mapped.notify_row_changed(0)
    assert instance.get_property("first") == 3


def test_map_model_of_slint_model() -> None:
    compiler = native.Compiler()
    compdef = compiler.build_from_source(
        """
        export component App {
            in-out property<[int]> data: [1, 2, 3];
        }
        """,
        Path(""),
    ).component("App")
    assert compdef is not None
    instance = compdef.create()
    assert instance is not None

    mapped = models.MapModel(instance.get_property("data"), lambda value: -value)
    assert list(mapped) == [-1, -2, -3]


def test_map_model_function_exception() -> None:
    def fail(value: int) -> int:
        raise ValueError(f"cannot map {value}")

    mapped = models.MapModel(models.ListModel([1]), fail)
    with pytest.raises(ValueError, match="cannot map 1"):
        mapped.row_data(0)
    assert mapped.row_count() == 1


def test_map_model_source_exception() -> None:
    class Failing(models.Model[int]):
        def row_count(self) -> int:
            raise RuntimeError("no count")

        def row_data(self, row: int) -> int | None:
            return None

    mapped = models.MapModel(Failing(), lambda value: value)
    with pytest.raises(RuntimeError, match="no count"):
        mapped.row_count()


def test_map_model_rejects_non_model_source() -> None:
    with pytest.raises(TypeError):
        models.MapModel(typing.cast(models.Model[int], [1, 2]), lambda value: value)


def test_map_model_function_takes_precedence_over_map_row() -> None:
    class Negated(models.MapModel[int, int]):
        def map_row(self, row_data: int) -> int:
            return -row_data

    assert list(Negated(models.ListModel([1, 2]))) == [-1, -2]
    assert list(Negated(models.ListModel([1, 2]), lambda value: value * 2)) == [2, 4]


def test_map_model_requires_map_function_or_map_row() -> None:
    with pytest.raises(TypeError, match="map_row"):
        models.MapModel(models.ListModel([1]))


def test_map_model_skips_missing_source_rows() -> None:
    class Sparse(models.Model[int]):
        def row_count(self) -> int:
            return 2

        def row_data(self, row: int) -> int | None:
            return 1 if row == 0 else None

    mapped = models.MapModel(Sparse(), lambda value: value * 10)
    assert mapped.row_data(0) == 10
    assert mapped.row_data(1) is None
    assert mapped.row_data(5) is None


def test_map_model_negative_index() -> None:
    mapped = models.MapModel(models.ListModel([1, 2, 3]), lambda value: value * 10)
    assert mapped[-1] == 30
    assert mapped.row_data(-3) == 10
    assert mapped.row_data(-4) is None
