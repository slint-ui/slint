# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import pytest
from slint.language import StandardListViewItem, KeyEvent, KeyboardModifiers


NAMED_TUPLES = [
    (StandardListViewItem, {"text": ""}),
    (KeyEvent, {"text": "", "modifiers": None, "repeat": False}),
    (
        KeyboardModifiers,
        {"shift": False, "control": False, "alt": False, "meta": False},
    ),
]


@pytest.fixture(params=NAMED_TUPLES, ids=lambda t: t[0].__name__)
def named_tuple_info(request):
    return request.param


def test_is_tuple_subclass(named_tuple_info):
    cls, _ = named_tuple_info
    assert issubclass(cls, tuple)


def test_default_values(named_tuple_info):
    cls, defaults = named_tuple_info
    instance = cls()
    for field, expected in defaults.items():
        assert getattr(instance, field) == expected


def test_instance_is_tuple(named_tuple_info):
    cls, _ = named_tuple_info
    assert isinstance(cls(), tuple)


def test_asdict(named_tuple_info):
    cls, defaults = named_tuple_info
    d = cls()._asdict()
    assert isinstance(d, dict)
    for field in defaults:
        assert field in d


def test_has_docstring(named_tuple_info):
    cls, _ = named_tuple_info
    assert cls.__doc__ is not None
    assert len(cls.__doc__) > 0


def test_fields_attribute(named_tuple_info):
    cls, defaults = named_tuple_info
    assert hasattr(cls, "_fields")
    assert set(cls._fields) == set(defaults.keys())


def test_keyword_init(named_tuple_info):
    cls, defaults = named_tuple_info
    first_field = next(iter(defaults))
    instance = cls(**{first_field: defaults[first_field]})
    assert getattr(instance, first_field) == defaults[first_field]


def test_replace(named_tuple_info):
    cls, defaults = named_tuple_info
    instance = cls()
    first_field = next(iter(defaults))
    replaced = instance._replace(**{first_field: defaults[first_field]})
    assert getattr(replaced, first_field) == defaults[first_field]


def test_StandardListViewItem() -> None:
    item = StandardListViewItem()
    assert item.text == ""
    item = item._replace(text="Test")
    assert item.text == "Test"


def test_KeyboardModifiers() -> None:
    # Test initialization with default values
    mods = KeyboardModifiers()
    assert mods.shift is False
    assert mods.control is False
    assert mods.alt is False
    assert mods.meta is False

    # Test initialization with arguments
    mods = KeyboardModifiers(shift=True, control=True, alt=True, meta=True)
    assert mods.shift is True
    assert mods.control is True
    assert mods.alt is True
    assert mods.meta is True

    # Test setters (_replace for NamedTuple)
    mods = mods._replace(shift=False)
    assert mods.shift is False
    mods = mods._replace(control=False)
    assert mods.control is False
    mods = mods._replace(alt=False)
    assert mods.alt is False
    mods = mods._replace(meta=False)
    assert mods.meta is False

    # Test equality
    mods2 = KeyboardModifiers()
    assert mods == mods2
    mods3 = mods2._replace(shift=True)
    assert mods2 != mods3
