# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import sys
import typing
import weakref
from abc import abstractmethod
from collections.abc import Callable, Iterable, Iterator
from typing import Any

from ._native import native


class Model[T](native.PyModelBase, Iterable[T]):
    """Model is the base class for feeding dynamic data into Slint views.

    Subclass Model to implement your own models, or use `ListModel` to wrap a list.

    Models are iterable and can be used in for loops."""

    def __new__(cls, *args: Any) -> typing.Self:
        return super().__new__(cls)

    def __init__(self) -> None:
        """Kept for backwards compatibility; there is nothing to initialize."""

    def __len__(self) -> int:
        return self.row_count()

    def __getitem__(self, index: int) -> T | None:
        return self.row_data(index)

    def __setitem__(self, index: int, value: T) -> None:
        self.set_row_data(index, value)

    def __iter__(self) -> Iterator[T]:
        return ModelIterator(self)

    def set_row_data(self, row: int, value: T) -> None:
        """Call this method on mutable models to change the data for the given row.
        The UI will also call this method when modifying a model's data.
        Re-implement this method in a sub-class to handle the change."""
        print(
            "set_row_data called on a model which does not re-implement this method. This happens when trying to modify a read-only model",
            file=sys.stderr,
        )

    @abstractmethod
    def row_count(self) -> int:
        """Returns the number of rows in the model.
        Re-implement this method in a sub-class to provide the row count."""
        ...

    @abstractmethod
    def row_data(self, row: int) -> T | None:
        """Returns the data for the given row.
        Re-implement this method in a sub-class to provide the data."""
        ...

    def push_row(self, value: T) -> None:
        """Add a new row to the model with the provided value.
        The default implementation calls `insert_row` with the row count."""
        self.insert_row(self.row_count(), value)

    def remove_row(self, row: int) -> None:
        """Remove the row at the given index.
        Raises an exception when the model rejects the modification.
        The default implementation raises NotImplementedError. A model that
        supports removing rows should also call `notify_row_removed`."""
        raise NotImplementedError(
            f"{type(self).__name__} does not support removing rows"
        )

    def insert_row(self, row: int, value: T) -> None:
        """Insert a new row at the given index.
        Raises an exception when the model rejects the modification.
        The default implementation raises NotImplementedError. A model that
        supports inserting rows should also call `notify_row_added`."""
        raise NotImplementedError(
            f"{type(self).__name__} does not support inserting rows"
        )

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
    Use `ListModel.push_row()`, or its `append` alias, to add items to the
    model, and use the `del` statement to remove items.

    Any changes to the model are automatically reflected in the views
    in UI they're used with.
    """

    def __init__(self, iterable: Iterable[T] | None = None):
        """Constructs a new ListModel from the give iterable. All the values
        the iterable produces are stored in a list."""

        super().__init__()
        self.list: list[T]
        if iterable is not None:
            self.list = list(iterable)
        else:
            self.list = []

    def row_count(self) -> int:
        return len(self.list)

    def row_data(self, row: int) -> T | None:
        return self.list[row]

    def set_row_data(self, row: int, value: T) -> None:
        self.list[row] = value
        super().notify_row_changed(row if row >= 0 else row + len(self.list))

    def remove_row(self, row: int) -> None:
        if row < 0 or row >= len(self.list):
            raise IndexError("row index out of range")
        del self.list[row]
        super().notify_row_removed(row, 1)

    def insert_row(self, row: int, value: T) -> None:
        if row < 0 or row > len(self.list):
            raise IndexError("row index out of range")
        self.insert(row, value)

    def __delitem__(self, key: int | slice) -> None:
        if isinstance(key, slice):
            rows = range(*key.indices(len(self.list)))
            if not rows:
                return
            if abs(rows.step) == 1:
                first = rows.start if rows.step > 0 else rows[-1]
                del self.list[key]
                super().notify_row_removed(first, len(rows))
            else:
                # notify_row_removed describes contiguous rows, so an extended
                # slice needs one notification per row. Remove them highest
                # index first, so the list matches every notification as it goes
                # out and the lower indices stay valid.
                for row in sorted(rows, reverse=True):
                    del self.list[row]
                    super().notify_row_removed(row, 1)
        else:
            row = key if key >= 0 else key + len(self.list)
            del self.list[key]
            super().notify_row_removed(row, 1)

    def push_row(self, value: T) -> None:
        """Appends the value to the end of the list."""
        index = len(self.list)
        self.list.append(value)
        super().notify_row_added(index, 1)

    def append(self, value: T) -> None:
        """Appends the value to the end of the list, like `push_row`."""
        self.push_row(value)

    def insert(self, index: int, value: T) -> None:
        """Inserts the value at the given index. Negative indices and indices
        past the end of the list behave like `list.insert`."""
        clamped = max(0, min(index, len(self.list)))
        self.list.insert(clamped, value)
        super().notify_row_added(clamped, 1)


class MapModel[T, U](Model[U]):
    """MapModel is a read-only `Model` that provides the rows of a source model,
    each passed through a map function.

    The MapModel follows the changes of the source model.

    ```python
    names = slint.ListModel([("Hans", "Emil"), ("Max", "Mustermann")])
    full_names = slint.MapModel(names, lambda name: f"{name[1]}, {name[0]}")
    assert full_names[0] == "Emil, Hans"
    ```

    Alternatively, subclass MapModel and implement `map_row`:

    ```python
    class FullNames(slint.MapModel[tuple[str, str], str]):
        def map_row(self, row_data: tuple[str, str]) -> str:
            return f"{row_data[1]}, {row_data[0]}"

    full_names = FullNames(names)
    ```
    """

    def __init__(
        self,
        source_model: Model[T],
        map_function: Callable[[T], U] | None = None,
    ):
        """Constructs a new MapModel that maps the rows of `source_model` when
        they are read.
        Pass `map_function` to map the rows, or omit it in a subclass that
        implements `map_row`."""
        super().__init__()
        self.source_model = source_model
        if map_function is None:
            if type(self).map_row is MapModel.map_row:
                raise TypeError(
                    "MapModel requires a map function or a subclass that implements map_row()"
                )
            this = weakref.ref(self)

            def map_function(row_data: T) -> U:
                model = this()
                assert model is not None
                return model.map_row(row_data)

        self._adapter = native.PyModelAdapter.map(source_model, map_function, self)

    def map_row(self, row_data: T) -> U:
        """Returns the row of this model for `row_data`, a row of the source model.
        Re-implement this method in a sub-class that doesn't pass a map function
        to the constructor."""
        raise NotImplementedError(f"{type(self).__name__} does not implement map_row()")

    def row_count(self) -> int:
        return self._adapter.row_count()

    def row_data(self, row: int) -> U | None:
        if row < 0:
            row += self.row_count()
            if row < 0:
                return None
        return typing.cast(U | None, self._adapter.row_data(row))


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
