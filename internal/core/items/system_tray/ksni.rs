// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Linux system tray backend using the `ksni` crate (StatusNotifierItem/AppIndicator).

use super::{Error, Params};
use crate::SharedVector;
use crate::graphics::Image;
use crate::items::MenuEntry;
use crate::menus::MenuVTable;
use ::ksni::blocking::TrayMethods;

/// Plain-data mirror of the menu tree. `ksni::MenuItem` isn't `Clone` (the
/// `activate` callback is a `Box<dyn Fn>`), so we cache this intermediate
/// representation and rebuild fresh `MenuItem`s each time ksni calls `menu()`.
/// `Clone` is required because the cache lives on `PlatformTray` and is
/// handed to each freshly spawned `KsniTray` when visibility toggles back on.
#[derive(Clone)]
enum MenuNode {
    Separator,
    SubMenu { label: std::string::String, enabled: bool, children: std::vec::Vec<MenuNode> },
    Standard { label: std::string::String, enabled: bool, entry_index: usize },
}

enum Event {
    Activate(i32, i32),
    Menu(usize),
}

struct KsniTray {
    icon: ::ksni::Icon,
    tooltip: std::string::String,
    title: std::string::String,
    menu: std::vec::Vec<MenuNode>,
    event_tx: async_channel::Sender<Event>,
}

impl ::ksni::Tray for KsniTray {
    fn id(&self) -> std::string::String {
        // This cannot be empty.
        "slint-tray".into()
    }

    fn activate(&mut self, x: i32, y: i32) {
        let _ = self.event_tx.try_send(Event::Activate(x, y));
    }

    // Slint's `tooltip` is the hover text — it goes into SNI `ToolTip`.
    fn tool_tip(&self) -> ::ksni::ToolTip {
        ::ksni::ToolTip { title: self.tooltip.clone(), ..Default::default() }
    }

    // Slint's `title` is the descriptive name — it goes into SNI `Title`,
    // which watchers expose for accessibility and overflow listings.
    fn title(&self) -> std::string::String {
        self.title.clone()
    }

    fn icon_pixmap(&self) -> std::vec::Vec<::ksni::Icon> {
        std::vec![self.icon.clone()]
    }

    fn menu(&self) -> std::vec::Vec<::ksni::MenuItem<KsniTray>> {
        self.menu.iter().map(|n| node_to_ksni(n, &self.event_tx)).collect()
    }
}

fn node_to_ksni(
    node: &MenuNode,
    event_tx: &async_channel::Sender<Event>,
) -> ::ksni::MenuItem<KsniTray> {
    match node {
        MenuNode::Separator => ::ksni::MenuItem::Separator,
        MenuNode::SubMenu { label, enabled, children } => ::ksni::menu::SubMenu {
            label: label.clone(),
            enabled: *enabled,
            submenu: children.iter().map(|c| node_to_ksni(c, event_tx)).collect(),
            ..Default::default()
        }
        .into(),
        MenuNode::Standard { label, enabled, entry_index } => {
            let tx = event_tx.clone();
            let entry_index = *entry_index;
            ::ksni::menu::StandardItem {
                label: label.clone(),
                enabled: *enabled,
                activate: std::boxed::Box::new(move |_tray: &mut KsniTray| {
                    // ksni runs the callback on its own thread; the channel hands the
                    // click off to the dispatcher task living on the Slint event loop.
                    let _ = tx.try_send(Event::Menu(entry_index));
                }),
                ..Default::default()
            }
            .into()
        }
    }
}

pub struct PlatformTray {
    // SNI has no hide operation: the only way to make the icon disappear is to
    // drop the registered handle (which deregisters from the watcher), and the
    // only way to bring it back is to spawn a new `KsniTray`. The state needed
    // to rebuild a fresh tray therefore lives on `PlatformTray`, not inside
    // `KsniTray` itself.
    icon: core::cell::RefCell<::ksni::Icon>,
    tooltip: core::cell::RefCell<std::string::String>,
    title: core::cell::RefCell<std::string::String>,
    event_tx: async_channel::Sender<Event>,
    menu: core::cell::RefCell<std::vec::Vec<MenuNode>>,
    handle: core::cell::RefCell<Option<::ksni::blocking::Handle<KsniTray>>>,
    _dispatcher: crate::future::JoinHandle<()>,
}

impl PlatformTray {
    pub fn new(
        params: Params,
        self_weak: crate::item_tree::ItemWeak,
        context: &crate::SlintContext,
    ) -> Result<Self, Error> {
        let icon = image_to_argb_icon(params.icon)?;

        let (event_tx, event_rx) = async_channel::unbounded();
        let tooltip: std::string::String = params.tooltip.into();
        let title: std::string::String = params.title.into();

        let handle = spawn_tray(
            icon.clone(),
            tooltip.clone(),
            title.clone(),
            std::vec::Vec::new(),
            &event_tx,
        )?;

        let dispatcher = context
            .spawn_local(dispatch_loop(event_rx, self_weak))
            .map_err(Error::EventLoopError)?;

        Ok(Self {
            icon: core::cell::RefCell::new(icon),
            tooltip: core::cell::RefCell::new(tooltip),
            title: core::cell::RefCell::new(title),
            event_tx,
            menu: core::cell::RefCell::new(std::vec::Vec::new()),
            handle: core::cell::RefCell::new(Some(handle)),
            _dispatcher: dispatcher,
        })
    }

    pub fn rebuild_menu(
        &self,
        menu: vtable::VRef<'_, MenuVTable>,
        entries_out: &mut std::vec::Vec<MenuEntry>,
    ) {
        entries_out.clear();
        let new_menu = build_ksni_menu(menu, entries_out);
        // Update the cache unconditionally so the next respawn picks up the
        // current menu even if we're hidden right now.
        if let Some(handle) = self.handle.borrow().as_ref() {
            let to_apply = new_menu.clone();
            handle.update(move |tray: &mut KsniTray| {
                tray.menu = to_apply;
            });
        }
        *self.menu.borrow_mut() = new_menu;
    }

    pub fn set_visible(&self, visible: bool) {
        let mut slot = self.handle.borrow_mut();
        match (visible, slot.is_some()) {
            (true, false) => {
                let menu = self.menu.borrow().clone();
                let icon = self.icon.borrow().clone();
                let tooltip = self.tooltip.borrow().clone();
                let title = self.title.borrow().clone();
                match spawn_tray(icon, tooltip, title, menu, &self.event_tx) {
                    Ok(handle) => *slot = Some(handle),
                    Err(_) => {
                        // Leave the slot empty; the next set_visible(true) retries.
                        // Matches the project's existing tolerance for transient
                        // watcher-offline conditions on Linux.
                    }
                }
            }
            (false, true) => {
                // Drop the handle: the ksni service loop tears down and the
                // StatusNotifierWatcher removes the item.
                slot.take();
            }
            _ => {}
        }
    }

    pub fn set_icon(&self, icon: &Image) {
        // If the conversion fails (image with no rgba8 buffer), keep the
        // previous icon. Mirrors the silent-failure behavior elsewhere on
        // this backend; the platform handle is best-effort.
        let Ok(new_icon) = image_to_argb_icon(icon) else { return };
        *self.icon.borrow_mut() = new_icon.clone();
        if let Some(handle) = self.handle.borrow().as_ref() {
            handle.update(move |tray: &mut KsniTray| {
                tray.icon = new_icon;
            });
        }
    }

    pub fn set_tooltip(&self, tooltip: &str) {
        let new_tooltip: std::string::String = tooltip.into();
        *self.tooltip.borrow_mut() = new_tooltip.clone();
        if let Some(handle) = self.handle.borrow().as_ref() {
            handle.update(move |tray: &mut KsniTray| {
                tray.tooltip = new_tooltip;
            });
        }
    }

    pub fn set_title(&self, title: &str) {
        let new_title: std::string::String = title.into();
        *self.title.borrow_mut() = new_title.clone();
        if let Some(handle) = self.handle.borrow().as_ref() {
            handle.update(move |tray: &mut KsniTray| {
                tray.title = new_title;
            });
        }
    }
}

fn image_to_argb_icon(image: &Image) -> Result<::ksni::Icon, Error> {
    let pixel_buffer = image.to_rgba8().ok_or(Error::Rgba8)?;
    let mut data = pixel_buffer.as_bytes().to_vec();
    let width = pixel_buffer.width() as i32;
    let height = pixel_buffer.height() as i32;
    for pixel in data.chunks_exact_mut(4) {
        pixel.rotate_right(1) // rgba to argb
    }
    Ok(::ksni::Icon { width, height, data })
}

fn spawn_tray(
    icon: ::ksni::Icon,
    tooltip: std::string::String,
    title: std::string::String,
    menu: std::vec::Vec<MenuNode>,
    event_tx: &async_channel::Sender<Event>,
) -> Result<::ksni::blocking::Handle<KsniTray>, Error> {
    let tray = KsniTray { icon, tooltip, title, menu, event_tx: event_tx.clone() };
    // Blocks briefly on D-Bus name claim / service setup, then spawns the
    // service loop on its own background thread. Returning the handle
    // synchronously eliminates the pending-menu race an async spawn would
    // otherwise create.
    tray.spawn().map_err(|e| {
        Error::PlatformError(crate::platform::PlatformError::Other(std::format!(
            "Failed to spawn ksni tray: {e}"
        )))
    })
}

async fn dispatch_loop(rx: async_channel::Receiver<Event>, self_weak: crate::item_tree::ItemWeak) {
    while let Ok(event) = rx.recv().await {
        let Some(item_rc) = self_weak.upgrade() else { continue };
        let Some(tray) = item_rc.downcast::<super::SystemTrayIcon>() else { continue };
        let tray = tray.as_pin_ref();

        match event {
            Event::Menu(entry_index) => {
                let menu_borrow = tray.data.menu.borrow();
                let Some(state) = menu_borrow.as_ref() else { continue };
                if let Some(entry) = state.entries.get(entry_index) {
                    vtable::VRc::borrow(&state.menu_vrc).activate(entry);
                }
            }
            Event::Activate(_x, _y) => {
                tray.activated.call(&());
            }
        }
    }
}

fn build_ksni_menu(
    menu: vtable::VRef<'_, MenuVTable>,
    entries_out: &mut std::vec::Vec<MenuEntry>,
) -> std::vec::Vec<MenuNode> {
    let mut top = SharedVector::<MenuEntry>::default();
    menu.sub_menu(None, &mut top);
    top.iter().map(|entry| entry_to_node(menu, entry, 0, entries_out)).collect()
}

fn entry_to_node(
    menu: vtable::VRef<'_, MenuVTable>,
    entry: &MenuEntry,
    depth: usize,
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
                .map(|c| entry_to_node(menu, c, depth + 1, entries_out))
                .collect(),
        }
    } else {
        let entry_index = entries_out.len();
        entries_out.push(entry.clone());
        MenuNode::Standard {
            label: entry.title.as_str().into(),
            enabled: entry.enabled,
            entry_index,
        }
    }
}
