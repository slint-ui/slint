// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com
// SPDX-License-Identifier: MIT

fn main() {
    // Emit debug info so the integration tests can locate elements via
    // `ElementHandle`.
    let config = slint_build::CompilerConfiguration::new().with_debug_info(true);
    slint_build::compile_with_config(".", config.clone()).expect("Slint build failed");
    let output_directory = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::copy(output_directory.join("app-window.rs"), output_directory.join("directory.rs"))
        .unwrap();
    slint_build::compile_with_config("slint-project.json", config).expect("Slint build failed");
}
