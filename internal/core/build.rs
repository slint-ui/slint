// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use cfg_aliases::cfg_aliases;

fn main() {
    cfg_aliases! {
       // Targets where a system tray icon backend is available:
       // - macOS and Windows use the `tray-icon` crate (muda-based).
       // - Linux/BSD (unix minus Apple and Android) use `ksni`.
       // Other targets (iOS, Android, wasm, bare-metal) have no backend, so the
       // `system_tray` module is not compiled there.
       system_tray: {
           any(
               target_os = "macos",
               target_os = "windows",
               all(
                   target_family = "unix",
                   not(target_vendor = "apple"),
                   not(target_os = "android")
               )
           )
       },
    }
    println!("cargo:rustc-check-cfg=cfg(system_tray)");
}
