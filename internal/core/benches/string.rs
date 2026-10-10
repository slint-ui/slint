// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::string::SharedString;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

const REFERENCE_NUMBER: &str = "2384.2345345345";
const RESULT_NUMBER: &str = "2384,2345345345";

#[divan::bench]
fn string_replace() {
    let string = SharedString::from(REFERENCE_NUMBER);
    let string = string.replace('.', ",");
    divan::black_box(&string);
    assert_eq!(string, RESULT_NUMBER);
}

#[divan::bench]
fn string_replacen() {
    let string = SharedString::from(REFERENCE_NUMBER);
    let string = string.replacen('.', ",", 1);
    divan::black_box(&string);
    assert_eq!(string, RESULT_NUMBER);
}

#[divan::bench]
fn string_replace_own_character() {
    let mut string = SharedString::from(REFERENCE_NUMBER);
    string.replace_characters('.', ',', 1);
    divan::black_box(&string);
    assert_eq!(string, RESULT_NUMBER);
}

fn main() {
    divan::main();
}
