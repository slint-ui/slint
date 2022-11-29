// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

fn main() {
    slint_build::compile_with_config(
        "main.slint",
        slint_build::CompilerConfiguration::new().with_style("material".into()),
    )
    .unwrap();
}
