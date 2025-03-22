# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from . import slint as native
from collections.abc import Iterable
from abc import abstractmethod
import typing
from typing import Any, cast, Iterator


class Model[T](native.PyModelBase, Iterable[T]):
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

    def set_row_data(self, row: int, value: T) -> None:
        """Call this method on mutable models to change the data for the given row.
        The UI will also call this method when modifying a model's data.
        Re-implement this method in a sub-class to handle the change."""
        super().set_row_data(row, value)

    @abstractmethod
    def row_data(self, row: int) -> typing.Optional[T]:
        """Returns the data for the given row.
        Re-implement this method in a sub-class to provide the data."""
        return cast(T, super().row_data(row))

    def notify_row_changed(self, row: int) -> None:
        """Call this method from a sub-class to notify the views that a row has changed."""
        super().notify_row_changed(row)

    def notify_row_removed(self, row: int, count: int) -> None:
        """Call this method from a sub-class to notify the views that
        `count` rows have been removed starting at `row`."""
        super().notify_row_removed(row, count)

    def notify_row_added(self, row: int, count: int) -> None:
        """Call this method from a sub-class to notify the views that
        `count` rows have been added starting at `row`."""
        super().notify_row_added(row, count)


class ListModel[T](Model[T]):
    """ListModel is a `Model` that stores its data in a Python list.

    Construct a ListMode from an iterable (such as a list itself).
    Use `ListModel.append()` to add items to the model, and use the
    `del` statement to remove items.

    Any changes to the model are automatically reflected in the views
    in UI they're used with.
    """

    def __init__(self, iterable: typing.Optional[Iterable[T]] = None):
        """Constructs a new ListModel from the give iterable. All the values
        the iterable produces are stored in a list."""

        super().__init__()
        if iterable is not None:
            self.list = list(iterable)
        else:
            self.list = []

    def row_count(self) -> int:
        return len(self.list)

    def row_data(self, row: int) -> typing.Optional[T]:
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
    def __init__(self, model: Model[T]):
        self.model = model
        self.index = 0

    def __iter__(self) -> "ModelIterator[T]":
        return self

    def __next__(self) -> T:
        if self.index >= self.model.row_count():
            raise StopIteration()
        index = self.index
        self.index += 1
        data = self.model.row_data(index)
        assert data is not None
        return data
