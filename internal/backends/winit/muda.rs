// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore nsapp
use super::CustomEvent;
use super::WinitWindowAdapter;
use crate::{SharedBackendData, SlintEvent};
use core::pin::Pin;
use i_slint_core::SharedVector;
use i_slint_core::api::LogicalPosition;
use i_slint_core::items::MenuEntry;
#[cfg(any(target_os = "macos", test))]
use i_slint_core::items::MenuItemRole;
use i_slint_core::menus::MenuVTable;
use i_slint_core::properties::{PropertyDirtyHandler, PropertyTracker};
use muda::ContextMenu;
use std::rc::Weak;
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicU64};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

pub struct MudaAdapter {
    entries: Vec<MenuEntry>,
    tracker: Option<Pin<Box<PropertyTracker<false, MudaPropertyTracker>>>>,
    menu: Option<muda::Menu>,
    generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumString, strum::Display)]
pub enum MudaType {
    Menubar,
    Context,
}

#[cfg(target_os = "macos")]
pub(crate) const DEFAULT_QUIT_MENU_ID: &str = "slint-default-quit";

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

impl MudaAdapter {
    pub fn setup(
        menubar: &vtable::VRc<MenuVTable>,
        winit_window: &Window,
        shared_backend_data: &SharedBackendData,
        window_adapter_weak: Weak<WinitWindowAdapter>,
    ) -> Self {
        install_event_handler_if_necessary(shared_backend_data);

        let tracker =
            Some(Box::pin(PropertyTracker::new_with_dirty_handler(MudaPropertyTracker {
                window_adapter_weak,
            })));

        let mut s = Self { entries: Default::default(), tracker, menu: None, generation: 0 };
        s.rebuild_menu(winit_window, Some(menubar), MudaType::Menubar);
        s
    }

    pub fn show_context_menu(
        context_menu: &vtable::VRc<MenuVTable>,
        winit_window: &Window,
        position: LogicalPosition,
        shared_backend_data: &SharedBackendData,
    ) -> Option<Self> {
        install_event_handler_if_necessary(shared_backend_data);

        let mut s = Self { entries: Default::default(), tracker: None, menu: None, generation: 0 };
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
        let must_set_window_redraw = cfg!(windows) && winit_window.is_visible() == Some(true);
        if must_set_window_redraw {
            win32_set_window_redraw(winit_window, false);
        }

        self.reset_entries();

        if let Some(menu_tree) = menu_tree {
            let mut build_menu = || {
                let menu = vtable::VRc::borrow(menu_tree);
                let mut menu_entries = Default::default();
                if menu.visible() {
                    menu.sub_menu(None, &mut menu_entries);
                }

                if menu_entries.is_empty() && muda_type == MudaType::Menubar {
                    self.menu = None;
                } else if let Some(menu) = self.menu.as_ref() {
                    while menu.remove_at(0).is_some() {}
                } else {
                    self.menu = Some(muda::Menu::new());

                    if muda_type == MudaType::Menubar
                        && let Some(menu) = self.menu.as_ref()
                    {
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
                    }
                }

                #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
                let mut nodes = collect_entries(menu, menu_entries, 0);
                let mut builder = ItemBuilder {
                    entries: &mut self.entries,
                    window_id: u64::from(winit_window.id()).to_string(),
                    muda_type,
                    generation: self.generation,
                };

                #[cfg(target_os = "macos")]
                if muda_type == MudaType::Menubar
                    && let Some(menu) = self.menu.as_ref()
                {
                    let role_entries = take_role_entries(&mut nodes);
                    create_app_menu(menu, &role_entries, |entry, default_accelerator| {
                        builder.menu_item(entry, default_accelerator)
                    })
                    .unwrap();
                }

                if let Some(menu) = self.menu.as_ref() {
                    for node in &nodes {
                        menu.append(&*builder.build(node, 0)).unwrap();
                    }
                }
            };

            if let Some(tracker) = self.tracker.as_ref() {
                tracker.as_ref().evaluate(build_menu);
            } else {
                build_menu()
            }
        }

        if must_set_window_redraw {
            win32_set_window_redraw(winit_window, true);
        }
    }

    fn reset_entries(&mut self) {
        // AppKit can retain old items after a rebuild or adapter replacement.
        self.generation = NEXT_MENU_GENERATION.fetch_add(1, Ordering::Relaxed);
        self.entries.clear();
    }

    pub fn invoke(&self, menubar: &vtable::VRc<MenuVTable>, generation: u64, entry_id: usize) {
        if generation != self.generation {
            return;
        }
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
    pub fn setup_default_menu_bar(
        shared_backend_data: &SharedBackendData,
    ) -> Result<Self, i_slint_core::api::PlatformError> {
        install_event_handler_if_necessary(shared_backend_data);
        let menu_bar = muda::Menu::new();
        create_app_menu(&menu_bar, &[], |_, _| unreachable!())?;
        menu_bar.init_for_nsapp();
        Ok(Self { entries: Vec::new(), menu: Some(menu_bar), tracker: None, generation: 0 })
    }

    #[cfg(target_os = "macos")]
    pub fn window_activation_changed(&self, is_active: bool) {
        if is_active && let Some(menu) = self.menu.as_ref() {
            menu.init_for_nsapp();
        }
    }
}

// Infinite menu depth is possible, but we want to limit the amount of items passed to muda.
const MAX_MENU_DEPTH: usize = 15;

struct EntryNode {
    entry: MenuEntry,
    children: Vec<EntryNode>,
}

fn collect_entries(
    menu: vtable::VRef<'_, MenuVTable>,
    entries: SharedVector<MenuEntry>,
    depth: usize,
) -> Vec<EntryNode> {
    entries
        .into_iter()
        .map(|entry| {
            let mut children = Vec::new();
            if entry.has_sub_menu && depth < MAX_MENU_DEPTH {
                let mut sub_entries = Default::default();
                menu.sub_menu(Some(&entry), &mut sub_entries);
                children = collect_entries(menu, sub_entries, depth + 1);
            }
            EntryNode { entry, children }
        })
        .collect()
}

/// Removes the entries matching `take` from `nodes` and returns them in menu order.
/// Also removes the separators and sub-menus that the removal leaves without content.
#[cfg(any(target_os = "macos", test))]
fn take_entries(
    nodes: &mut Vec<EntryNode>,
    take: &mut impl FnMut(&MenuEntry) -> bool,
) -> Vec<MenuEntry> {
    let mut taken = Vec::new();
    nodes.retain_mut(|node| {
        if node.entry.has_sub_menu {
            let had_children = !node.children.is_empty();
            taken.extend(take_entries(&mut node.children, take));
            !had_children || !node.children.is_empty()
        } else if !node.entry.is_separator && take(&node.entry) {
            taken.push(node.entry.clone());
            false
        } else {
            true
        }
    });

    let mut last_is_separator = true;
    nodes.retain(|node| {
        let keep = !(node.entry.is_separator && last_is_separator);
        last_is_separator = node.entry.is_separator;
        keep
    });
    if nodes.last().is_some_and(|node| node.entry.is_separator) {
        nodes.pop();
    }

    taken
}

/// Takes the entries that belong in the macOS application menu: the first entry of each role,
/// and any further entries with an application menu role, since they can't do anything in place.
#[cfg(any(target_os = "macos", test))]
fn take_role_entries(nodes: &mut Vec<EntryNode>) -> Vec<MenuEntry> {
    let mut seen_roles = Vec::new();
    take_entries(nodes, &mut |entry| {
        if entry.role == MenuItemRole::None {
            false
        } else if seen_roles.contains(&entry.role) {
            entry.role.is_application_menu_role()
        } else {
            seen_roles.push(entry.role);
            true
        }
    })
}

struct ItemBuilder<'a> {
    entries: &'a mut Vec<MenuEntry>,
    window_id: String,
    muda_type: MudaType,
    generation: u64,
}

impl ItemBuilder<'_> {
    fn register(&mut self, entry: &MenuEntry) -> muda::MenuId {
        let id = menu_item_id(&self.window_id, self.generation, self.entries.len(), self.muda_type);
        self.entries.push(entry.clone());
        id
    }

    fn set_accelerator(
        entry: &MenuEntry,
        accelerator: Option<muda::accelerator::KeyAccelerator>,
        set: impl FnOnce(Option<muda::accelerator::KeyAccelerator>) -> muda::Result<()>,
    ) {
        set(accelerator)
            .map_err(|err| {
                i_slint_core::debug_log!(
                    "Warning: Could not set accelerator {} for menu item {}: {err}",
                    entry.shortcut,
                    entry.title
                )
            })
            .ok();
    }

    fn menu_item(
        &mut self,
        entry: &MenuEntry,
        default_accelerator: Option<muda::accelerator::KeyAccelerator>,
    ) -> muda::MenuItem {
        let id = self.register(entry);
        let menu_item = muda::MenuItem::with_id(id, &entry.title, entry.enabled, None);
        let accelerator = keys_to_accelerator(&entry.shortcut).or(default_accelerator);
        Self::set_accelerator(entry, accelerator, |a| menu_item.set_key_accelerator(a));
        menu_item
    }

    fn build(&mut self, node: &EntryNode, depth: usize) -> Box<dyn muda::IsMenuItem> {
        let entry = &node.entry;
        if entry.is_separator {
            self.register(entry);
            Box::new(muda::PredefinedMenuItem::separator())
        } else if !entry.has_sub_menu {
            if entry.checkable {
                let id = self.register(entry);
                let check_menu = muda::CheckMenuItem::with_id(
                    id,
                    &entry.title,
                    entry.enabled,
                    entry.checked,
                    None,
                );
                Self::set_accelerator(entry, keys_to_accelerator(&entry.shortcut), |a| {
                    check_menu.set_key_accelerator(a)
                });
                Box::new(check_menu)
            } else if let Some(rgba) = entry.icon.to_rgba8() {
                let id = self.register(entry);
                let icon =
                    muda::Icon::from_rgba(rgba.as_bytes().to_vec(), rgba.width(), rgba.height())
                        .ok();
                let icon_menu =
                    muda::IconMenuItem::with_id(id, &entry.title, entry.enabled, icon, None);
                Self::set_accelerator(entry, keys_to_accelerator(&entry.shortcut), |a| {
                    icon_menu.set_key_accelerator(a)
                });
                Box::new(icon_menu)
            } else {
                Box::new(self.menu_item(entry, None))
            }
        } else {
            let id = self.register(entry);
            let sub_menu = muda::Submenu::with_id(id, &entry.title, entry.enabled);
            if depth < MAX_MENU_DEPTH {
                for child in &node.children {
                    sub_menu.append(&*self.build(child, depth + 1)).unwrap();
                }
            } else {
                sub_menu
                    .append(&muda::MenuItem::new("<Error: Menu Depth limit reached>", false, None))
                    .unwrap();
            }
            Box::new(sub_menu)
        }
    }
}

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

fn install_event_handler_if_necessary(shared_backend_data: &SharedBackendData) {
    // `MenuEvent::set_event_handler()` in `muda` seems to use `OnceCell`, which is an
    // can only be set a single time.  Therefore, we need to take care to only call this
    // a single time
    //
    // Arguably, `set_event_handler()` is unsafe
    if !MUDA_SET_EVENT_HANDLER_INSTALLED.load(Ordering::Relaxed) {
        let proxy = shared_backend_data.event_loop_proxy.clone();
        #[cfg(target_os = "macos")]
        let event_loop_generation = shared_backend_data.event_loop_generation.clone();
        muda::MenuEvent::set_event_handler(Some(move |e: muda::MenuEvent| {
            #[cfg(target_os = "macos")]
            if e.id().0 == DEFAULT_QUIT_MENU_ID {
                let generation = event_loop_generation.load(Ordering::Relaxed);
                let _ = proxy.send_event(SlintEvent(CustomEvent::Exit(generation)));
                return;
            }
            let _ = proxy.send_event(SlintEvent(CustomEvent::Muda(e)));
        }));

        MUDA_SET_EVENT_HANDLER_INSTALLED.store(true, Ordering::Relaxed);
    }
}

/// Creates the macOS application menu. Entries in `role_entries` replace the default items,
/// and `menu_item` creates the native item for such an entry.
#[cfg(target_os = "macos")]
fn create_app_menu(
    menu_bar: &muda::Menu,
    role_entries: &[MenuEntry],
    mut menu_item: impl FnMut(&MenuEntry, Option<muda::accelerator::KeyAccelerator>) -> muda::MenuItem,
) -> Result<(), i_slint_core::api::PlatformError> {
    use muda::PredefinedMenuItem;
    use muda::accelerator::{Key, KeyAccelerator, Modifiers};

    let entry = |role| role_entries.iter().find(|entry| entry.role == role);
    let title = |role| entry(role).map(|entry| entry.title.as_str());
    let command_key =
        |key: &str| Some(KeyAccelerator::new(Modifiers::META, Key::Character(key.into())));

    let mut items: Vec<Box<dyn muda::IsMenuItem>> = Vec::new();
    items.push(match entry(MenuItemRole::About) {
        Some(about) => Box::new(menu_item(about, None)),
        None => Box::new(PredefinedMenuItem::about(None, None)),
    });
    items.push(Box::new(PredefinedMenuItem::separator()));
    if let Some(preferences) = entry(MenuItemRole::Preferences) {
        items.push(Box::new(menu_item(preferences, command_key(","))));
        items.push(Box::new(PredefinedMenuItem::separator()));
    }
    items.push(Box::new(PredefinedMenuItem::services(title(MenuItemRole::Services))));
    items.push(Box::new(PredefinedMenuItem::separator()));
    items.push(Box::new(PredefinedMenuItem::hide(title(MenuItemRole::Hide))));
    items.push(Box::new(PredefinedMenuItem::hide_others(title(MenuItemRole::HideOthers))));
    items.push(Box::new(PredefinedMenuItem::show_all(title(MenuItemRole::ShowAll))));
    items.push(Box::new(PredefinedMenuItem::separator()));
    items.push(match entry(MenuItemRole::Quit) {
        Some(quit) => Box::new(menu_item(quit, command_key("q"))),
        None => {
            let quit = muda::MenuItem::with_id(
                DEFAULT_QUIT_MENU_ID,
                PredefinedMenuItem::quit(None).text(),
                true,
                None,
            );
            quit.set_key_accelerator(command_key("q"))
                .map_err(|err| i_slint_core::api::PlatformError::Other(err.to_string()))?;
            Box::new(quit)
        }
    });

    let app_menu = muda::Submenu::new("App", true);
    menu_bar
        .append(&app_menu)
        .and_then(|_| app_menu.append_items(&items.iter().map(|item| &**item).collect::<Vec<_>>()))
        .map_err(|menu_bar_err| {
            i_slint_core::api::PlatformError::Other(menu_bar_err.to_string())
        })?;
    Ok(())
}

/// On Windows, we need to disable window redraw while rebuilding the menu, otherwise
/// we might see flickering
#[allow(unused_variables)]
fn win32_set_window_redraw(winit_window: &Window, redraw: bool) {
    #[cfg(target_os = "windows")]
    if let RawWindowHandle::Win32(handle) = winit_window.window_handle().unwrap().as_raw() {
        use std::os::raw::c_void;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Foundation::WPARAM;
        use windows::Win32::UI::WindowsAndMessaging::DrawMenuBar;
        use windows::Win32::UI::WindowsAndMessaging::SendMessageW;
        use windows::Win32::UI::WindowsAndMessaging::WM_SETREDRAW;

        let hwnd = HWND(handle.hwnd.get() as *mut c_void);

        unsafe {
            SendMessageW(hwnd, WM_SETREDRAW, Some(WPARAM(redraw as usize)), None);
        }

        if redraw {
            unsafe {
                let _ = DrawMenuBar(hwnd);
            }
        }
    }
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
        let mut adapter =
            MudaAdapter { entries: Vec::new(), tracker: None, menu: None, generation: 0 };
        replace_entry(&mut adapter, id);
        adapter
    }

    fn replace_entry(adapter: &mut MudaAdapter, id: &str) {
        adapter.reset_entries();
        adapter.entries.push(MenuEntry { id: id.into(), ..Default::default() });
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

    fn item(title: &str, role: MenuItemRole) -> EntryNode {
        EntryNode {
            entry: MenuEntry { title: title.into(), role, ..Default::default() },
            children: Vec::new(),
        }
    }

    fn separator() -> EntryNode {
        EntryNode {
            entry: MenuEntry { is_separator: true, ..Default::default() },
            children: Vec::new(),
        }
    }

    fn menu(title: &str, children: Vec<EntryNode>) -> EntryNode {
        EntryNode {
            entry: MenuEntry { title: title.into(), has_sub_menu: true, ..Default::default() },
            children,
        }
    }

    fn outline(nodes: &[EntryNode]) -> Vec<String> {
        nodes
            .iter()
            .map(|node| match &node.entry {
                entry if entry.is_separator => "-".into(),
                entry if entry.has_sub_menu => {
                    format!("{}[{}]", entry.title, outline(&node.children).join(" "))
                }
                entry => entry.title.to_string(),
            })
            .collect()
    }

    fn titles(entries: &[MenuEntry]) -> Vec<&str> {
        entries.iter().map(|entry| entry.title.as_str()).collect()
    }

    #[test]
    fn role_entries_are_taken_from_any_depth() {
        use MenuItemRole::*;
        let mut nodes = vec![
            menu(
                "File",
                vec![
                    item("Open", None),
                    separator(),
                    item("Settings", Preferences),
                    separator(),
                    item("Quit", Quit),
                ],
            ),
            menu("Help", vec![menu("More", vec![item("About", About)])]),
        ];
        let taken = take_role_entries(&mut nodes);
        assert_eq!(titles(&taken), ["Settings", "Quit", "About"]);
        assert_eq!(outline(&nodes), ["File[Open]"]);
    }

    #[test]
    fn separators_around_taken_entries_collapse() {
        use MenuItemRole::*;
        let mut nodes = vec![menu(
            "File",
            vec![
                item("Quit", Quit),
                separator(),
                item("Open", None),
                separator(),
                item("Hide", Hide),
                separator(),
                item("Close", None),
            ],
        )];
        take_role_entries(&mut nodes);
        assert_eq!(outline(&nodes), ["File[Open - Close]"]);
    }

    #[test]
    fn duplicate_roles_keep_the_first_entry() {
        use MenuItemRole::*;
        let mut nodes = vec![menu(
            "File",
            vec![item("Quit", Quit), item("Hide", Hide), item("Exit", Quit), item("Hide 2", Hide)],
        )];
        let taken = take_role_entries(&mut nodes);
        assert_eq!(titles(&taken), ["Quit", "Hide", "Hide 2"]);
        assert_eq!(outline(&nodes), ["File[Exit]"]);
    }

    #[test]
    fn menus_without_roles_are_unchanged() {
        let mut nodes = vec![
            menu("File", vec![item("Open", MenuItemRole::None)]),
            item("Empty", MenuItemRole::None),
        ];
        assert!(take_role_entries(&mut nodes).is_empty());
        assert_eq!(outline(&nodes), ["File[Open]", "Empty"]);
    }
}
