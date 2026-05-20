# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Tests for the built-in DropEvent struct exposed via slint.language."""

from slint.language import DragAction, DropEvent


def test_drop_event_is_a_namedtuple() -> None:
    assert issubclass(DropEvent, tuple)


def test_drop_event_default_construction() -> None:
    e = DropEvent()
    # Boolean fields default to False; reference-typed fields default to None
    # (users receive populated instances from Slint callbacks).
    assert e.allow_copy is False
    assert e.allow_move is False
    assert e.allow_link is False
    assert e.data is None
    assert e.position is None
    assert e.proposed_action is None


def test_drop_event_field_override() -> None:
    e = DropEvent(allow_copy=True, proposed_action=DragAction.copy)
    assert e.allow_copy is True
    assert e.proposed_action is DragAction.copy


def test_drop_event_namedtuple_replace() -> None:
    e = DropEvent()._replace(allow_move=True)
    assert e.allow_move is True
    assert e.allow_copy is False


def test_drop_event_has_docstring() -> None:
    assert DropEvent.__doc__
    assert "DropArea" in DropEvent.__doc__
