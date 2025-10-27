# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

from slint.core import (
    KeyboardModifiers,
    PointerEvent,
    PointerEventButton,
    PointerEventKind,
)


def test_keyboard_modifiers_ctor() -> None:
    mods = KeyboardModifiers(control=True)
    assert mods.control is True
    assert mods.alt is False


def test_pointer_event_ctor_returns_struct() -> None:
    mods = KeyboardModifiers(alt=True)
    event = PointerEvent(
        button=PointerEventButton.left,
        kind=PointerEventKind.down,
        modifiers=mods,
    )

    assert event.button == PointerEventButton.left
    assert event.kind == PointerEventKind.down
    assert event.modifiers.alt is True
