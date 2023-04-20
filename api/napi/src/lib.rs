#![deny(clippy::all)]

use slint_interpreter::{
    ComponentCompiler, ComponentDefinition, ComponentHandle, SharedString, Value,
};

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
