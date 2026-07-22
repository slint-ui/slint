// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Fallback backend for builds without a real system tray (the `system-tray`
//! feature is off, or Android, WASM, embedded targets, …). Handle creation
//! fails with [`Error::Unsupported`], which the tray item reports through
//! `debug_log`; the application keeps running without an icon.

use super::{Error, Params};
use crate::graphics::Image;
use crate::item_tree::ItemWeak;
use crate::items::MenuEntry;
use crate::menus::MenuVTable;

pub struct PlatformTray;

impl PlatformTray {
    pub fn new(
        _params: Params,
        _self_weak: ItemWeak,
        _context: &crate::SlintContext,
    ) -> Result<Self, Error> {
        Err(Error::Unsupported)
    }

    pub fn rebuild_menu(
        &self,
        _menu: vtable::VRef<'_, MenuVTable>,
        entries_out: &mut alloc::vec::Vec<MenuEntry>,
    ) {
        entries_out.clear();
    }

    pub fn set_visible(&self, _visible: bool) {}

    pub fn set_icon(&self, _icon: &Image) {}

    pub fn set_tooltip(&self, _tooltip: &str) {}

    pub fn set_title(&self, _title: &str) {}
}
