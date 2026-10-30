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
        keep_activation_token();
        let _ = self.event_tx.try_send(Event::Activate(x, y));
    }

    // Slint's `tooltip` is the hover text — it goes into SNI `ToolTip.title`,
    // which is a plain-text field per the spec (the `description` slot is the
    // one that accepts HTML markup), so no escaping is applied.
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
                    keep_activation_token();
                    let _ = tx.try_send(Event::Menu(entry_index));
                }),
                ..Default::default()
            }
            .into()
        }
    }
}

/// The activation token of the last click, until a window takes it.
static ACTIVATION_TOKEN: std::sync::Mutex<Option<std::string::String>> =
    std::sync::Mutex::new(None);

/// Keeps the XDG activation token the host sent with the click being handled. KDE sends it
/// through `ProvideXdgActivationToken`, which can arrive just after the click, so this waits a
/// moment for it, as ksni advises. Plasma shows a launch entry on its task bar for each token it
/// hands out, until a window uses that token.
fn keep_activation_token() {
    const WAIT: std::time::Duration = std::time::Duration::from_millis(20);
    const TRY_EVERY: std::time::Duration = std::time::Duration::from_millis(1);
    let start = std::time::Instant::now();
    let token = loop {
        match ::ksni::current_xdg_activation_token() {
            Some(token) => break Some(token),
            None if start.elapsed() >= WAIT => break None,
            None => std::thread::sleep(TRY_EVERY),
        }
    };
    if let Some(token) = token {
        *ACTIVATION_TOKEN.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(token);
    }
}

pub fn take_activation_token() -> Option<std::string::String> {
    ACTIVATION_TOKEN.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take()
}

pub struct PlatformTray {
    // SNI has no hide operation: the only way to make the icon disappear is to
    // shut down the registered service (which deregisters from the watcher), and
    // the only way to bring it back is to spawn a new `KsniTray`. The state needed
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
        let menu = params
            .menu
            .map(|(menu, entries_out)| build_ksni_menu(menu, entries_out))
            .unwrap_or_default();

        let handle =
            spawn_tray(icon.clone(), tooltip.clone(), title.clone(), menu.clone(), &event_tx)?;

        let dispatcher = context
            .spawn_local(dispatch_loop(event_rx, self_weak))
            .map_err(Error::EventLoopError)?;

        Ok(Self {
            icon: core::cell::RefCell::new(icon),
            tooltip: core::cell::RefCell::new(tooltip),
            title: core::cell::RefCell::new(title),
            event_tx,
            menu: core::cell::RefCell::new(menu),
            handle: core::cell::RefCell::new(Some(handle)),
            _dispatcher: dispatcher,
        })
    }

    pub fn rebuild_menu(
        &self,
        menu: vtable::VRef<'_, MenuVTable>,
        entries_out: &mut std::vec::Vec<MenuEntry>,
    ) {
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
                // The service loop runs detached and outlives dropped handles;
                // only an explicit shutdown tears it down so the
                // StatusNotifierWatcher removes the item.
                if let Some(handle) = slot.take() {
                    handle.shutdown();
                }
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

impl Drop for PlatformTray {
    fn drop(&mut self) {
        // The service loop would otherwise keep the icon registered until the
        // process exits.
        self.set_visible(false);
    }
}

fn image_to_argb_icon(image: &Image) -> Result<::ksni::Icon, Error> {
    let pixel_buffer = image.to_rgba8().ok_or(Error::Rgba8)?;
    let mut data = pixel_buffer.as_bytes().to_vec();
    let width = pixel_buffer.width() as i32;
    let height = pixel_buffer.height() as i32;
    for pixel in data.as_chunks_mut::<4>().0 {
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
    // service loop on its own background thread.
    tray.disable_dbus_name(*IN_SANDBOX).spawn().map_err(|e| {
        Error::PlatformError(crate::platform::PlatformError::Other(std::format!(
            "Failed to spawn ksni tray: {e}"
        )))
    })
}

/// Whether the application runs in a Flatpak or Snap sandbox, where owning the D-Bus name
/// the StatusNotifierItem specification asks for needs an extra permission.
/// Matches `ashpd::is_sandboxed()` as of ashpd 0.13.13, without depending on a full portal client.
static IN_SANDBOX: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    std::path::Path::new("/.flatpak-info").exists()
        // A snap runs in a cgroup unit named `snap.*`: the last segment of the cgroup v2 path,
        // or of the v1 `freezer` or `name=systemd` path.
        || std::fs::read_to_string("/proc/self/cgroup").is_ok_and(|cgroups| {
            cgroups.lines().any(|line| {
                let Some((id, rest)) = line.split_once(':') else { return false };
                let Some((controllers, path)) = rest.split_once(':') else { return false };
                let snap_hierarchy = matches!(controllers, "" | "freezer" | "name=systemd");
                let snap_unit =
                    path.rsplit('/').next().is_some_and(|unit| unit.starts_with("snap."));
                id.parse::<u32>().is_ok() && snap_hierarchy && snap_unit
            })
        })
});

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
                tray.clicked.call(&());
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
