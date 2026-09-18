// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[satchel::test]
fn platform_uses_mock_data() {
    slint::slint! {
        export component MainWindow inherits Window {
            out property <bool> uses_mock_data: Platform.uses-mock-data;
        }
    }

    let app = MainWindow::new().unwrap();
    assert_eq!(
        app.get_uses_mock_data(),
        false,
        "Platform.uses-mock-data must be false for a real compiled application"
    );
}
