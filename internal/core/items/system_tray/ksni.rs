// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Linux system tray backend using the `ksni` crate (StatusNotifierItem/AppIndicator).

use super::{Error, Params};
use crate::SharedVector;
use crate::api::invoke_from_event_loop;
use crate::items::MenuEntry;
use crate::menus::MenuVTable;
use ::ksni::blocking::TrayMethods;

/// Plain-data mirror of the menu tree. `ksni::MenuItem` isn't `Clone` (the
/// `activate` callback is a `Box<dyn Fn>`), so we cache this intermediate
/// representation and rebuild fresh `MenuItem`s each time ksni calls `menu()`.
enum MenuNode {
    Separator,
    SubMenu { label: std::string::String, enabled: bool, children: std::vec::Vec<MenuNode> },
    Standard { label: std::string::String, enabled: bool, tray_id: u64, entry_index: usize },
}

struct KsniTray {
    icon: ::ksni::Icon,
    title: std::string::String,
    menu: std::vec::Vec<MenuNode>,
}

impl ::ksni::Tray for KsniTray {
    fn id(&self) -> std::string::String {
        // This cannot be empty.
        "slint-tray".into()
    }

    fn title(&self) -> std::string::String {
        self.title.clone()
    }

    fn icon_pixmap(&self) -> std::vec::Vec<::ksni::Icon> {
        std::vec![self.icon.clone()]
    }

    fn menu(&self) -> std::vec::Vec<::ksni::MenuItem<KsniTray>> {
        self.menu.iter().map(node_to_ksni).collect()
    }
}

fn node_to_ksni(node: &MenuNode) -> ::ksni::MenuItem<KsniTray> {
    match node {
        MenuNode::Separator => ::ksni::MenuItem::Separator,
        MenuNode::SubMenu { label, enabled, children } => ::ksni::menu::SubMenu {
            label: label.clone(),
            enabled: *enabled,
            submenu: children.iter().map(node_to_ksni).collect(),
            ..Default::default()
        }
        .into(),
        MenuNode::Standard { label, enabled, tray_id, entry_index } => {
            let tray_id = *tray_id;
            let entry_index = *entry_index;
            ::ksni::menu::StandardItem {
                label: label.clone(),
                enabled: *enabled,
                activate: std::boxed::Box::new(move |_tray: &mut KsniTray| {
                    // ksni requires the callback to be `Send`; hop to the Slint event loop
                    // so the click can be resolved against the (`!Send`) tray registry.
                    let _ = invoke_from_event_loop(move || {
                        super::activate_tray_menu_entry(tray_id, entry_index);
                    });
                }),
                ..Default::default()
            }
            .into()
        }
    }
}

pub struct PlatformTray {
    handle: ::ksni::blocking::Handle<KsniTray>,
}

impl PlatformTray {
    pub fn new(params: Params) -> Result<Self, Error> {
        let pixel_buffer = params.icon.to_rgba8().ok_or(Error::Rgba8)?;

        let mut data = pixel_buffer.as_bytes().to_vec();
        let width = pixel_buffer.width() as i32;
        let height = pixel_buffer.height() as i32;

        for pixel in data.chunks_exact_mut(4) {
            pixel.rotate_right(1) // rgba to argb
        }

        let tray = KsniTray {
            icon: ::ksni::Icon { width, height, data },
            title: params.title.into(),
            menu: std::vec::Vec::new(),
        };

        // Blocks briefly on D-Bus name claim / service setup, then spawns the
        // service loop on its own background thread. Returning the handle
        // synchronously eliminates the pending-menu race an async spawn would
        // otherwise create.
        let handle = tray.spawn().map_err(|e| {
            Error::PlatformError(crate::platform::PlatformError::Other(std::format!(
                "Failed to spawn ksni tray: {e}"
            )))
        })?;
        Ok(Self { handle })
    }

    pub fn rebuild_menu(
        &self,
        menu: vtable::VRef<'_, MenuVTable>,
        tray_id: u64,
        entries_out: &mut std::vec::Vec<MenuEntry>,
    ) {
        entries_out.clear();
        let new_menu = build_ksni_menu(menu, tray_id, entries_out);
        self.handle.update(move |tray: &mut KsniTray| {
            tray.menu = new_menu;
        });
    }
}

fn build_ksni_menu(
    menu: vtable::VRef<'_, MenuVTable>,
    tray_id: u64,
    entries_out: &mut std::vec::Vec<MenuEntry>,
) -> std::vec::Vec<MenuNode> {
    let mut top = SharedVector::<MenuEntry>::default();
    menu.sub_menu(None, &mut top);
    top.iter().map(|entry| entry_to_node(menu, entry, 0, tray_id, entries_out)).collect()
}

fn entry_to_node(
    menu: vtable::VRef<'_, MenuVTable>,
    entry: &MenuEntry,
    depth: usize,
    tray_id: u64,
    entries_out: &mut std::vec::Vec<MenuEntry>,
) -> MenuNode {
    if entry.is_separator {
        return MenuNode::Separator;
    }

    // Mirror muda.rs's depth cap to protect against accidental infinite menu trees.
    const MAX_DEPTH: usize = 15;

    if entry.has_sub_menu && depth < MAX_DEPTH {
        let mut children = SharedVector::<MenuEntry>::default();
        menu.sub_menu(Some(entry), &mut children);
        MenuNode::SubMenu {
            label: entry.title.as_str().into(),
            enabled: entry.enabled,
            children: children
                .iter()
                .map(|c| entry_to_node(menu, c, depth + 1, tray_id, entries_out))
                .collect(),
        }
    } else {
        let entry_index = entries_out.len();
        entries_out.push(entry.clone());
        MenuNode::Standard {
            label: entry.title.as_str().into(),
            enabled: entry.enabled,
            tray_id,
            entry_index,
        }
    }
}
