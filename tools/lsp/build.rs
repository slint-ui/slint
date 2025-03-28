// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    // Make the compiler handle ComponentContainer:
    std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    #[cfg(not(target_os = "macos"))]
    {
        slint_build::compile("ui/main.slint").unwrap();
    }

    #[cfg(feature = "preview-engine")]
    {
        #[cfg(target_os = "macos")]
        {
            let config = slint_build::CompilerConfiguration::new()
                .with_style("fluent".into());
            slint_build::compile_with_config("ui/main.slint", config).unwrap();
        }
    }
}
