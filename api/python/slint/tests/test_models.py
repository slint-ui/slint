# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import typing
from pathlib import Path

from slint import models
from slint import slint as native


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
    model.append(75)
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
