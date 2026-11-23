// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use cfg_aliases::cfg_aliases;

fn main() {
    slint_build::compile("slint-project.json").unwrap();
    cfg_aliases! {
       slint_gstreamer_egl: { target_os = "linux" },
    }
}
