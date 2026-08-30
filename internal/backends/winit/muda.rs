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
use muda::ContextMenu;
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
    entries: Vec<MenuEntry>,
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

fn menu_entry_count(node: &MenuNode) -> usize {
    1 + node.children.iter().fold(0usize, |count, child| count + menu_entry_count(child))
}

fn flatten_menu_tree(tree: &[MenuNode], result: &mut Vec<MenuEntry>) {
    for item in tree {
        result.push(item.entry.clone());
        if item.entry.has_sub_menu {
            flatten_menu_tree(&item.children, result);
        }
    }
}

fn build_menu_item(
    node: &MenuNode,
    start_index: usize,
    window_id: &str,
    muda_type: MudaType,
) -> Box<dyn muda::IsMenuItem> {
    let id = muda::MenuId(format!("{window_id}|{start_index}|{muda_type}"));
    if node.entry.is_separator {
        return Box::new(muda::PredefinedMenuItem::separator());
    }

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
        return Box::new(menu_item);
    }

    let sub_menu = muda::Submenu::with_id(id.clone(), &node.entry.title, node.entry.enabled);
    let mut next_index = start_index + 1;
    for child in &node.children {
        sub_menu.append(&*build_menu_item(child, next_index, window_id, muda_type)).unwrap();
        next_index += menu_entry_count(child);
    }
    Box::new(sub_menu)
}

fn rebuild_root_menu(
    menu: &muda::Menu,
    tree: &[MenuNode],
    start_index: usize,
    window_id: &str,
    muda_type: MudaType,
) {
    while menu.remove_at(0).is_some() {}
    let mut next_index = start_index;
    for item in tree {
        menu.append(&*build_menu_item(item, next_index, window_id, muda_type)).unwrap();
        next_index += menu_entry_count(item);
    }
}

fn update_submenu(
    submenu: &muda::Submenu,
    old_items: &[MenuNode],
    new_items: &[MenuNode],
    start_index: usize,
    window_id: &str,
    muda_type: MudaType,
) {
    if old_items.len() != new_items.len() {
        rebuild_submenu(submenu, new_items, start_index, window_id, muda_type);
        return;
    }

    let mut local_index = 0usize;
    let mut next_index = start_index;
    for (old_item, new_item) in old_items.iter().zip(new_items) {
        if old_item.entry == new_item.entry {
            if old_item.entry.has_sub_menu
                && let Some(muda::MenuItemKind::Submenu(child_submenu)) =
                    submenu.items().get(local_index)
            {
                update_submenu(
                    child_submenu,
                    &old_item.children,
                    &new_item.children,
                    next_index + 1,
                    window_id,
                    muda_type,
                );
            }
        } else {
            submenu.remove_at(local_index);
            submenu
                .insert(&*build_menu_item(new_item, next_index, window_id, muda_type), local_index)
                .unwrap();
        }

        local_index += 1;
        next_index += menu_entry_count(old_item);
    }
}

fn rebuild_submenu(
    submenu: &muda::Submenu,
    tree: &[MenuNode],
    start_index: usize,
    window_id: &str,
    muda_type: MudaType,
) {
    while submenu.remove_at(0).is_some() {}
    let mut next_index = start_index;
    for item in tree {
        submenu.append(&*build_menu_item(item, next_index, window_id, muda_type)).unwrap();
        next_index += menu_entry_count(item);
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

fn update_menu_branch(
    menu: &muda::Menu,
    old_items: &[MenuNode],
    new_items: &[MenuNode],
    start_index: usize,
    window_id: &str,
    muda_type: MudaType,
) {
    if old_items.len() != new_items.len() {
        rebuild_root_menu(menu, new_items, start_index, window_id, muda_type);
        return;
    }

    let mut local_index = 0usize;
    let mut next_index = start_index;
    for (old_item, new_item) in old_items.iter().zip(new_items) {
        if old_item.entry == new_item.entry {
            if old_item.entry.has_sub_menu
                && let Some(muda::MenuItemKind::Submenu(submenu)) = menu.items().get(local_index)
            {
                update_submenu(
                    submenu,
                    &old_item.children,
                    &new_item.children,
                    next_index + 1,
                    window_id,
                    muda_type,
                );
            }
        } else {
            menu.remove_at(local_index);
            menu.insert(&*build_menu_item(new_item, next_index, window_id, muda_type), local_index)
                .unwrap();
        }

        local_index += 1;
        next_index += menu_entry_count(old_item);
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

        let mut s =
            Self { entries: Default::default(), tracker, menu: None, menu_tree: Vec::new() };
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

        let mut s =
            Self { entries: Default::default(), tracker: None, menu: None, menu_tree: Vec::new() };
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
                let new_menu_tree = build_menu_tree(menu_tree);
                if new_menu_tree.is_empty() && muda_type == MudaType::Menubar {
                    self.menu = None;
                    self.entries.clear();
                    self.menu_tree.clear();
                    return;
                }

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
                        }
                    };
                    menu
                });

                #[cfg(target_os = "macos")]
                if matches!(muda_type, MudaType::Menubar) {
                    create_default_app_menu(menu).unwrap();
                }

                let window_id = u64::from(winit_window.id()).to_string();
                let old_tree = std::mem::take(&mut self.menu_tree);
                if old_tree.is_empty() {
                    rebuild_root_menu(menu, &new_menu_tree, 0, &window_id, muda_type);
                } else {
                    update_menu_branch(menu, &old_tree, &new_menu_tree, 0, &window_id, muda_type);
                };
                self.menu_tree = new_menu_tree;
                let mut flat = Vec::new();
                flatten_menu_tree(&self.menu_tree, &mut flat);
                self.entries = flat;
            };

            if let Some(tracker) = self.tracker.as_ref() {
                tracker.as_ref().evaluate(build_menu);
            } else {
                build_menu()
            }
        }
    }

    pub fn invoke(&self, menubar: &vtable::VRc<MenuVTable>, entry_id: usize) {
        let Some(entry) = &self.entries.get(entry_id) else { return };
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
