/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Tobias Hunger <tobias.hunger@gmail.com>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

#![no_main]
use libfuzzer_sys::fuzz_target;

use sixtyfps_compilerlib::diagnostics::BuildDiagnostics;
use sixtyfps_compilerlib::parser;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let mut bd = BuildDiagnostics::default();
        let _node = parser::parse(s.to_string(), None, &mut bd);
    }
});
