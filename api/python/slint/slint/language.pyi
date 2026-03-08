# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import typing

class StandardListViewItem(typing.NamedTuple):
    """Represents an item in a StandardListView and a StandardTableView."""

    text: str = ""
    """The text content of the item"""

class KeyboardModifiers(typing.NamedTuple):
    """Indicates which modifier keys are pressed during an event.

    On macOS, the Command key (⌘) is mapped to ``control`` and the Control key is mapped to ``meta``.
    On Windows, the Windows key is mapped to ``meta``.
    """

    shift: bool = False
    """Indicates the Shift key on a keyboard."""
    control: bool = False
    """Indicates the Control key on a keyboard, except on macOS, where it is the Command key (⌘)."""
    alt: bool = False
    """Indicates the Alt key on a keyboard."""
    meta: bool = False
    """Indicates the Control key on macOS, and the Windows key on Windows."""
