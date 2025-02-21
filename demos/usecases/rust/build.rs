// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint_build::CompilerConfiguration;

fn main() {
    slint_build::compile_with_config(
        "../ui/app.slint",
        CompilerConfiguration::new().with_style("cosmic".into()),
    )
    .unwrap();
}
