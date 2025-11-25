// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    println!("cargo:warning=ENV={:?}", std::env::vars().collect::<Vec<_>>());

    println!("cargo:warning=FONTCONFIG={:?}", pkg_config::find_library("fontconfig"))
}
