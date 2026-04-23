// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! macOS and Windows system tray backend using the `tray-icon` crate (muda-based).

use super::{Error, Params};
use crate::SharedVector;
use crate::api::invoke_from_event_loop;
use crate::graphics::Image;
use crate::item_tree::ItemWeak;
use crate::items::MenuEntry;
use crate::menus::MenuVTable;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

std::thread_local! {
    /// Map from `tray_id` to the `SystemTray` item that owns it. muda's global menu
    /// event handler is shared across all trays, so clicks arrive as a bare `MenuId`
    /// and we need this side table to route each one back to the right tray.
    static TRAYS: core::cell::RefCell<std::collections::HashMap<u64, ItemWeak>> =
        core::cell::RefCell::new(std::collections::HashMap::new());
}

fn register_tray(self_weak: ItemWeak) -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    TRAYS.with(|t| {
        t.borrow_mut().insert(id, self_weak);
    });
    id
}

fn unregister_tray(id: u64) {
    TRAYS.with(|t| {
        t.borrow_mut().remove(&id);
    });
}

fn activate_tray_menu_entry(tray_id: u64, entry_index: usize) {
    let Some(item_weak) = TRAYS.with(|t| t.borrow().get(&tray_id).cloned()) else { return };
    let Some(item_rc) = item_weak.upgrade() else { return };
    let Some(tray) = item_rc.downcast::<super::SystemTray>() else { return };
    let tray = tray.as_pin_ref();
    let menu_borrow = tray.data.menu.borrow();
    let Some(state) = menu_borrow.as_ref() else { return };
    if let Some(entry) = state.entries.get(entry_index) {
        vtable::VRc::borrow(&state.menu_vrc).activate(entry);
    }
}

fn install_menu_event_handler() {
    // `muda::MenuEvent::set_event_handler` is install-once (OnceCell); we own
    // tray-icon's muda copy so claiming it from here is safe.
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.load(Ordering::Relaxed) {
        return;
    }
    ::tray_icon::menu::MenuEvent::set_event_handler(Some(|event: ::tray_icon::menu::MenuEvent| {
        // muda delivers events on an arbitrary thread — hop to the Slint event loop.
        let id = event.id().0.clone();
        let _ = invoke_from_event_loop(move || {
            let Some((tid, eid)) = id.split_once('|') else { return };
            let Ok(tray_id) = tid.parse::<u64>() else { return };
            let Ok(entry_index) = eid.parse::<usize>() else { return };
            activate_tray_menu_entry(tray_id, entry_index);
        });
    }));
    INSTALLED.store(true, Ordering::Relaxed);
}

fn icon_to_tray_icon(icon: &Image) -> Result<::tray_icon::Icon, Error> {
    let pixel_buffer = icon.to_rgba8().ok_or(Error::Rgba8)?;

    let rgba = pixel_buffer.as_bytes();
    let width = pixel_buffer.width() as u32;
    let height = pixel_buffer.height() as u32;

    let tray_icon =
        ::tray_icon::Icon::from_rgba(rgba.to_vec(), width, height).map_err(Error::BadIcon)?;

    Ok(tray_icon)
}

pub struct PlatformTray {
    tray_icon: ::tray_icon::TrayIcon,
    tray_id: u64,
}

impl PlatformTray {
    pub fn new(
        params: Params,
        self_weak: ItemWeak,
        _context: &crate::SlintContext,
    ) -> Result<Self, Error> {
        install_menu_event_handler();

        let icon = icon_to_tray_icon(params.icon)?;

        let tray_icon = ::tray_icon::TrayIconBuilder::new()
            .with_icon(icon)
            .with_title(params.title)
            .build()
            .map_err(Error::BuildError)?;

        let tray_id = register_tray(self_weak);

        Ok(Self { tray_icon, tray_id })
    }

    pub fn rebuild_menu(
        &self,
        menu: vtable::VRef<'_, MenuVTable>,
        entries_out: &mut std::vec::Vec<MenuEntry>,
    ) {
        entries_out.clear();
        let muda_menu = build_muda_menu(menu, self.tray_id, entries_out);
        self.tray_icon.set_menu(Some(muda_menu));
    }
}

impl Drop for PlatformTray {
    fn drop(&mut self) {
        unregister_tray(self.tray_id);
    }
}

fn build_muda_menu(
    menu: vtable::VRef<'_, MenuVTable>,
    tray_id: u64,
    entries_out: &mut std::vec::Vec<MenuEntry>,
) -> std::boxed::Box<dyn ::tray_icon::menu::ContextMenu> {
    let root = ::tray_icon::menu::Menu::new();
    let mut top = SharedVector::<MenuEntry>::default();
    menu.sub_menu(None, &mut top);
    for entry in top.iter() {
        let item = entry_to_muda(menu, entry, 0, tray_id, entries_out);
        let _ = root.append(item.as_ref());
    }
    std::boxed::Box::new(root)
}

fn entry_to_muda(
    menu: vtable::VRef<'_, MenuVTable>,
    entry: &MenuEntry,
    depth: usize,
    tray_id: u64,
    entries_out: &mut std::vec::Vec<MenuEntry>,
) -> std::boxed::Box<dyn ::tray_icon::menu::IsMenuItem> {
    use ::tray_icon::menu::{IsMenuItem, MenuId, MenuItem, PredefinedMenuItem, Submenu};

    // Mirror muda.rs's depth cap to protect against accidental infinite menu trees.
    const MAX_DEPTH: usize = 15;

    if entry.is_separator {
        return std::boxed::Box::new(PredefinedMenuItem::separator());
    }

    if entry.has_sub_menu && depth < MAX_DEPTH {
        // Sub-menu headers don't get clicked directly, so they don't need a registry
        // entry. Use a non-numeric second field so the event handler's parse bails.
        let submenu = Submenu::with_id(
            MenuId(std::format!("{tray_id}|submenu")),
            entry.title.as_str(),
            entry.enabled,
        );
        let mut children = SharedVector::<MenuEntry>::default();
        menu.sub_menu(Some(entry), &mut children);
        for child in children.iter() {
            let child_item = entry_to_muda(menu, child, depth + 1, tray_id, entries_out);
            let _ = submenu.append(child_item.as_ref());
        }
        std::boxed::Box::new(submenu) as std::boxed::Box<dyn IsMenuItem>
    } else {
        let entry_index = entries_out.len();
        entries_out.push(entry.clone());
        let id = MenuId(std::format!("{tray_id}|{entry_index}"));
        std::boxed::Box::new(MenuItem::with_id(id, entry.title.as_str(), entry.enabled, None))
    }
}
