// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[satchel::test]
fn platform_is_app() {
    slint::slint! {
        export component MainWindow inherits Window {
            out property <bool> is_app: Platform.is-app;
        }
    }

    let app = MainWindow::new().unwrap();
    assert_eq!(app.get_is_app(), true, "Platform.is-app must be true for a real compiled application");
}
