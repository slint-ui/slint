// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() -> Result<(), slint_build::CompileError> {
    let config = slint_build::CompilerConfiguration::new().as_library("BLogicB");
    slint_build::compile_with_config("ui/blogicb.slint", config)?;

    Ok(())
}
