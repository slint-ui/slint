// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

use std::any::Any;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use slint_interpreter::ComponentHandle;

use deno_core::error::AnyError;
// use deno_core::op2;
use deno_core::FsModuleLoader;
use deno_core::ModuleSpecifier;
use deno_fs::RealFs;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::worker::WorkerServiceOptions;
use slint_interpreter::Compiler;
use slint_interpreter::EventLoopError;
use slint_interpreter::JoinHandle;

// #[op2(fast)]
// fn op_hello(#[string] text: &str) {
//   println!("Hello {} from an op!", text);
// }

// deno_core::extension!(
//   hello_runtime,
//   ops = [op_hello],
//   esm_entry_point = "ext:hello_runtime/bootstrap.js",
//   esm = [dir "examples/extension", "bootstrap.js"]
// );

pub fn main() -> Result<(), AnyError> {
    let deno_future = async move {
        let js_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/simple.js");
        let main_module = ModuleSpecifier::from_file_path(js_path).unwrap();
        eprintln!("Running {main_module}...");
        let fs = Arc::new(RealFs);
        let permission_desc_parser =
            Arc::new(RuntimePermissionDescriptorParser::new(sys_traits::impls::RealSys));
        let mut worker = MainWorker::bootstrap_from_options(
            main_module.clone(),
            WorkerServiceOptions::<sys_traits::impls::RealSys> {
                module_loader: Rc::new(FsModuleLoader),
                permissions: PermissionsContainer::allow_all(permission_desc_parser),
                blob_store: Default::default(),
                broadcast_channel: Default::default(),
                feature_checker: Default::default(),
                node_services: Default::default(),
                npm_process_state_provider: Default::default(),
                root_cert_store_provider: Default::default(),
                fetch_dns_resolver: Default::default(),
                shared_array_buffer_store: Default::default(),
                compiled_wasm_module_store: Default::default(),
                v8_code_cache: Default::default(),
                fs,
            },
            WorkerOptions {
                // extensions: vec![hello_runtime::init_ops_and_esm()],
                extensions: vec![],
                ..Default::default()
            },
        );

        let _ = worker.execute_main_module(&main_module).await;
        worker.run_event_loop(false).await.unwrap();
        slint_interpreter::quit_event_loop().unwrap();
    };

    slint_interpreter::spawn_local(async_compat::Compat::new(deno_future)).unwrap();

    let slint_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/simple.slint");

    let compiler = Compiler::new();
    let r = smol::block_on(compiler.build_from_path(slint_path));

    let definitions: Vec<slint_interpreter::ComponentDefinition> = r.components().collect();

    let definition = definitions.first().unwrap();

    let instance = definition.create()?;
    instance.show()?;

    slint_interpreter::run_event_loop().unwrap();

    Ok(())
}
