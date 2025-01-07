// Copyright © 2025 David Haig
// SPDX-License-Identifier: MIT

fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
    slint_build::compile_with_config("../ui/main.slint", config).unwrap();
    slint_build::print_rustc_flags().unwrap();
}
