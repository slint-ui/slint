// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Every wasm-bindgen-exported async method on `SlintServer` must take `&self`,
// not `&mut self`: a `&mut self` receiver is held as an exclusive borrow across
// the future's `.await` points, so re-entering from the JS event loop trips
// wasm-bindgen's borrow check (regression of #11258).
// cSpell:ignore reentrancy

// wasm_main.rs is only compiled for wasm32, so check the invariant as text.
const WASM_MAIN: &str = include_str!("../wasm_main.rs");

#[test]
fn no_exported_async_method_takes_mut_self() {
    let offenders: Vec<&str> = WASM_MAIN
        .lines()
        .map(str::trim)
        .filter(|line| line.contains("async fn") && line.contains("&mut self"))
        .collect();

    assert!(
        offenders.is_empty(),
        "exported async SlintServer methods must take `&self`, not `&mut self`:\n{}",
        offenders.join("\n"),
    );
}
