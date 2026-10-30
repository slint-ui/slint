// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint_interpreter::{Compiler, ComponentHandle, LogicalSize};

fn main() {
    i_slint_backend_selector::with_platform(|_| Ok(())).unwrap();
    let path = std::env::args().nth(1).expect("source path");
    let compiler = Compiler::default();
    let result = spin_on::spin_on(compiler.build_from_path(path));
    for diagnostic in result.diagnostics() {
        eprintln!("{diagnostic}");
    }
    let instance = result.component("App").expect("App component").create().unwrap();
    instance.window().set_size(LogicalSize::new(400., 500.));
    instance.run().unwrap();
}
