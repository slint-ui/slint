// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::items::MenuEntry;
use crate::SharedVector;
use vtable::{VRef, VRefMut};

/// Interface for native menu and menubar
#[vtable::vtable]
#[repr(C)]
pub struct MenuVTable {
    /// destructor
    drop: fn(VRefMut<MenuVTable>),
    /// Return the list of items for the sub menu (or the main menu of parent is None)
    sub_menu: fn(VRef<MenuVTable>, Option<&MenuEntry>, &mut SharedVector<MenuEntry>),
    /// Handler when the menu entry is activated
    activate: fn(VRef<MenuVTable>, &MenuEntry),
}
