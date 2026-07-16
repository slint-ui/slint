// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// The expansion of cfg_aliases! places macro-emitted semicolons in expression
// position, which nightly rejects by default since 2026-07. Allow it until a
// fixed cfg-aliases release is available.
#![allow(semicolon_in_expressions_from_macros)]

use cfg_aliases::cfg_aliases;

fn main() {
    slint_build::compile("scene.slint").unwrap();
    cfg_aliases! {
       slint_gstreamer_egl: { target_os = "linux" },
    }
}
