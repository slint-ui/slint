// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
use winit::event_loop::EventLoopProxy;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

pub struct MudaAdapter {
    entries: Vec<MenuEntry>,
    tracker: Option<Pin<Box<PropertyTracker<MudaPropertyTracker>>>>,
    menu: muda::Menu,
}

#[derive(Clone, Copy, Debug)]
pub enum MudaType {
    Menubar,
    Context,
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
        proxy: EventLoopProxy<SlintEvent>,
        window_adapter_weak: Weak<WinitWindowAdapter>,
    ) -> Self {
        let menu = muda::Menu::new();

        muda::MenuEvent::set_event_handler(Some(move |e| {
            let _ = proxy.send_event(SlintEvent(CustomEvent::Muda(e, MudaType::Menubar)));
        }));

        #[cfg(target_os = "windows")]
        if let RawWindowHandle::Win32(handle) = winit_window.window_handle().unwrap().as_raw() {
            unsafe { menu.init_for_hwnd(handle.hwnd.get()).unwrap() };
        }

        #[cfg(target_os = "macos")]
        {
            menu.init_for_nsapp();
        }

        let tracker =
            Some(Box::pin(PropertyTracker::new_with_dirty_handler(MudaPropertyTracker {
                window_adapter_weak,
            })));

        let mut s = Self { entries: Default::default(), tracker, menu };
        s.rebuild_menu(winit_window, Some(menubar));
        s
    }

    pub fn show_context_menu(
        context_menu: &vtable::VRc<MenuVTable>,
        winit_window: &Window,
        position: LogicalPosition,
        proxy: EventLoopProxy<SlintEvent>,
    ) -> Option<Self> {
        if cfg!(target_os = "macos") {
            // TODO: Implement this on macOS (Note that rebuild_menu must not create the default app)
            return None;
        }

        let menu = muda::Menu::new();

        muda::MenuEvent::set_event_handler(Some(move |e| {
            let _ = proxy.send_event(SlintEvent(CustomEvent::Muda(e, MudaType::Context)));
        }));

        let mut s = Self { entries: Default::default(), tracker: None, menu };
        s.rebuild_menu(winit_window, Some(context_menu));

        let position = i_slint_core::api::WindowPosition::Logical(position);
        let position = Some(crate::winitwindowadapter::position_to_winit(&position));

        match winit_window.window_handle().ok()?.as_raw() {
            #[cfg(target_os = "windows")]
            RawWindowHandle::Win32(handle) => {
                unsafe {
                    s.menu.show_context_menu_for_hwnd(handle.hwnd.get(), position);
                }
                Some(s)
            }
            #[cfg(target_os = "macos")]
            RawWindowHandle::AppKit(handle) => {
                unsafe { s.menu.show_context_menu_for_nsview(handle.ns_view.as_ptr(), position) };
                Some(s)
            }
            _ => None,
        }
    }

    pub fn rebuild_menu(
        &mut self,
        winit_window: &Window,
        menubar: Option<&vtable::VRc<MenuVTable>>,
    ) {
        // clear the menu
        while self.menu.remove_at(0).is_some() {}
        self.entries.clear();

        fn generate_menu_entry(
            menu: vtable::VRef<'_, MenuVTable>,
            entry: &MenuEntry,
            depth: usize,
            map: &mut Vec<MenuEntry>,
            window_id: &str,
        ) -> Box<dyn muda::IsMenuItem> {
            let id = muda::MenuId(format!("{window_id}|{}", map.len()));
            map.push(entry.clone());
            if entry.is_separator {
                Box::new(muda::PredefinedMenuItem::separator())
            } else if !entry.has_sub_menu {
                // the top level always has a sub menu regardless of entry.has_sub_menu
                let icon = entry
                    .icon
                    .to_rgba8()
                    .map(|rgba| {
                        muda::Icon::from_rgba(rgba.as_bytes().to_vec(), rgba.width(), rgba.height())
                            .ok()
                    })
                    .flatten();
                Box::new(muda::IconMenuItem::with_id(
                    id.clone(),
                    &entry.title,
                    entry.enabled,
                    icon,
                    None,
                ))
            } else {
                let sub_menu = muda::Submenu::with_id(id.clone(), &entry.title, entry.enabled);
                if depth < 15 {
                    let mut sub_entries = Default::default();
                    menu.sub_menu(Some(entry), &mut sub_entries);
                    for e in sub_entries {
                        sub_menu
                            .append(&*generate_menu_entry(menu, &e, depth + 1, map, window_id))
                            .unwrap();
                    }
                } else {
                    // infinite menu depth is possible, but we want to limit the amount of item passed to muda
                    sub_menu
                        .append(&muda::MenuItem::new(
                            "<Error: Menu Depth limit reached>",
                            false,
                            None,
                        ))
                        .unwrap();
                }
                Box::new(sub_menu)
            }
        }

        // Until we have menu roles, always create an app menu on macOS.
        #[cfg(target_os = "macos")]
        create_default_app_menu(&self.menu).unwrap();

        if let Some(menubar) = menubar.as_deref() {
            let mut build_menu = || {
                let mut menu_entries = Default::default();
                vtable::VRc::borrow(&menubar).sub_menu(None, &mut menu_entries);
                let window_id = u64::from(winit_window.id()).to_string();
                for e in menu_entries {
                    self.menu
                        .append(&*generate_menu_entry(
                            vtable::VRc::borrow(&menubar),
                            &e,
                            0,
                            &mut self.entries,
                            &window_id,
                        ))
                        .unwrap();
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
        let Some(entry) = &self.entries.get(entry_id) else { return };
        vtable::VRc::borrow(&menubar).activate(entry);
    }

    #[cfg(target_os = "macos")]
    pub fn setup_default_menu_bar() -> Result<Self, i_slint_core::api::PlatformError> {
        let menu_bar = muda::Menu::new();
        create_default_app_menu(&menu_bar)?;
        menu_bar.init_for_nsapp();
        Ok(Self { entries: vec![], menu: menu_bar, tracker: None })
    }

    #[cfg(target_os = "macos")]
    pub fn window_activation_changed(&self, is_active: bool) {
        if is_active {
            self.menu.init_for_nsapp();
        } else {
            self.menu.remove_for_nsapp();
        }
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
