// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() -> Result<(), slint_build::CompileError> {
    let config =
        slint_build::CompilerConfiguration::new().as_library("BLogicA").rust_module("backend");
    slint_build::compile_with_config("ui/blogica.slint", config)?;

    Ok(())
}
