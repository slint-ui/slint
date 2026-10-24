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
use itertools::EitherOrBoth;
use itertools::Itertools;
use muda::ContextMenu;
use std::collections::HashMap;
use std::rc::Weak;
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicU64};
use winit::event_loop::EventLoopProxy;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

const MAX_DEPTH: u32 = 15;

#[derive(Clone, Debug)]
struct MenuNode {
    entry: MenuEntry,
    children: Vec<MenuNode>,
    id: usize,
}

pub struct MudaAdapter {
    map: HashMap<usize, MenuEntry>,
    map_next_id: usize,
    tracker: Option<Pin<Box<PropertyTracker<false, MudaPropertyTracker>>>>,
    menu: Option<muda::Menu>,
    menu_tree: Vec<MenuNode>,
    generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumString, strum::Display)]
pub enum MudaType {
    Menubar,
    Context,
}

static NEXT_MENU_GENERATION: AtomicU64 = AtomicU64::new(1);

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
    depth: u32,
    map_next_id: &mut usize,
) -> Vec<MenuNode> {
    let mut raw_entries = Default::default();
    menu.sub_menu(parent, &mut raw_entries);

    let mut tree = Vec::new();
    if depth > MAX_DEPTH {
        if !raw_entries.is_empty() {
            let entry = MenuEntry {
                title: "<Error: Menu Depth limit reached>".into(),
                enabled: false,
                ..Default::default()
            };
            let id = *map_next_id;
            *map_next_id += 1;
            tree.push(MenuNode { entry, children: Vec::new(), id });
        }
        return tree;
    }
    for entry in raw_entries {
        let children = if entry.has_sub_menu && depth <= MAX_DEPTH {
            build_menu_tree_for_parent(menu, Some(&entry), depth + 1, map_next_id)
        } else {
            Vec::new()
        };
        let id = *map_next_id;
        *map_next_id += 1;
        tree.push(MenuNode { entry: entry.clone(), children, id });
    }
    tree
}

fn build_menu_tree(menu: &vtable::VRc<MenuVTable>, map_next_id: &mut usize) -> Vec<MenuNode> {
    build_menu_tree_for_parent(vtable::VRc::borrow(menu), None, 0, map_next_id)
}

fn build_menu_item(node: &MenuNode, id: muda::MenuId) -> Box<dyn muda::IsMenuItem> {
    if !node.entry.has_sub_menu {
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
    map: &mut HashMap<usize, MenuEntry>,
    generation: u64,
    menu: MenuRef<'_>,
    old_items: &[MenuNode],
    new_items: &[MenuNode],
    window_id: &str,
    muda_type: MudaType,
    depth: u32,
    menu_offset: usize,
) {
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
                    if let Some(muda::MenuItemKind::Submenu(submenu)) =
                        menu_items.get(position + menu_offset)
                    {
                        update_menu_branch(
                            map,
                            generation,
                            MenuRef::Submenu(submenu),
                            &old_item.children,
                            &new_item.children,
                            window_id,
                            muda_type,
                            depth + 1,
                            0,
                        );
                    }

                    // We have not changed anything, but we need to refresh the item
                    if let Some(item) = menu_items.get(position + menu_offset) {
                        map.insert(new_item.id, new_item.entry.clone());
                    }
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
            if let Some(muda::MenuItemKind::Submenu(submenu)) =
                menu_items.get(position + menu_offset)
            {
                update_menu_branch(
                    map,
                    generation,
                    MenuRef::Submenu(submenu),
                    &old_item_to_remove.children,
                    &[],
                    window_id,
                    muda_type,
                    depth + 1,
                    0,
                );
            }

            // Only remove if we're going to be inserting later; we'll take care of the other scenario later
            if new_item_to_build.is_some() {
                let idx = position + menu_offset;
                if let Some(item) = menu.remove_at(idx) {
                    if let Some((_, _, entry_id, _)) = parse_menu_item_id(item.id()) {
                        map.remove(&entry_id);
                    }
                }
                // Removing shifts items, but we handle by position carefully
            }
        }

        // Do we need to add a new item?
        if let Some(new_item_to_build) = new_item_to_build {
            // Create the new item
            let new_muda_item = if new_item_to_build.entry.is_separator {
                Box::new(muda::PredefinedMenuItem::separator())
            } else {
                map.insert(new_item_to_build.id, new_item_to_build.entry.clone());
                let menu_id = menu_item_id(window_id, generation, new_item_to_build.id, muda_type);

                // And build it
                build_menu_item(new_item_to_build, menu_id)
            };

            // And if this is a submenu, recurse
            if let muda::MenuItemKind::Submenu(submenu) = new_muda_item.kind() {
                update_menu_branch(
                    map,
                    generation,
                    MenuRef::Submenu(&submenu),
                    &[],
                    &new_item_to_build.children,
                    window_id,
                    muda_type,
                    depth + 1,
                    0,
                );
            }

            // And insert
            let _ = menu.insert(&*new_muda_item, position + menu_offset);
        }
    }

    // Purge extraneous items
    let mut idx = new_items.len() + menu_offset;
    let old_count = old_items.len();
    for _ in new_items.len()..old_count {
        if let Some(item) = menu.remove_at(idx) {
            if let Some((_, _, entry_id, _)) = parse_menu_item_id(item.id()) {
                map.remove(&entry_id);
            }
            // Don't increment idx because items shift
        } else {
            break;
        }
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

        let mut s = Self {
            map: Default::default(),
            map_next_id: 0,
            tracker,
            menu: None,
            menu_tree: Vec::new(),
            generation: 0,
        };
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

        let mut s = Self {
            map: Default::default(),
            map_next_id: 0,
            tracker: None,
            menu: None,
            menu_tree: Vec::new(),
            generation: 0,
        };
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
            // Eventually, we will have to be doing full rebuilds
            if self.map_next_id > 1000000 {
                self.reset_entries();
            }

            let mut build_menu = || {
                let mut new_map_next_id = self.map_next_id;
                let new_menu_tree = if vtable::VRc::borrow(menu_tree).visible() {
                    build_menu_tree(menu_tree, &mut new_map_next_id)
                } else {
                    Vec::new()
                };
                self.map_next_id = new_map_next_id;

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
                                create_default_app_menu(&menu).unwrap();
                            }
                        };
                        menu
                    });

                    let window_id = u64::from(winit_window.id()).to_string();
                    #[cfg(target_os = "macos")]
                    let menu_offset = if muda_type == MudaType::Menubar { 1usize } else { 0 };
                    #[cfg(not(target_os = "macos"))]
                    let menu_offset = 0;
                    update_menu_branch(
                        &mut self.map,
                        self.generation,
                        MenuRef::Menu(menu),
                        &self.menu_tree,
                        &new_menu_tree,
                        &window_id,
                        muda_type,
                        0,
                        menu_offset,
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

    fn reset_entries(&mut self) {
        // AppKit can retain old items after a rebuild or adapter replacement.
        self.generation = NEXT_MENU_GENERATION.fetch_add(1, Ordering::Relaxed);
        self.map.clear();
        self.menu_tree.clear();
        self.map_next_id = 0;
    }

    pub fn invoke(&self, menubar: &vtable::VRc<MenuVTable>, generation: u64, entry_id: usize) {
        if generation != self.generation {
            return;
        }
        let Some(entry) = &self.map.get(&entry_id) else { return };
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
        Ok(Self {
            map: Default::new(),
            map_next_id: 0,
            menu: Some(menu_bar),
            tracker: None,
            menu_tree: Vec::new(),
            generation: 0,
        })
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
        && icons_equal(&a.icon, &b.icon)
        && a.enabled == b.enabled
        && a.checkable == b.checkable
        && a.checked == b.checked
        && a.has_sub_menu == b.has_sub_menu
        && a.is_separator == b.is_separator
        && a.shortcut == b.shortcut
}

fn icons_equal(a: &i_slint_core::graphics::Image, b: &i_slint_core::graphics::Image) -> bool {
    let a_path = a.path();
    let b_path = b.path();
    if a_path.is_some() && b_path.is_some() {
        a_path == b_path
    } else if a_path.is_none() && b_path.is_none() {
        a == b
    } else {
        a_path == b_path
    }
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

/*
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
     */

fn menu_item_id(
    window_id: &str,
    generation: u64,
    entry_id: usize,
    muda_type: MudaType,
) -> muda::MenuId {
    muda::MenuId(format!("{window_id}|{generation}|{entry_id}|{muda_type}"))
}

pub fn parse_menu_item_id(
    id: &muda::MenuId,
) -> Option<(winit::window::WindowId, u64, usize, MudaType)> {
    let (window, entry) = id.0.split_once('|')?;
    let (generation, entry) = entry.split_once('|')?;
    let (entry, muda_type) = entry.split_once('|')?;
    Some((
        winit::window::WindowId::from(window.parse::<u64>().ok()?),
        generation.parse().ok()?,
        entry.parse().ok()?,
        muda_type.parse().ok()?,
    ))
}

fn key_string_to_key(string: &str) -> muda::accelerator::Key {
    use muda::accelerator::{Key, NamedKey};
    macro_rules! key_string_to_code_impl {
        ($($char:literal # $_name:ident # $($_shifted:ident)? $(=> $($muda:ident)? # $($_qt:ident)|* # $($_winit:ident $(($_pos:ident))?)|* # $($_xkb:ident)|*)?;)*) => {
            match string.chars().next() {
                $($($(Some($char) => Key::Named(NamedKey::$muda),)?)?)*
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
            modifiers |= Modifiers::META;
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
            modifiers |= Modifiers::META;
        }
    }
    let key = key_string_to_key(&shortcut.key);

    Some(KeyAccelerator::new(modifiers, key))
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

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_core::SharedVector;
    use i_slint_core::menus::Menu;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct RecordingMenu(Rc<RefCell<Vec<i_slint_core::SharedString>>>);

    impl Menu for RecordingMenu {
        fn sub_menu(&self, _: Option<&MenuEntry>, _: &mut SharedVector<MenuEntry>) {}

        fn activate(&self, entry: &MenuEntry) {
            self.0.borrow_mut().push(entry.id.clone());
        }

        fn visible(&self) -> bool {
            true
        }
    }

    i_slint_core::MenuVTable_static!(static RECORDING_MENU_VT for RecordingMenu);

    fn adapter(id: &str) -> MudaAdapter {
        let mut adapter = MudaAdapter {
            map: Default::default(),
            map_next_id: 0,
            tracker: None,
            menu: None,
            menu_tree: Vec::new(),
            generation: 0,
        };
        replace_entry(&mut adapter, id);
        adapter
    }

    fn replace_entry(adapter: &mut MudaAdapter, id: &str) {
        adapter.reset_entries();
        adapter.map.insert(0, MenuEntry { id: id.into(), ..Default::default() });
    }

    fn recording_menu() -> (vtable::VRc<MenuVTable>, Rc<RefCell<Vec<i_slint_core::SharedString>>>) {
        let activations = Rc::new(RefCell::new(Vec::new()));
        let menu = vtable::VRc::into_dyn(vtable::VRc::new(RecordingMenu(activations.clone())));
        (menu, activations)
    }

    fn deliver(adapter: &MudaAdapter, menu: &vtable::VRc<MenuVTable>, id: &muda::MenuId) {
        let (_, generation, entry, _) = parse_menu_item_id(id).unwrap();
        adapter.invoke(menu, generation, entry);
    }

    #[test]
    fn native_menu_ids_round_trip() {
        for menu_type in [MudaType::Menubar, MudaType::Context] {
            let id = menu_item_id("42", 17, 3, menu_type);
            assert_eq!(parse_menu_item_id(&id), Some((42.into(), 17, 3, menu_type)));
        }
        for id in [
            "",
            "42|0|Menubar",
            "x|17|0|Menubar",
            "42|x|0|Menubar",
            "42|17|x|Menubar",
            "42|17|0|Unknown",
            "42|17|0|Menubar|extra",
        ] {
            assert!(parse_menu_item_id(&muda::MenuId(id.into())).is_none(), "{id}");
        }
    }

    #[test]
    fn current_event_activates_its_entry() {
        let (menu, activations) = recording_menu();
        let adapter = adapter("deck-a");
        let id = menu_item_id("42", adapter.generation, 0, MudaType::Menubar);
        deliver(&adapter, &menu, &id);
        assert_eq!(&*activations.borrow(), &[i_slint_core::SharedString::from("deck-a")]);
    }

    #[test]
    fn missing_entry_is_ignored() {
        let (menu, activations) = recording_menu();
        let adapter = adapter("deck-a");
        let id = menu_item_id("42", adapter.generation, 1, MudaType::Menubar);
        deliver(&adapter, &menu, &id);
        assert!(activations.borrow().is_empty());
    }

    #[test]
    fn event_queued_before_rebuild_does_not_activate_replacement() {
        for menu_type in [MudaType::Menubar, MudaType::Context] {
            let (menu, activations) = recording_menu();
            let mut adapter = adapter("deck-a");
            let queued = menu_item_id("42", adapter.generation, 0, menu_type);
            replace_entry(&mut adapter, "deck-b");
            deliver(&adapter, &menu, &queued);
            assert!(activations.borrow().is_empty(), "{menu_type}: {:?}", activations.borrow());
            let current = menu_item_id("42", adapter.generation, 0, menu_type);
            deliver(&adapter, &menu, &current);
            assert_eq!(&*activations.borrow(), &[i_slint_core::SharedString::from("deck-b")]);
        }
    }

    #[test]
    fn event_from_replaced_adapter_is_ignored() {
        for menu_type in [MudaType::Menubar, MudaType::Context] {
            let (menu, activations) = recording_menu();
            let old = adapter("deck-a");
            let queued = menu_item_id("42", old.generation, 0, menu_type);
            drop(old);
            let replacement = adapter("deck-b");
            deliver(&replacement, &menu, &queued);
            assert!(activations.borrow().is_empty(), "{menu_type}: {:?}", activations.borrow());
            let current = menu_item_id("42", replacement.generation, 0, menu_type);
            deliver(&replacement, &menu, &current);
            assert_eq!(&*activations.borrow(), &[i_slint_core::SharedString::from("deck-b")]);
        }
    }
}
