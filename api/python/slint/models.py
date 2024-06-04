# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from . import slint as native


class Model(native.PyModelBase):
    def __new__(cls, *args):
        return super().__new__(cls)

    def __init__(self, lst=None):
        self.init_self(self)

    def __len__(self):
        return self.row_count()

    def __getitem__(self, index):
        return self.row_data(index)

    def __setitem__(self, index, value):
        self.set_row_data(index, value)

    def __iter__(self):
        return ModelIterator(self)


class ListModel(Model):
    def __init__(self, iterable=None):
        super().__init__()
        if iterable is not None:
            self.list = list(iterable)
        else:
            self.list = []

    def row_count(self):
        return len(self.list)

    def row_data(self, row):
        return self.list[row]

    def set_row_data(self, row, data):
        self.list[row] = data
        super().notify_row_changed(row)

    def __delitem__(self, key):
        if isinstance(key, slice):
            start, stop, step = key.indices(len(self.list))
            del self.list[key]
            count = len(range(start, stop, step))
            super().notify_row_removed(start, count)
        else:
            del self.list[key]
            super().notify_row_removed(key, 1)

    def append(self, value):
        index = len(self.list)
        self.list.append(value)
        super().notify_row_added(index, 1)


class ModelIterator:
    def __init__(self, model):
        self.model = model
        self.index = 0

    def __iter__(self):
        return self

    def __next__(self):
        if self.index >= self.model.row_count():
            raise StopIteration()
        index = self.index
        self.index += 1
        return self.model.row_data(index)
