// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore nsapp
use super::CustomEvent;
use super::WinitWindowAdapter;
use crate::SlintEvent;
use core::pin::Pin;
use i_slint_core::api::LogicalPosition;
use i_slint_core::items::MenuEntry;
use i_slint_core::menus::MenuVTable;
use i_slint_core::properties::{PropertyDirtyHandler, PropertyTracker};
use id_pool::IdPool;
use itertools::EitherOrBoth;
use itertools::Itertools;
use muda::ContextMenu;
use std::collections::HashMap;
use std::rc::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use winit::event_loop::EventLoopProxy;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

#[derive(Clone, Debug)]
struct MenuNode {
    entry: MenuEntry,
    children: Vec<MenuNode>,
}

pub struct MudaAdapter {
    map: EntryMap<MenuEntry>,
    tracker: Option<Pin<Box<PropertyTracker<false, MudaPropertyTracker>>>>,
    menu: Option<muda::Menu>,
    menu_tree: Vec<MenuNode>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumString, strum::Display)]
pub enum MudaType {
    Menubar,
    Context,
}

static MUDA_SET_EVENT_HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Returns true when `SLINT_NO_MUDA` is set, in which case the menu bar and the context menus
/// are rendered by Slint. That's the only way to exercise these code paths on macOS and Windows.
pub fn is_disabled() -> bool {
    std::env::var_os("SLINT_NO_MUDA").is_some()
}

struct MudaPropertyTracker {
    window_adapter_weak: Weak<WinitWindowAdapter>,
}

impl PropertyDirtyHandler for MudaPropertyTracker {
    fn notify(self: Pin<&Self>) {
        let win = self.window_adapter_weak.clone();
        i_slint_core::timers::Timer::single_shot(Default::default(), move || {
            if let Some(win) = win.upgrade() {
                win.rebuild_menubar();
            }
        })
    }
}

fn build_menu_tree_for_parent(
    menu: vtable::VRef<'_, MenuVTable>,
    parent: Option<&MenuEntry>,
    depth: usize,
) -> Vec<MenuNode> {
    let mut raw_entries = Default::default();
    match parent {
        Some(parent) => menu.sub_menu(Some(parent), &mut raw_entries),
        None => menu.sub_menu(None, &mut raw_entries),
    }

    let mut tree = Vec::new();
    for entry in raw_entries {
        let children = if entry.has_sub_menu && depth < 15 {
            build_menu_tree_for_parent(menu, Some(&entry), depth + 1)
        } else {
            Vec::new()
        };
        tree.push(MenuNode { entry: entry.clone(), children });
    }
    tree
}

fn build_menu_tree(menu: &vtable::VRc<MenuVTable>) -> Vec<MenuNode> {
    build_menu_tree_for_parent(vtable::VRc::borrow(menu), None, 0)
}

fn build_menu_item(node: &MenuNode, id: muda::MenuId) -> Box<dyn muda::IsMenuItem> {
    if node.entry.is_separator {
        Box::new(muda::PredefinedMenuItem::separator())
    } else if !node.entry.has_sub_menu {
        let accelerator = keys_to_accelerator(&node.entry.shortcut);
        let err_handler = |err| {
            i_slint_core::debug_log!(
                "Warning: Could not set accelerator {} for menu item {}: {err}",
                node.entry.shortcut,
                node.entry.title
            )
        };

        if node.entry.checkable {
            let check_menu = muda::CheckMenuItem::with_id(
                id.clone(),
                &node.entry.title,
                node.entry.enabled,
                node.entry.checked,
                None,
            );
            check_menu.set_key_accelerator(accelerator).map_err(err_handler).ok();
            return Box::new(check_menu);
        }

        if let Some(rgba) = node.entry.icon.to_rgba8() {
            let icon =
                muda::Icon::from_rgba(rgba.as_bytes().to_vec(), rgba.width(), rgba.height()).ok();
            let icon_menu = muda::IconMenuItem::with_id(
                id.clone(),
                &node.entry.title,
                node.entry.enabled,
                icon,
                None,
            );
            icon_menu.set_key_accelerator(accelerator).map_err(err_handler).ok();
            return Box::new(icon_menu);
        }

        let menu_item =
            muda::MenuItem::with_id(id.clone(), &node.entry.title, node.entry.enabled, None);
        menu_item.set_key_accelerator(accelerator).map_err(err_handler).ok();
        Box::new(menu_item)
    } else {
        let sub_menu = muda::Submenu::with_id(id.clone(), &node.entry.title, node.entry.enabled);
        Box::new(sub_menu)
    }
}

fn update_menu_branch(
    map: &mut EntryMap<MenuEntry>,
    menu: MenuRef<'_>,
    old_items: &[MenuNode],
    new_items: &[MenuNode],
    window_id: &str,
    muda_type: MudaType,
    depth: u32,
) {
    if depth > 15 && !new_items.is_empty() {
        // infinite menu depth is possible, but we want to limit the amount of item passed to muda
        menu.insert(&muda::MenuItem::new("<Error: Menu Depth limit reached>", false, None), 0)
            .unwrap();
        return;
    }

    // Get all menu items out of the muda item
    let menu_items = menu.items();

    // Enumerate through the old and new items simultaneously
    for (position, zipped) in Itertools::zip_longest(old_items.iter(), new_items.iter()).enumerate()
    {
        let (old_item_to_remove, new_item_to_build) = match zipped {
            EitherOrBoth::Both(old_item, new_item) => {
                // This item is there for the old and new; check for changes
                if menu_entry_looks_same(&old_item.entry, &new_item.entry) {
                    // Looks the same - recurse if this is a submenu
                    if let Some(muda::MenuItemKind::Submenu(submenu)) = menu_items.get(position) {
                        update_menu_branch(
                            map,
                            MenuRef::Submenu(submenu),
                            &old_item.children,
                            &new_item.children,
                            window_id,
                            muda_type,
                            depth + 1,
                        );
                    }

                    // We have not changed anything, but we need to refresh the item
                    refresh_map_entry(map, &menu_items[position], new_item.entry.clone());
                    (None, None)
                } else {
                    (Some(old_item), Some(new_item))
                }
            }
            EitherOrBoth::Left(old_item) => {
                // The old menu had an entry where the new menu does not; we need to remove it
                (Some(old_item), None)
            }
            EitherOrBoth::Right(new_item) => {
                // The new menu has an entry where the old menu did not; we need to append it
                (None, Some(new_item))
            }
        };

        // Do we have to remove the old menu?
        if let Some(old_item_to_remove) = old_item_to_remove {
            // If this is a submenu, we have to "update" (i.e. - delete the branch)
            if let Some(muda::MenuItemKind::Submenu(submenu)) = menu_items.get(position) {
                update_menu_branch(
                    map,
                    MenuRef::Submenu(submenu),
                    &old_item_to_remove.children,
                    &[],
                    window_id,
                    muda_type,
                    depth + 1,
                );
            }

            // Only remove if we're going to be inserting later; we'll take care of the other scenario later
            if new_item_to_build.is_some() {
                remove_menu_item_and_map_entry(map, &menu, position);
            }
        }

        // Do we need to add a new item?
        if let Some(new_item_to_build) = new_item_to_build {
            // Allocate an id for this item
            let entry_id = map.insert(new_item_to_build.entry.clone());
            let menu_id = muda::MenuId(format!("{window_id}|{entry_id}|{muda_type}"));

            // Create the new item
            let new_muda_item = build_menu_item(new_item_to_build, menu_id);

            // And if this is a submenu, recurse
            if let muda::MenuItemKind::Submenu(submenu) = new_muda_item.kind() {
                update_menu_branch(
                    map,
                    MenuRef::Submenu(&submenu),
                    &[],
                    &new_item_to_build.children,
                    window_id,
                    muda_type,
                    depth + 1,
                );
            }

            // And insert
            let _ = menu.insert(&*new_muda_item, position);
        }
    }

    // Purge extraneous items
    for _ in new_items.len()..old_items.len() {
        remove_menu_item_and_map_entry(map, &menu, new_items.len());
    }
}

impl MudaAdapter {
    pub fn setup(
        menubar: &vtable::VRc<MenuVTable>,
        winit_window: &Window,
        proxy: EventLoopProxy<SlintEvent>,
        window_adapter_weak: Weak<WinitWindowAdapter>,
    ) -> Self {
        install_event_handler_if_necessary(proxy);

        let tracker =
            Some(Box::pin(PropertyTracker::new_with_dirty_handler(MudaPropertyTracker {
                window_adapter_weak,
            })));

        let mut s = Self { map: EntryMap::new(), tracker, menu: None, menu_tree: Vec::new() };
        s.rebuild_menu(winit_window, Some(menubar), MudaType::Menubar);
        s
    }

    pub fn show_context_menu(
        context_menu: &vtable::VRc<MenuVTable>,
        winit_window: &Window,
        position: LogicalPosition,
        proxy: EventLoopProxy<SlintEvent>,
    ) -> Option<Self> {
        install_event_handler_if_necessary(proxy);

        let mut s = Self { map: EntryMap::new(), tracker: None, menu: None, menu_tree: Vec::new() };
        s.rebuild_menu(winit_window, Some(context_menu), MudaType::Context);

        match winit_window.window_handle().ok()?.as_raw() {
            #[cfg(target_os = "windows")]
            RawWindowHandle::Win32(handle) => {
                let position = i_slint_core::api::WindowPosition::Logical(position);
                let position = crate::winitwindowadapter::position_to_winit(&position);
                unsafe {
                    s.menu
                        .as_ref()
                        .expect("context menus should always have a menu")
                        .show_context_menu_for_hwnd(handle.hwnd.get(), Some(position));
                }
                Some(s)
            }
            #[cfg(target_os = "macos")]
            RawWindowHandle::AppKit(handle) => {
                // muda assumes a non-flipped NSView and flips Y internally. But winit's view
                // has isFlipped=true, so we pre-flip Y to compensate.
                let h =
                    winit_window.inner_size().to_logical::<f64>(winit_window.scale_factor()).height;
                let position = Some(winit::dpi::Position::Logical(
                    winit::dpi::LogicalPosition::new(position.x as f64, h - position.y as f64),
                ));
                unsafe {
                    s.menu
                        .as_ref()
                        .expect("context menus should always have a menu")
                        .show_context_menu_for_nsview(handle.ns_view.as_ptr(), position)
                };
                Some(s)
            }
            _ => None,
        }
    }

    pub fn rebuild_menu(
        &mut self,
        winit_window: &Window,
        menu_tree: Option<&vtable::VRc<MenuVTable>>,
        muda_type: MudaType,
    ) {
        if let Some(menu_tree) = menu_tree {
            let mut build_menu = || {
                let new_menu_tree = if vtable::VRc::borrow(menu_tree).visible() {
                    build_menu_tree(menu_tree)
                } else {
                    Vec::new()
                };

                // There is a very special case we need to watch out for; if there are no items in the menu bar
                // (and not a context menu) either before or after, just leave everything alone
                if !new_menu_tree.is_empty()
                    || muda_type != MudaType::Menubar
                    || !self.menu_tree.is_empty()
                {
                    // Access the menu, creating if if necessary
                    let menu = self.menu.get_or_insert_with(|| {
                        let menu = muda::Menu::new();

                        if muda_type == MudaType::Menubar {
                            #[cfg(target_os = "windows")]
                            if let RawWindowHandle::Win32(handle) =
                                winit_window.window_handle().unwrap().as_raw()
                            {
                                let theme = match winit_window.theme() {
                                    Some(winit::window::Theme::Dark) => muda::MenuTheme::Dark,
                                    Some(winit::window::Theme::Light) => muda::MenuTheme::Light,
                                    None => muda::MenuTheme::Auto,
                                };
                                unsafe {
                                    menu.init_for_hwnd_with_theme(handle.hwnd.get(), theme).unwrap()
                                };
                            }

                            #[cfg(target_os = "macos")]
                            {
                                menu.init_for_nsapp();
                                create_default_app_menu(menu).unwrap();
                            }
                        };
                        menu
                    });

                    let window_id = u64::from(winit_window.id()).to_string();
                    update_menu_branch(
                        &mut self.map,
                        MenuRef::Menu(menu),
                        &self.menu_tree,
                        &new_menu_tree,
                        &window_id,
                        muda_type,
                        0,
                    );
                    self.menu_tree = new_menu_tree;

                    // And related to the special case above, if the new menu bar is empty just
                    // blow it away (the `update_menu_branch()` was necessary to clean things up)
                    if self.menu_tree.is_empty() && muda_type == MudaType::Menubar {
                        self.menu = None;
                    }
                }
            };

            if let Some(tracker) = self.tracker.as_ref() {
                tracker.as_ref().evaluate(build_menu);
            } else {
                build_menu()
            }
        }
    }

    pub fn invoke(&self, menubar: &vtable::VRc<MenuVTable>, entry_id: usize) {
        let Some(entry) = &self.map.get(entry_id) else { return };
        vtable::VRc::borrow(menubar).activate(entry);
    }

    #[cfg(target_os = "windows")]
    pub fn set_menubar_theme(
        &self,
        winit_window: &Window,
        theme: i_slint_core::items::ColorScheme,
    ) {
        let theme = match theme {
            i_slint_core::items::ColorScheme::Dark => muda::MenuTheme::Dark,
            i_slint_core::items::ColorScheme::Light => muda::MenuTheme::Light,
            i_slint_core::items::ColorScheme::Unknown | _ => muda::MenuTheme::Auto,
        };
        if let RawWindowHandle::Win32(handle) = winit_window.window_handle().unwrap().as_raw()
            && let Some(menu) = self.menu.as_ref()
        {
            unsafe { menu.set_theme_for_hwnd(handle.hwnd.get(), theme).unwrap() };
        }
    }

    #[cfg(target_os = "macos")]
    pub fn setup_default_menu_bar() -> Result<Self, i_slint_core::api::PlatformError> {
        let menu_bar = muda::Menu::new();
        create_default_app_menu(&menu_bar)?;
        menu_bar.init_for_nsapp();
        Ok(Self { entries: Vec::new(), menu: Some(menu_bar), tracker: None })
    }

    #[cfg(target_os = "macos")]
    pub fn window_activation_changed(&self, is_active: bool) {
        if is_active && let Some(menu) = self.menu.as_ref() {
            menu.init_for_nsapp();
        }
    }
}

/// Used to check if these menu items look the same
fn menu_entry_looks_same(a: &MenuEntry, b: &MenuEntry) -> bool {
    // There are two things stopping us from using the normal `Eq` trait
    // - `id` can change at will but is not relevant for our purposes
    // - Comparisons with `icon` seem to always return false
    a.title == b.title
        && a.icon.path() == b.icon.path()
        && a.enabled == b.enabled
        && a.checkable == b.checkable
        && a.checked == b.checked
        && a.has_sub_menu == b.has_sub_menu
        && a.is_separator == b.is_separator
        && a.shortcut == b.shortcut
}

enum MenuRef<'a> {
    Menu(&'a muda::Menu),
    Submenu(&'a muda::Submenu),
}

impl<'a> MenuRef<'a> {
    pub fn items(&self) -> Vec<muda::MenuItemKind> {
        match self {
            Self::Menu(menu) => menu.items(),
            Self::Submenu(submenu) => submenu.items(),
        }
    }

    pub fn remove_at(&self, position: usize) -> Option<muda::MenuItemKind> {
        match self {
            Self::Menu(menu) => menu.remove_at(position),
            Self::Submenu(submenu) => submenu.remove_at(position),
        }
    }

    pub fn insert(&self, item: &dyn muda::IsMenuItem, position: usize) -> muda::Result<()> {
        match self {
            Self::Menu(menu) => menu.insert(item, position),
            Self::Submenu(submenu) => submenu.insert(item, position),
        }
    }
}

#[derive(Debug)]
struct EntryMap<T> {
    map: HashMap<usize, T>,
    pool: IdPool,
}

impl<T> EntryMap<T> {
    pub fn new() -> Self {
        Self { map: HashMap::new(), pool: IdPool::new() }
    }

    pub fn insert(&mut self, entry: T) -> usize {
        let id = self.pool.request_id().unwrap();
        self.map.insert(id, entry);
        id
    }

    pub fn release(&mut self, id: usize) {
        let _ = self.pool.return_id(id);
        self.map.remove(&id);
    }

    pub fn get(&self, id: usize) -> Option<&T> {
        self.map.get(&id)
    }
}

fn remove_menu_item_and_map_entry(
    map: &mut EntryMap<MenuEntry>,
    menu: &MenuRef<'_>,
    position: usize,
) {
    let Some(item) = menu.remove_at(position) else {
        return;
    };
    let &[_, id, _] = item.id().as_ref().split('|').collect::<Vec<_>>().as_slice() else {
        return;
    };
    let Ok(id) = id.parse::<usize>() else {
        return;
    };
    map.release(id);
}

fn refresh_map_entry(
    map: &mut EntryMap<MenuEntry>,
    item: &muda::MenuItemKind,
    updated_entry: MenuEntry,
) {
    let &[_, id, _] = item.id().as_ref().split('|').collect::<Vec<_>>().as_slice() else {
        return;
    };
    let Ok(id) = id.parse::<usize>() else {
        return;
    };
    let Some(entry) = map.map.get_mut(&id) else {
        return;
    };
    *entry = updated_entry;
}

fn key_string_to_key(string: &str) -> muda::accelerator::Key {
    use muda::accelerator::Key;
    macro_rules! key_string_to_code_impl {
        ($($char:literal # $_name:ident # $($_shifted:ident)? $(=> $($muda:ident)? # $($_qt:ident)|* # $($_winit:ident $(($_pos:ident))?)|* # $($_xkb:ident)|*)?;)*) => {
            match string.chars().next() {
                $($($(Some($char) => Key::$muda,)?)?)*
                _ => Key::Character(string.to_owned()),
            }
        };
    }
    i_slint_common::for_each_keys!(key_string_to_code_impl)
}

fn keys_to_accelerator(
    keys: &i_slint_core::input::Keys,
) -> Option<muda::accelerator::KeyAccelerator> {
    use muda::accelerator::*;

    if *keys == i_slint_core::input::Keys::default() {
        return None;
    }

    let shortcut = i_slint_core::input::KeysInner::from_pub(keys);

    let mut modifiers = Modifiers::empty();
    if shortcut.modifiers.control {
        if i_slint_core::is_apple_platform() {
            modifiers |= Modifiers::SUPER;
        } else {
            modifiers |= Modifiers::CONTROL;
        }
    }
    if shortcut.modifiers.alt {
        modifiers |= Modifiers::ALT;
    }
    if shortcut.modifiers.shift {
        modifiers |= Modifiers::SHIFT;
    }
    if shortcut.modifiers.meta {
        if i_slint_core::is_apple_platform() {
            modifiers |= Modifiers::CONTROL;
        } else {
            modifiers |= Modifiers::SUPER;
        }
    }
    let key = key_string_to_key(&shortcut.key);

    Some(KeyAccelerator::new(Some(modifiers), key))
}

fn install_event_handler_if_necessary(proxy: EventLoopProxy<SlintEvent>) {
    // `MenuEvent::set_event_handler()` in `muda` seems to use `OnceCell`, which is an
    // can only be set a single time.  Therefore, we need to take care to only call this
    // a single time
    //
    // Arguably, `set_event_handler()` is unsafe
    if !MUDA_SET_EVENT_HANDLER_INSTALLED.load(Ordering::Relaxed) {
        muda::MenuEvent::set_event_handler(Some(move |e| {
            let _ = proxy.send_event(SlintEvent(CustomEvent::Muda(e)));
        }));

        MUDA_SET_EVENT_HANDLER_INSTALLED.store(true, Ordering::Relaxed);
    }
}

#[cfg(target_os = "macos")]
fn create_default_app_menu(menu_bar: &muda::Menu) -> Result<(), i_slint_core::api::PlatformError> {
    let app_menu = muda::Submenu::new("App", true);
    menu_bar
        .append(&app_menu)
        .and_then(|_| {
            app_menu.append_items(&[
                &muda::PredefinedMenuItem::about(None, None),
                &muda::PredefinedMenuItem::separator(),
                &muda::PredefinedMenuItem::services(None),
                &muda::PredefinedMenuItem::separator(),
                &muda::PredefinedMenuItem::hide(None),
                &muda::PredefinedMenuItem::hide_others(None),
                &muda::PredefinedMenuItem::show_all(None),
                &muda::PredefinedMenuItem::separator(),
                &muda::PredefinedMenuItem::quit(None),
            ])
        })
        .map_err(|menu_bar_err| {
            i_slint_core::api::PlatformError::Other(menu_bar_err.to_string())
        })?;
    Ok(())
}
