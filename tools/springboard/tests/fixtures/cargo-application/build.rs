// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::PathBuf;

fn main() {
    println!("cargo::rerun-if-changed=src");
    println!("cargo::rerun-if-changed=ui/app.slint");

    let target_directory =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("target");
    std::fs::create_dir_all(&target_directory).unwrap();
    let counter_path = target_directory.join("springboard-build-count");
    let count = std::fs::read_to_string(&counter_path)
        .ok()
        .and_then(|count| count.trim().parse::<u32>().ok())
        .unwrap_or_default();
    std::fs::write(counter_path, (count + 1).to_string()).unwrap();
}
