/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
# SixtyFPS interpreter library

**NOTE:** This library is an internal crate for the SixtyFPS project.
This crate should not be used directly by application using SixtyFPS.
You should use the `sixtyfps` crate instead
*/
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]

mod dynamic_component;
mod dynamic_type;
mod eval;
mod global_component;
mod value_model;

/// FIXME: re-export everything from abi, but and everything else should be private
pub mod api;

pub use eval::{ModelPtr, Value};

pub use sixtyfps_compilerlib::CompilerConfiguration;
use sixtyfps_corelib::component::ComponentVTable;
use std::rc::Rc;

pub type ComponentDescription = dynamic_component::ComponentDescription<'static>;
pub type ComponentBox = dynamic_component::ComponentBox<'static>;
pub type ComponentRc = vtable::VRc<ComponentVTable, dynamic_component::ErasedComponentBox>;
pub async fn load(
    source: String,
    path: std::path::PathBuf,
    mut compiler_config: CompilerConfiguration,
) -> (Result<Rc<ComponentDescription>, ()>, sixtyfps_compilerlib::diagnostics::BuildDiagnostics) {
    if compiler_config.style.is_none() && std::env::var("SIXTYFPS_STYLE").is_err() {
        // Defaults to native if it exists:
        compiler_config.style = Some(if sixtyfps_rendering_backend_default::HAS_NATIVE_STYLE {
            "native".to_owned()
        } else {
            "ugly".to_owned()
        });
    }
    dynamic_component::load(source, path, compiler_config, unsafe {
        generativity::Guard::new(generativity::Id::new())
    })
    .await
}

pub fn run_event_loop() {
    sixtyfps_rendering_backend_default::backend().run_event_loop();
}

pub fn register_font_from_path<P: AsRef<std::path::Path>>(
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    sixtyfps_rendering_backend_default::backend().register_font_from_path(path.as_ref())
}

pub fn register_font_from_memory(data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    sixtyfps_rendering_backend_default::backend().register_font_from_memory(data)
}
