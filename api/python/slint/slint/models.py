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

    def __new__(cls, *args: Any, **kwargs: Any) -> typing.Self:
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


class _AdapterModel[T](Model[T]):
    """The base class of the models that wrap a model adapter of the Rust core library."""

    _adapter: native.PyModelAdapter

    def _row_index(self, row: int) -> int:
        return row + self.row_count() if row < 0 else row

    def row_count(self) -> int:
        return self._adapter.row_count()

    def row_data(self, row: int) -> T | None:
        row = self._row_index(row)
        if row < 0:
            return None
        return typing.cast(T | None, self._adapter.row_data(row))


class MapModel[T, U](_AdapterModel[U]):
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


class _WritableAdapterModel[T](_AdapterModel[T]):
    """The base class of the adapter models whose rows can be set."""

    def set_row_data(self, row: int, value: T) -> None:
        """Sets the row of the source model that corresponds to `row`.
        Raises IndexError if `row` is out of range."""
        index = self._row_index(row)
        if index < 0:
            raise IndexError("row index out of range")
        self._adapter.set_row_data(index, value)


class ReverseModel[T](_WritableAdapterModel[T]):
    """ReverseModel is a `Model` that provides the rows of a source model in
    reverse order.

    The ReverseModel follows the changes of the source model.
    Setting a row sets the corresponding row of the source model.

    ```python
    numbers = slint.ListModel([1, 2, 3])
    reversed_numbers = slint.ReverseModel(numbers)
    assert list(reversed_numbers) == [3, 2, 1]
    ```
    """

    def __init__(self, source_model: Model[T]):
        """Constructs a new ReverseModel that provides the rows of `source_model`
        in reverse order."""
        super().__init__()
        self.source_model = source_model
        self._adapter = native.PyModelAdapter.reverse(source_model, self)


class FilterModel[T](_WritableAdapterModel[T]):
    """FilterModel is a `Model` that provides the rows of a source model for
    which a filter function returns true.

    The FilterModel follows the changes of the source model.
    Setting a row sets the corresponding row of the source model.

    ```python
    numbers = slint.ListModel([1, 2, 3, 4])
    even_numbers = slint.FilterModel(numbers, lambda n: n % 2 == 0)
    assert list(even_numbers) == [2, 4]
    ```

    Alternatively, subclass FilterModel and implement `filter_row`.
    Call `reset` when the result of the filter changes for reasons other than
    a change of the source model:

    ```python
    class Search(slint.FilterModel[str]):
        def __init__(self, source: slint.Model[str]) -> None:
            self.text = ""
            super().__init__(source)

        def filter_row(self, row_data: str) -> bool:
            return self.text in row_data

    search = Search(slint.ListModel(["Hans", "Max", "Roman"]))
    search.text = "Max"
    search.reset()
    ```
    """

    def __init__(
        self,
        source_model: Model[T],
        filter_function: Callable[[T], bool] | None = None,
    ):
        """Constructs a new FilterModel that provides the rows of `source_model`
        for which the filter returns true.
        Pass `filter_function` to filter the rows, or omit it in a subclass that
        implements `filter_row`.
        The constructor applies the filter to all rows of the source model, so
        a subclass sets the state that `filter_row` uses before calling it."""
        super().__init__()
        self.source_model = source_model
        if filter_function is None:
            if type(self).filter_row is FilterModel.filter_row:
                raise TypeError(
                    "FilterModel requires a filter function or a subclass that implements filter_row()"
                )
            this = weakref.ref(self)

            def filter_function(row_data: T) -> bool:
                model = this()
                assert model is not None
                return model.filter_row(row_data)

        self._adapter = native.PyModelAdapter.filter(
            source_model, filter_function, self
        )

    def filter_row(self, row_data: T) -> bool:
        """Returns true if `row_data`, a row of the source model, is a row of this model.
        Re-implement this method in a sub-class that doesn't pass a filter function
        to the constructor."""
        raise NotImplementedError(
            f"{type(self).__name__} does not implement filter_row()"
        )

    def reset(self) -> None:
        """Applies the filter to all rows of the source model again.
        Call this when the result of the filter changes for reasons other than
        a change of the source model."""
        self._adapter.reset()

    def unfiltered_row(self, row: int) -> int:
        """Returns the index of the row of the source model that is `row` in this model.
        Raises IndexError if `row` is out of range."""
        index = self._row_index(row)
        if index < 0:
            raise IndexError("row index out of range")
        return self._adapter.source_row(index)


class SortModel[T](_WritableAdapterModel[T]):
    """SortModel is a `Model` that provides the rows of a source model in
    sorted order.

    Like `sorted()`, it orders the rows by the result of the `key` function,
    or by the rows themselves without a key, and in descending order with
    `reverse=True`.
    To sort by a comparison function, pass `key=functools.cmp_to_key(compare)`.
    Rows with a float NaN key sort last, followed by rows whose key raises an
    exception, regardless of `reverse`.
    Keys that can't be ordered, such as tuples that contain NaN, raise ValueError.

    The SortModel follows the changes of the source model.
    Setting a row sets the corresponding row of the source model.

    ```python
    names = slint.ListModel(["Max", "Hans", "Roman"])
    sorted_names = slint.SortModel(names)
    assert list(sorted_names) == ["Hans", "Max", "Roman"]
    by_length = slint.SortModel(names, key=len, reverse=True)
    ```

    Alternatively, subclass SortModel and implement `sort_key`.
    Call `reset` when the order changes for reasons other than a change of
    the source model.
    """

    def __init__(
        self,
        source_model: Model[T],
        key: Callable[[T], Any] | None = None,
        reverse: bool = False,
    ):
        """Constructs a new SortModel that provides the rows of `source_model`
        ordered by `key`.
        Omit `key` to order the rows by themselves, or by `sort_key` in a
        subclass that implements it."""
        super().__init__()
        self.source_model = source_model
        if key is None and type(self).sort_key is not SortModel.sort_key:
            this = weakref.ref(self)

            def key(row_data: T) -> Any:
                model = this()
                assert model is not None
                return model.sort_key(row_data)

        self._adapter = native.PyModelAdapter.sort(source_model, key, reverse, self)

    def sort_key(self, row_data: T) -> Any:
        """Returns the value to order `row_data`, a row of the source model, by.
        The default implementation returns `row_data` itself.
        Re-implement this method in a sub-class that doesn't pass a key to the
        constructor."""
        return row_data

    def reset(self) -> None:
        """Sorts all rows of the source model again.
        Call this when the order changes for reasons other than a change of
        the source model."""
        self._adapter.reset()

    def unsorted_row(self, row: int) -> int:
        """Returns the index of the row of the source model that is `row` in this model.
        Raises IndexError if `row` is out of range."""
        index = self._row_index(row)
        if index < 0:
            raise IndexError("row index out of range")
        return self._adapter.source_row(index)


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
