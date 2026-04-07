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
use std::cell::RefCell;
use std::rc::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use winit::event_loop::EventLoopProxy;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

pub struct MudaAdapter {
    entries: Vec<MenuEntry>,
    tracker: Option<Pin<Box<PropertyTracker<false, MudaPropertyTracker>>>>,
    menu: RefCell<Option<muda::Menu>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumString, strum::Display)]
pub enum MudaType {
    Menubar,
    Context,
}

static MUDA_SET_EVENT_HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);

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
        install_event_handler_if_necessary(proxy);

        let tracker =
            Some(Box::pin(PropertyTracker::new_with_dirty_handler(MudaPropertyTracker {
                window_adapter_weak,
            })));

        let mut s = Self { entries: Default::default(), tracker, menu: RefCell::new(None) };
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

        let mut s = Self { entries: Default::default(), tracker: None, menu: RefCell::new(None) };
        s.rebuild_menu(winit_window, Some(context_menu), MudaType::Context);

        match winit_window.window_handle().ok()?.as_raw() {
            #[cfg(target_os = "windows")]
            RawWindowHandle::Win32(handle) => {
                let position = i_slint_core::api::WindowPosition::Logical(position);
                let position = crate::winitwindowadapter::position_to_winit(&position);
                unsafe {
                    s.menu.borrow().as_ref().unwrap().show_context_menu_for_hwnd(handle.hwnd.get(), Some(position));
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
                unsafe { s.menu.borrow().as_ref().unwrap().show_context_menu_for_nsview(handle.ns_view.as_ptr(), position) };
                Some(s)
            }
            _ => None,
        }
    }

    pub fn rebuild_menu(
        &mut self,
        winit_window: &Window,
        menubar: Option<&vtable::VRc<MenuVTable>>,
        muda_type: MudaType,
    ) {
        let must_set_window_redraw = cfg!(windows) && winit_window.is_visible() == Some(true);
        if must_set_window_redraw {
            win32_set_window_redraw(winit_window, false);
        }

        let mut menu = self.menu.borrow_mut();

        // clear the menu
        self.entries.clear();

        fn generate_menu_entry(
            menu: vtable::VRef<'_, MenuVTable>,
            entry: &MenuEntry,
            depth: usize,
            map: &mut Vec<MenuEntry>,
            window_id: &str,
            muda_type: MudaType,
        ) -> Box<dyn muda::IsMenuItem> {
            let id = muda::MenuId(format!("{window_id}|{}|{}", map.len(), muda_type));
            map.push(entry.clone());
            if entry.is_separator {
                Box::new(muda::PredefinedMenuItem::separator())
            } else if !entry.has_sub_menu {
                let accelerator = keys_to_accelerator(&entry.shortcut)
                    .map_err(|_err| {
                        i_slint_core::debug_log!(
                            "Warning: Cannot convert {} to native shortcut - it will not be listed in the menu",
                            entry.shortcut
                        )
                    })
                    .unwrap_or(None);

                // the top level always has a sub menu regardless of entry.has_sub_menu
                if entry.checkable {
                    Box::new(muda::CheckMenuItem::with_id(
                        id.clone(),
                        &entry.title,
                        entry.enabled,
                        entry.checked,
                        accelerator,
                    ))
                } else if let Some(rgba) = entry.icon.to_rgba8() {
                    let icon = muda::Icon::from_rgba(
                        rgba.as_bytes().to_vec(),
                        rgba.width(),
                        rgba.height(),
                    )
                    .ok();
                    Box::new(muda::IconMenuItem::with_id(
                        id.clone(),
                        &entry.title,
                        entry.enabled,
                        icon,
                        accelerator,
                    ))
                } else {
                    Box::new(muda::MenuItem::with_id(
                        id.clone(),
                        &entry.title,
                        entry.enabled,
                        accelerator,
                    ))
                }
            } else {
                let sub_menu = muda::Submenu::with_id(id.clone(), &entry.title, entry.enabled);
                if depth < 15 {
                    let mut sub_entries = Default::default();
                    menu.sub_menu(Some(entry), &mut sub_entries);
                    for e in sub_entries {
                        sub_menu
                            .append(&*generate_menu_entry(
                                menu,
                                &e,
                                depth + 1,
                                map,
                                window_id,
                                muda_type,
                            ))
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
        if matches!(muda_type, MudaType::Menubar) {
            create_default_app_menu(&self.menu).unwrap();
        }

        if let Some(menubar) = menubar.as_deref() {
            let mut build_menu = || {
                let mut menu_entries = Default::default();
                vtable::VRc::borrow(&menubar).sub_menu(None, &mut menu_entries);

                if menu_entries.is_empty() && muda_type == MudaType::Menubar {
                    *menu = None;
                }
                else if let Some(menu) = menu.as_ref() {
                    while menu.remove_at(0).is_some() {}
                } else {
                    *menu = Some(muda::Menu::new());

                    #[cfg(target_os = "windows")]
                    if let RawWindowHandle::Win32(handle) = winit_window.window_handle().unwrap().as_raw() {
                        let theme = match winit_window.theme() {
                            Some(winit::window::Theme::Dark) => muda::MenuTheme::Dark,
                            Some(winit::window::Theme::Light) => muda::MenuTheme::Light,
                            None => muda::MenuTheme::Auto,
                        };
                        unsafe { menu.as_ref().unwrap().init_for_hwnd_with_theme(handle.hwnd.get(), theme).unwrap() };
                    }

                    #[cfg(target_os = "macos")]
                    {
                        menu.as_ref().unwrap().init_for_nsapp();
                    }
                }

                let window_id = u64::from(winit_window.id()).to_string();
                for e in menu_entries {
                    menu.as_ref().unwrap()
                        .append(&*generate_menu_entry(
                            vtable::VRc::borrow(&menubar),
                            &e,
                            0,
                            &mut self.entries,
                            &window_id,
                            muda_type,
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

        if must_set_window_redraw {
            win32_set_window_redraw(winit_window, true);
        }
    }

    pub fn invoke(&self, menubar: &vtable::VRc<MenuVTable>, entry_id: usize) {
        let Some(entry) = &self.entries.get(entry_id) else { return };
        vtable::VRc::borrow(&menubar).activate(entry);
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
        if let RawWindowHandle::Win32(handle) = winit_window.window_handle().unwrap().as_raw() {
            unsafe { self.menu.borrow().as_ref().unwrap().set_theme_for_hwnd(handle.hwnd.get(), theme).unwrap() };
        }
    }

    #[cfg(target_os = "macos")]
    pub fn setup_default_menu_bar() -> Result<Self, i_slint_core::api::PlatformError> {
        let menu_bar = muda::Menu::new();
        create_default_app_menu(&menu_bar)?;
        menu_bar.init_for_nsapp();
        Ok(Self { entries: Vec::new(), menu: RefCell::new(Some(menu_bar)), tracker: None })
    }

    #[cfg(target_os = "macos")]
    pub fn window_activation_changed(&self, is_active: bool) {
        if is_active {
            self.menu.init_for_nsapp();
        }
    }
}

fn key_string_to_code(string: &str) -> Option<muda::accelerator::Code> {
    use muda::accelerator::Code::*;
    macro_rules! key_string_to_code_impl {
        ($($char:literal # $_name:ident # $($_shifted:ident)? # $($muda:ident)? $(=> $($_qt:ident)|* # $($_winit:ident $(($_pos:ident))?)|* # $($_xkb:ident)|*)?;)*) => {
            match string.chars().next()? {
                $($($char => Some($muda),)?)*
                _ => None,
            }
        };
    }
    i_slint_common::for_each_keys!(key_string_to_code_impl)
}

fn keys_to_accelerator(
    keys: &i_slint_core::input::Keys,
) -> Result<Option<muda::accelerator::Accelerator>, ()> {
    use muda::accelerator::*;

    if *keys == i_slint_core::input::Keys::default() {
        return Ok(None);
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
    let key = key_string_to_code(&shortcut.key).ok_or(())?;

    Ok(Some(Accelerator::new(Some(modifiers), key)))
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
