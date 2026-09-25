// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// The native context menu is kept alive by the window adapter so that a later menu event can still
// be delivered. But the menu owns an item tree, which owns a strong reference back to the adapter
// through the globals: unless the adapter releases it, it outlives its own component and leaks. (#12971)
//
// The window is deliberately never shown: without a native window there is nothing to pop the menu
// up in, so the backend falls back to the Slint popup instead of entering the platform's modal menu
// loop, which no test could dismiss.
#[satchel::test]
fn native_context_menu_does_not_leak_the_window_adapter() {
    slint::slint! {
        export component App inherits Window {
            menu := ContextMenuArea {
                Menu {
                    MenuItem { title: "Item"; }
                }
            }
            public function show-menu() { menu.show({ x: 0, y: 0 }); }
            public function close-menu() { menu.close(); }
        }
    }

    let app = App::new().unwrap();
    app.invoke_show_menu();
    // An open popup keeps the adapter alive on its own, so close it: the menu is what's tested.
    app.invoke_close_menu();

    let adapter = std::rc::Rc::downgrade(
        &i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter(),
    );

    app.hide().unwrap();
    drop(app);

    assert!(adapter.upgrade().is_none(), "the window adapter outlived its component");
}
