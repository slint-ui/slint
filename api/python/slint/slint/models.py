# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import typing
from abc import ABC
from collections.abc import Iterable
from typing import Any, Iterator

from . import core


class Model[T](core.PyModelBase, ABC, Iterable[T]):
    """Model is the base class for feeding dynamic data into Slint views.

    Subclass Model to implement your own models, or use `ListModel` to wrap a list.

    Models are iterable and can be used in for loops."""

    def __new__(cls, *args: Any) -> "Model[T]":
        return super().__new__(cls)

    def __init__(self) -> None:
        self.init_self(self)

    def __len__(self) -> int:
        return self.row_count()

    def __getitem__(self, index: int) -> typing.Optional[T]:
        return self.row_data(index)

    def __setitem__(self, index: int, value: T) -> None:
        self.set_row_data(index, value)

    def __iter__(self) -> Iterator[T]:
        return ModelIterator(self)


class ListModel[T](Model[T]):
    """ListModel is a `Model` that stores its data in a Python list.

    Construct a ListMode from an iterable (such as a list itself).
    Use `ListModel.append()` to add items to the model, and use the
    `del` statement to remove items.

    Any changes to the model are automatically reflected in the views
    in UI they're used with.
    """

    def __init__(self, iterable: typing.Optional[Iterable[T]] = None) -> None:
        """Constructs a new ListModel from the give iterable. All the values
        the iterable produces are stored in a list."""

        super().__init__()
        items = list(iterable) if iterable is not None else []
        self.list: list[T] = items

    def row_count(self) -> int:
        return len(self.list)

    def row_data(self, row: int) -> T:
        return self.list[row]

    def set_row_data(self, row: int, data: T) -> None:
        self.list[row] = data
        super().notify_row_changed(row)

    def __delitem__(self, key: int | slice) -> None:
        if isinstance(key, slice):
            start, stop, step = key.indices(len(self.list))
            del self.list[key]
            count = len(range(start, stop, step))
            super().notify_row_removed(start, count)
        else:
            del self.list[key]
            super().notify_row_removed(key, 1)

    def append(self, value: T) -> None:
        """Appends the value to the end of the list."""
        index = len(self.list)
        self.list.append(value)
        super().notify_row_added(index, 1)


class ModelIterator[T](Iterator[T]):
    def __init__(self, model: Model[T]) -> None:
        self.model = model
        self.index = 0

    def __iter__(self) -> "ModelIterator[T]":
        return self

    def __next__(self) -> T:
        if self.index >= self.model.row_count():
            raise StopIteration()
        index = self.index
        self.index += 1
        return self.model.row_data(index)  # type: ignore
