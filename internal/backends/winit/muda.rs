// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::SlintUserEvent;
use i_slint_core::window::MenuVTable;
use winit::event_loop::EventLoopProxy;
use winit::window::Window;

pub struct MudaAdapter {
    entries: Vec<i_slint_core::items::MenuEntry>,
    menubar: vtable::VBox<MenuVTable>,
    // We need to keep menu alive, otherwise muda segfaults
    _menu: muda::Menu,
}

impl MudaAdapter {
    pub fn setup(
        menubar: vtable::VBox<MenuVTable>,
        winit_window: &Window,
        proxy: EventLoopProxy<SlintUserEvent>,
    ) -> Self {
        type MenuRef<'a> = vtable::VRef<'a, MenuVTable>;
        type EntryMap = Vec<i_slint_core::items::MenuEntry>;
        fn generate_menu_entry(
            menu: MenuRef,
            entry: &i_slint_core::items::MenuEntry,
            depth: usize,
            map: &mut EntryMap,
            window_id: &str,
        ) -> Box<dyn muda::IsMenuItem> {
            let id = muda::MenuId(format!("{window_id}|{}", map.len()));
            map.push(entry.clone());
            // the top level always has a sub menu regardless of entry.has_sub_menu
            if !entry.has_sub_menu && depth != 0 {
                Box::new(muda::MenuItem::with_id(
                    id.clone(),
                    &entry.title,
                    true, /*entry.enabled*/
                    None,
                ))
            } else {
                let sub_menu =
                    muda::Submenu::with_id(id.clone(), &entry.title, true /*entry.enabled*/);
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

        let menu = muda::Menu::new();
        let mut map = EntryMap::new();
        let mut menu_entries = Default::default();
        menubar.sub_menu(None, &mut menu_entries);
        let window_id = u64::from(winit_window.id()).to_string();
        for e in menu_entries {
            menu.append(&*generate_menu_entry(menubar.borrow(), &e, 0, &mut map, &window_id))
                .unwrap();
        }

        muda::MenuEvent::set_event_handler(Some(move |e| {
            let _ = proxy.send_event(SlintUserEvent(crate::event_loop::CustomEvent::Muda(e)));
        }));

        #[cfg(target_os = "windows")]
        {
            use winit::raw_window_handle::*;
            if let RawWindowHandle::Win32(handle) = winit_window.window_handle().unwrap().as_raw() {
                unsafe { menu.init_for_hwnd(handle.hwnd.get()).unwrap() };
            }
        }
        #[cfg(target_os = "macos")]
        {
            menu.init_for_nsapp();
        }

        Self { entries: map, menubar, _menu: menu }
    }

    pub fn invoke(&self, entry_id: usize) {
        let Some(entry) = &self.entries.get(entry_id) else { return };
        self.menubar.activate(entry);
    }
}
