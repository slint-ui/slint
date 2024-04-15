// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Community OR LicenseRef-Slint-commercial

#![deny(warnings)]

pub mod testing;

#[cfg(test)]
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

fn main() {
    println!("Nothing to see here, please run me through cargo test :)");
}
