# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import typing

class KeyboardModifiers(typing.NamedTuple):
    """
    The `KeyboardModifiers` struct provides booleans to indicate possible modifier keys on a keyboard, such as Shift, Control, etc.
     It is provided as part of `KeyEvent`'s `modifiers` field.

     Keyboard shortcuts on Apple platforms typically use the Command key (⌘), such as Command+C for "Copy". On other platforms
     the same shortcut is typically represented using Control+C. To make it easier to develop cross-platform applications, on macOS,
     Slint maps the Command key to the control modifier, and the Control key to the meta modifier.

     On Windows, the Windows key is mapped to the meta modifier.
    """

    alt: bool = False
    """
    Indicates the Alt key on a keyboard.
    """
    control: bool = False
    """
    Indicates the Control key on a keyboard, except on macOS, where it is the Command key (⌘).
    """
    shift: bool = False
    """
    Indicates the Shift key on a keyboard.
    """
    meta: bool = False
    """
    Indicates the Control key on macos, and the Windows key on Windows.
    """

class StandardListViewItem(typing.NamedTuple):
    """
    Represents an item in a StandardListView and a StandardTableView.
    """

    text: str = ""
    """
    The text content of the item
    """
