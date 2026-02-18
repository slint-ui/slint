# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest
from slint import KeyboardModifiers, StandardListViewItem


NAMED_TUPLES = [
    (
        KeyboardModifiers,
        {"alt": False, "control": False, "shift": False, "meta": False},
    ),
    (StandardListViewItem, {"text": ""}),
]


@pytest.fixture(params=NAMED_TUPLES, ids=lambda t: t[0].__name__)
def named_tuple_info(request):
    return request.param


class TestNamedTupleGeneric:
    def test_is_tuple_subclass(self, named_tuple_info):
        cls, _ = named_tuple_info
        assert issubclass(cls, tuple)

    def test_default_values(self, named_tuple_info):
        cls, defaults = named_tuple_info
        instance = cls()
        for field, expected in defaults.items():
            assert getattr(instance, field) == expected

    def test_instance_is_tuple(self, named_tuple_info):
        cls, _ = named_tuple_info
        assert isinstance(cls(), tuple)

    def test_asdict(self, named_tuple_info):
        cls, defaults = named_tuple_info
        d = cls()._asdict()
        assert isinstance(d, dict)
        for field in defaults:
            assert field in d

    def test_has_docstring(self, named_tuple_info):
        cls, _ = named_tuple_info
        assert cls.__doc__ is not None
        assert len(cls.__doc__) > 0

    def test_fields_attribute(self, named_tuple_info):
        cls, defaults = named_tuple_info
        assert hasattr(cls, "_fields")
        assert set(cls._fields) == set(defaults.keys())

    def test_keyword_init(self, named_tuple_info):
        cls, defaults = named_tuple_info
        first_field = next(iter(defaults))
        instance = cls(**{first_field: defaults[first_field]})
        assert getattr(instance, first_field) == defaults[first_field]

    def test_replace(self, named_tuple_info):
        cls, defaults = named_tuple_info
        instance = cls()
        first_field = next(iter(defaults))
        replaced = instance._replace(**{first_field: defaults[first_field]})
        assert getattr(replaced, first_field) == defaults[first_field]
