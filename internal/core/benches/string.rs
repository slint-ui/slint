// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use criterion::{Criterion, criterion_group, criterion_main};
use i_slint_core::string::SharedString;

const REFERENCE_NUMBER: &str = "2384.2345345345";
const RESULT_NUMBER: &str = "2384,2345345345";

fn string_replace(c: &mut Criterion) {
    c.bench_function("string_replace", |b| {
        b.iter(|| {
            let string = SharedString::from(REFERENCE_NUMBER);
            let string = string.replace('.', ",");
            assert_eq!(string, RESULT_NUMBER);
        });
    });
}

fn string_replacen(c: &mut Criterion) {
    c.bench_function("string_replacen", |b| {
        b.iter(|| {
            let string = SharedString::from(REFERENCE_NUMBER);
            let string = string.replacen('.', ",", 1);
            assert_eq!(string, RESULT_NUMBER);
        });
    });
}

fn string_replace_own_character(c: &mut Criterion) {
    c.bench_function("string_replace_own_character", |b| {
        b.iter(|| {
            let mut string = SharedString::from(REFERENCE_NUMBER);
            string.replace_characters('.', ',', 1);
            assert_eq!(string, RESULT_NUMBER);
        });
    });
}

criterion_group!(benches, string_replace, string_replacen, string_replace_own_character);
criterion_main!(benches);
