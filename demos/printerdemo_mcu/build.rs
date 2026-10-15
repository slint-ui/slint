// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    println!("cargo:rerun-if-env-changed=SLINT_INLINING");
    let config = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
    slint_build::compile_with_config("ui/printerdemo.slint", config).unwrap();
    slint_build::print_rustc_flags().unwrap();
}
