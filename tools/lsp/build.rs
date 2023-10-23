// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

fn main() {
    // Make the compiler handle ComponentContainer:
    // TODO: Remove if ComponentContainer anc component-factory no longer need this!
    std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    #[cfg(feature = "preview-engine")]
    slint_build::compile_with_config(
        "ui/main.slint",
        slint_build::CompilerConfiguration::new().with_style("fluent-light".into()),
    )
    .unwrap();
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");
}
