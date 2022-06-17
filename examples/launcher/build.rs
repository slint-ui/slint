// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

fn main() {
    slint_build::compile("launcher.slint").unwrap();
    slint_build::print_rustc_flags().unwrap();
}
