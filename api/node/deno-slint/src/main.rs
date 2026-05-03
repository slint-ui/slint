// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! deno-slint: a Deno-based runner with native Slint event loop integration.
//!
//! Embeds the full Deno runtime and integrates it with Slint's event loop
//! using `spawn_local` + `async_compat::Compat`.
//!
//! The slint-ui API is provided by linking the slint-node NAPI crate (as
//! rlib) into the binary.  At startup the NAPI module is registered
//! in-process (no dlopen) and the CJS `.node` handler is patched so that
//! `require('slint-ui')` returns the built-in module.
//!
//! Usage:  deno-slint my-app.ts

mod napi_registration;

use deno_node::NodeExtInitServices;
use deno_resolver::npm::{
    ByonmNpmResolverCreateOptions, CreateInNpmPkgCheckerOptions,
    DenoInNpmPackageChecker, NpmResolver, NpmResolverCreateOptions,
};
use deno_runtime::deno_core::{
    FsModuleLoader, ModuleLoadResponse, ModuleLoader,
    ModuleSpecifier, ResolutionKind,
};
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::worker::{MainWorker, WorkerOptions, WorkerServiceOptions};
use node_resolver::cache::NodeResolutionSys;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use std::borrow::Cow;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use sys_traits::impls::RealSys;

type Sys = RealSys;
type InNpmChecker = DenoInNpmPackageChecker;
type NpmFolderResolver = NpmResolver<Sys>;

// ---------------------------------------------------------------------------
// Module loader: passes node: specifiers through to the runtime,
// delegates everything else to FsModuleLoader.
// ---------------------------------------------------------------------------

struct DenoSlintModuleLoader;

impl ModuleLoader for DenoSlintModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, deno_error::JsErrorBox> {
        if specifier.starts_with("node:") {
            return Ok(ModuleSpecifier::parse(specifier)
                .map_err(deno_error::JsErrorBox::from_err)?);
        }
        FsModuleLoader.resolve(specifier, referrer, kind)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        maybe_referrer: Option<&deno_runtime::deno_core::ModuleLoadReferrer>,
        options: deno_runtime::deno_core::ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        FsModuleLoader.load(module_specifier, maybe_referrer, options)
    }
}

// ---------------------------------------------------------------------------
// NodeRequireLoader -- loads CJS files from disk for require()
// ---------------------------------------------------------------------------

struct FsNodeRequireLoader {
    pkg_json_resolver: node_resolver::PackageJsonResolverRc<Sys>,
}

impl deno_node::NodeRequireLoader for FsNodeRequireLoader {
    fn ensure_read_permission<'a>(
        &self,
        _permissions: &mut PermissionsContainer,
        path: Cow<'a, Path>,
    ) -> Result<Cow<'a, Path>, deno_error::JsErrorBox> {
        Ok(path)
    }

    fn load_text_file_lossy(
        &self,
        path: &Path,
    ) -> Result<deno_runtime::deno_core::FastString, deno_error::JsErrorBox> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?;
        Ok(deno_runtime::deno_core::FastString::from(contents))
    }

    fn is_maybe_cjs(
        &self,
        specifier: &url::Url,
    ) -> Result<bool, node_resolver::errors::PackageJsonLoadError> {
        let path = specifier.to_file_path().unwrap_or_default();
        match path.extension().and_then(|e| e.to_str()) {
            Some("cjs") => return Ok(true),
            Some("mjs") => return Ok(false),
            _ => {}
        }
        if let Some(pkg) = self.pkg_json_resolver.get_closest_package_json(&path)? {
            return Ok(pkg.typ != "module");
        }
        Ok(true)
    }
}

/// JS bootstrap: make initTesting a no-op since the testing backend
/// is already initialized from Rust when SLINT_BACKEND=testing.
const SLINT_BOOTSTRAP: &str = r#"
(function() {
    if (globalThis.__slintNapiExports?.private_api?.initTesting) {
        globalThis.__slintNapiExports.private_api.initTesting = function() {};
    }
    if (globalThis.__slintNapiExports?.initTesting) {
        globalThis.__slintNapiExports.initTesting = function() {};
    }
})();
"#;

fn main() {
    let script = match std::env::args().nth(1) {
        Some(s) => s,
        None => {
            eprintln!("Usage: deno-slint <script.js|ts>");
            std::process::exit(1);
        }
    };

    let main_module = deno_runtime::deno_core::resolve_path(
        &script,
        &std::env::current_dir().expect("failed to get cwd"),
    )
    .expect("failed to resolve script path");

    // When SLINT_BACKEND=testing, init the testing backend with mock time
    // before spawn_local creates the default (winit) backend.
    // The bootstrap JS patches initTesting() to a no-op so JS callers
    // don't try to reinitialize.
    #[cfg(feature = "testing")]
    if std::env::var("SLINT_BACKEND").as_deref() == Ok("testing") {
        i_slint_backend_testing::init_integration_test_with_mock_time();
    }

    // Ensure slint-node's napi-rs registration symbol is linked.
    napi_registration::_force_slint_node_link();

    // Create the V8 snapshot once (transpiles .ts extensions like
    // deno_telemetry).  Cached to disk so subsequent runs are fast.

    // Save MODULE_TO_REGISTER before snapshot creation clears it.
    let saved_module = napi_registration::save_module_to_register();

    static SNAPSHOT: std::sync::LazyLock<Box<[u8]>> = std::sync::LazyLock::new(|| {
        let cache_path = std::env::temp_dir().join("deno-slint-snapshot.bin");
        if let Ok(data) = std::fs::read(&cache_path) {
            return data.into_boxed_slice();
        }
        let tmp = tempfile::NamedTempFile::new_in(std::env::temp_dir())
            .expect("failed to create temp file for snapshot");
        deno_runtime::snapshot::create_runtime_snapshot(
            tmp.path().to_path_buf(),
            deno_runtime::ops::bootstrap::SnapshotOptions::default(),
            vec![],
        );
        let data = std::fs::read(tmp.path()).expect("failed to read snapshot");
        let _ = std::fs::rename(tmp.path(), &cache_path);
        data.into_boxed_slice()
    });

    // Restore MODULE_TO_REGISTER after snapshot (in case it was consumed).
    napi_registration::restore_module_to_register(saved_module);

    let deno_future = async move {
        let snapshot: &'static [u8] = &SNAPSHOT;
        let fs = Arc::new(deno_runtime::deno_fs::RealFs);
        let permission_desc_parser = Arc::new(
            deno_runtime::deno_permissions::RuntimePermissionDescriptorParser::new(RealSys),
        );

        // BYONM (Bring Your Own Node Modules) resolver for require().
        let sys = NodeResolutionSys::new(RealSys, None);
        let pkg_json_resolver: node_resolver::PackageJsonResolverRc<Sys> =
            Arc::new(node_resolver::PackageJsonResolver::new(RealSys, None));

        let in_npm_checker =
            DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Byonm);

        let npm_resolver = NpmResolver::<Sys>::new(NpmResolverCreateOptions::Byonm(
            ByonmNpmResolverCreateOptions {
                root_node_modules_dir: None,
                search_stop_dir: None,
                sys: sys.clone(),
                pkg_json_resolver: pkg_json_resolver.clone(),
            },
        ));

        let node_resolver: deno_node::NodeResolverRc<InNpmChecker, NpmFolderResolver, Sys> =
            Arc::new(node_resolver::NodeResolver::new(
                in_npm_checker.clone(),
                DenoIsBuiltInNodeModuleChecker,
                npm_resolver.clone(),
                pkg_json_resolver.clone(),
                sys,
                node_resolver::NodeResolverOptions {
                    conditions: node_resolver::NodeConditionOptions {
                        conditions: vec![],
                        import_conditions_override: None,
                        require_conditions_override: None,
                    },
                    is_browser_platform: false,
                    bundle_mode: false,
                    typescript_version: None,
                },
            ));

        let node_require_loader: deno_node::NodeRequireLoaderRc =
            Rc::new(FsNodeRequireLoader {
                pkg_json_resolver: pkg_json_resolver.clone(),
            });

        let node_services = NodeExtInitServices {
            node_require_loader,
            node_resolver,
            pkg_json_resolver,
            sys: RealSys,
        };

        let service_options: WorkerServiceOptions<InNpmChecker, NpmFolderResolver, Sys> =
            WorkerServiceOptions {
                blob_store: Default::default(),
                broadcast_channel: Default::default(),
                deno_rt_native_addon_loader: None,
                feature_checker: Default::default(),
                fs,
                module_loader: Rc::new(DenoSlintModuleLoader),
                node_services: Some(node_services),
                npm_process_state_provider: None,
                permissions: PermissionsContainer::allow_all(permission_desc_parser),
                root_cert_store_provider: None,
                fetch_dns_resolver: Default::default(),
                shared_array_buffer_store: None,
                compiled_wasm_module_store: None,
                v8_code_cache: None,
                bundle_provider: None,
            };

        let mut worker = MainWorker::bootstrap_from_options(
            &main_module,
            service_options,
            WorkerOptions {
                startup_snapshot: Some(snapshot),
                ..Default::default()
            },
        );

        // Register slint-node's NAPI module in-process (no dlopen).
        napi_registration::register_slint_napi(&mut worker);

        // Patch CJS .node loading so require('slint-ui') uses our module.
        if let Err(e) = worker
            .execute_script("[deno-slint:bootstrap]", SLINT_BOOTSTRAP.to_string().into())
        {
            eprintln!("deno-slint: bootstrap warning: {e}");
        }

        // Load the user script as CJS so that plain `require()` works
        // the same way as under node.  We create a tiny ESM wrapper that
        // uses createRequire to enter CJS mode.
        //
        // Before loading the user script, patch Module._extensions so
        // that require('slint-ui') returns the in-process NAPI module
        // instead of dlopen-ing the .node cdylib (which would create a
        // second copy of slint with separate state).
        let user_path = main_module.to_file_path().unwrap();
        let wrapper_dir = user_path.parent().unwrap_or(Path::new("."));
        let wrapper_path = wrapper_dir.join("__deno_slint_wrapper.mjs");
        let wrapper_code = format!(
            "import {{ createRequire }} from 'node:module';\n\
             import Module from 'node:module';\n\
             const require = createRequire(import.meta.url);\n\
             \n\
             // Intercept .node loading so require('slint-ui') uses the\n\
             // in-process NAPI exports instead of dlopen-ing a second copy.\n\
             const origNode = Module._extensions['.node'];\n\
             Module._extensions['.node'] = function(mod, filename) {{\n\
                 if (filename.includes('slint-ui') || filename.includes('slint_ui')) {{\n\
                     mod.exports = globalThis.__slintNapiExports;\n\
                     return;\n\
                 }}\n\
                 if (origNode) origNode(mod, filename);\n\
             }};\n\
             const origCjs = Module._extensions['.cjs'] || Module._extensions['.js'];\n\
             Module._extensions['.cjs'] = function(mod, filename) {{\n\
                 if (filename.endsWith('rust-module.cjs')) {{\n\
                     mod.exports = globalThis.__slintNapiExports;\n\
                     return;\n\
                 }}\n\
                 if (origCjs) origCjs(mod, filename);\n\
             }};\n\
             \n\
             require(String.raw`{}`);\n",
            user_path.display()
        );
        std::fs::write(&wrapper_path, &wrapper_code).expect("failed to write ESM wrapper");
        let wrapper_module = deno_runtime::deno_core::resolve_path(
            wrapper_path.to_str().unwrap(),
            &std::env::current_dir().expect("failed to get cwd"),
        )
        .expect("failed to resolve wrapper path");

        if let Err(e) = worker.execute_main_module(&wrapper_module).await {
            let _ = std::fs::remove_file(&wrapper_path);
            eprintln!("deno-slint: failed to load script: {e}");
            std::process::exit(1);
        }
        let _ = std::fs::remove_file(&wrapper_path);

        if let Err(e) = worker.run_event_loop(false).await {
            eprintln!("deno-slint: JS error: {e}");
            std::process::exit(1);
        }

        // Script finished — tell slint to stop its event loop.
        slint_node::i_slint_core::api::quit_event_loop().ok();
    };

    // Spawn the deno future onto Slint's event loop.
    i_slint_backend_selector::with_global_context(|ctx| {
        ctx.spawn_local(async_compat::Compat::new(deno_future))
    })
    .expect("backend not initialized")
    .expect("failed to spawn deno future");

    // Slint owns the main thread from here on.
    i_slint_backend_selector::with_platform(|b| b.run_event_loop())
        .expect("event loop error");
}
