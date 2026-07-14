// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com
// SPDX-License-Identifier: MIT

fn main() {
    // Emit debug info so the integration tests can locate elements via
    // `ElementHandle`.
    let config = slint_build::CompilerConfiguration::new().with_debug_info(true);
    slint_build::compile_with_config("ui/app-window.slint", config).expect("Slint build failed");
}
