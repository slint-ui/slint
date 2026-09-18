// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// A native menu bar is kept alive by the window adapter so that a later menu event can activate the
// selected entry. But the menu owns an item tree, which owns a strong reference back to the adapter
// through the globals: unless the adapter releases it, it outlives its own component and leaks its
// resources (including the GPU objects held by the renderer). (#12971)
#[satchel::test]
fn native_menu_bar_does_not_leak_the_window_adapter() {
    slint::slint! {
        export component App inherits Window {
            MenuBar {
                Menu {
                    title: "File";
                    MenuItem { title: "Quit"; }
                }
            }
        }
    }

    let app = App::new().unwrap();

    let adapter = std::rc::Rc::downgrade(
        &i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter(),
    );

    app.hide().unwrap();
    drop(app);

    assert!(adapter.upgrade().is_none(), "the window adapter outlived its component");
}
