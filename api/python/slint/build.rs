// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::fs::File;
use std::io::BufWriter;

#[path = "stub-gen/language.rs"]
mod language;

fn main() {
    println!("cargo:rerun-if-changed=../../internal/common/builtin_structs.rs");
    println!("cargo:rerun-if-changed=stub-gen/language.rs");
    println!("cargo:rerun-if-changed=slint/language.pyi");

    let pyi_path = "slint/language.pyi";
    let file = File::create(pyi_path).expect("Failed to create language.pyi");
    let mut writer = BufWriter::new(file);
    language::generate(&mut writer);
}
