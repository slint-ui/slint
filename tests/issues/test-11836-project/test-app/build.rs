// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    let config = slint_build::CompilerConfiguration::new();
    slint_build::compile_with_config("ui/app-window.slint", config).expect("Slint build failed");
}
