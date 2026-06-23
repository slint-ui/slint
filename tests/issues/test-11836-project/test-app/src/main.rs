// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use issue_11836_test_library::test_library;
use slint::ComponentHandle;

slint::include_modules!();

fn main() {
    let ui = AppWindow::new().unwrap();

    test_library::init(&ui);
    ui.run().unwrap();
}
