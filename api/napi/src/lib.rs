// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![deny(clippy::all)]

use slint_interpreter::{ComponentCompiler, ComponentHandle};

mod interpreter;
pub use interpreter::*;

mod types;
pub use types::*;

#[macro_use]
extern crate napi_derive;

#[napi]
pub fn run(code: String) {
    let mut compiler = ComponentCompiler::default();
    let definition = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(compiler.diagnostics().is_empty());
    let instance = definition.unwrap().create().unwrap();
    instance.run().unwrap();
}
