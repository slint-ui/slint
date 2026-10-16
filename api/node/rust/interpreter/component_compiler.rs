// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::JsComponentDefinition;
use super::JsDiagnostic;
use i_slint_compiler::langtype::Type;
use itertools::Itertools;
use napi::bindgen_prelude::*;
use napi::{Env, JsValue};
use slint_interpreter::{CompilationResult, Compiler, Value};
use smol_str::StrExt;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Wake};

/// ComponentCompiler is the entry point to the Slint interpreter that can be used
/// to load .slint files or compile them on-the-fly from a string.
#[napi(js_name = "ComponentCompiler")]
pub struct JsComponentCompiler {
    internal: Compiler,
    file_loader: Option<FileLoader>,
    output: Rc<RefCell<BuildOutput>>,
}

#[derive(Default)]
struct BuildOutput {
    structs_and_enums: Vec<Type>,
    diagnostics: Vec<slint_interpreter::Diagnostic>,
}

impl BuildOutput {
    fn take_from(&mut self, result: &CompilationResult) -> HashMap<String, JsComponentDefinition> {
        self.structs_and_enums =
            result.structs_and_enums(i_slint_core::InternalToken {}).cloned().collect();
        self.diagnostics = result.diagnostics().collect();
        result.components().map(|c| (c.name().to_owned(), c.into())).collect()
    }
}

/// The JavaScript function set as `fileLoader`.
#[derive(Clone)]
struct FileLoader {
    env: Env,
    callback: Rc<FunctionRef<crate::DynArgs, Unknown<'static>>>,
}

enum Loaded {
    Source(Option<std::io::Result<String>>),
    Promise(PromiseRaw<'static, Unknown<'static>>),
}

impl FileLoader {
    fn call(&self, path: &Path) -> Loaded {
        let called = (|| {
            let path = self.env.create_string(path.display().to_string())?;
            let result =
                self.callback.borrow_back(&self.env)?.call(crate::DynArgs(vec![path.raw()]))?;
            Ok(if result.is_promise()? {
                Loaded::Promise(PromiseRaw::from_unknown(result)?)
            } else {
                Loaded::Source(loaded_source(result))
            })
        })();
        called.unwrap_or_else(|e| Loaded::Source(Some(Err(io_error(e)))))
    }

    /// Loads files for a synchronous build, which can't wait for a promise.
    fn load_sync(&self, path: &Path) -> Option<std::io::Result<String>> {
        let promise = match self.call(path) {
            Loaded::Source(source) => return source,
            Loaded::Promise(promise) => promise,
        };
        // Node.js terminates on an unhandled rejection.
        let _ = promise.catch(|_: CallbackContext<Unknown>| Ok(()));
        Some(Err(std::io::Error::other(
            "the file loader returned a Promise, which only loadFileAsync() and loadSourceAsync() wait for",
        )))
    }

    /// Loads files for `build`, which resumes when a promise that the file loader returns settles.
    fn load_async(
        &self,
        build: &Weak<AsyncBuild>,
        path: &Path,
    ) -> Pin<Box<dyn Future<Output = Option<std::io::Result<String>>>>> {
        let promise = match self.call(path) {
            Loaded::Source(source) => return Box::pin(std::future::ready(source)),
            Loaded::Promise(promise) => promise,
        };
        let settled = Rc::new(RefCell::new(None));
        let settle = Rc::new({
            let settled = settled.clone();
            let build = build.upgrade();
            move |source| {
                *settled.borrow_mut() = Some(source);
                if let Some(build) = &build {
                    build.resume();
                }
            }
        });
        let on_resolve = {
            let settle = settle.clone();
            move |ctx: CallbackContext<Unknown>| {
                settle(loaded_source(ctx.value));
                Ok(())
            }
        };
        let on_reject = move |ctx: CallbackContext<Unknown>| {
            settle(Some(Err(io_error(napi::Error::from_unknown_without_coercion(ctx.value)))));
            Ok(())
        };
        if let Err(e) = promise.then(on_resolve).and_then(|p| p.catch(on_reject)) {
            return Box::pin(std::future::ready(Some(Err(io_error(e)))));
        }
        Box::pin(std::future::poll_fn(move |_| match settled.borrow_mut().take() {
            Some(source) => Poll::Ready(source),
            None => Poll::Pending,
        }))
    }
}

/// Runs a build on the JavaScript thread, without blocking it while the file loader's promises settle.
struct AsyncBuild {
    env: Env,
    future: RefCell<Option<Pin<Box<dyn Future<Output = CompilationResult>>>>>,
    woken: Arc<WakeFlag>,
    on_done: Cell<Option<FunctionRef<crate::DynArgs, Unknown<'static>>>>,
    output: Rc<RefCell<BuildOutput>>,
}

struct WakeFlag(AtomicBool);

impl Wake for WakeFlag {
    fn wake(self: Arc<Self>) {
        self.0.store(true, Ordering::Relaxed);
    }
}

impl AsyncBuild {
    /// Polls the build until it waits for a promise, or calls `on_done` once it's complete.
    fn resume(&self) {
        let Ok(mut future_slot) = self.future.try_borrow_mut() else { return };
        let Some(future) = future_slot.as_mut() else { return };
        let waker = self.woken.clone().into();
        let mut cx = Context::from_waker(&waker);
        let result = loop {
            self.woken.0.store(false, Ordering::Relaxed);
            if let Poll::Ready(result) = future.as_mut().poll(&mut cx) {
                break result;
            }
            if !self.woken.0.load(Ordering::Relaxed) {
                return;
            }
        };
        *future_slot = None;
        drop(future_slot);

        let Some(on_done) = self.on_done.take() else { return };
        let definitions = self.output.borrow_mut().take_from(&result);
        let env = &self.env;
        let args = definitions
            .into_unknown(env)
            .and_then(|definitions| Ok(vec![Null.into_unknown(env)?.raw(), definitions.raw()]))
            .or_else(|e| Ok::<_, napi::Error>(vec![env.create_string(&e.reason)?.raw()]));
        let called = args.and_then(|args| on_done.borrow_back(env)?.call(crate::DynArgs(args)));
        if called.is_err() {
            eprintln!("Node.js: the callback of an asynchronous build threw an exception");
        }
    }
}

fn io_error(e: napi::Error) -> std::io::Error {
    std::io::Error::other(e.reason)
}

/// Converts what the file loader returned, or what its promise resolved to, into the source of the file.
/// `undefined` and `null` load the file as if there was no file loader.
fn loaded_source(value: Unknown<'_>) -> Option<std::io::Result<String>> {
    if let Ok(ValueType::Undefined | ValueType::Null) = value.get_type() {
        return None;
    }
    let source = value.coerce_to_string().and_then(|s| String::from_unknown(s.to_unknown()));
    Some(
        source.map_err(|e| {
            std::io::Error::other(format!("the file loader didn't return a string: {e}"))
        }),
    )
}

#[napi]
impl JsComponentCompiler {
    /// Returns a new ComponentCompiler.
    #[napi(constructor)]
    pub fn new() -> Self {
        let mut compiler = Compiler::default();
        let include_paths = match std::env::var_os("SLINT_INCLUDE_PATH") {
            Some(paths) => {
                std::env::split_paths(&paths).filter(|path| !path.as_os_str().is_empty()).collect()
            }
            None => Vec::new(),
        };
        let library_paths = match std::env::var_os("SLINT_LIBRARY_PATH") {
            Some(paths) => std::env::split_paths(&paths)
                .filter_map(|entry| {
                    entry
                        .to_str()
                        .unwrap_or_default()
                        .split('=')
                        .collect_tuple()
                        .map(|(k, v)| (k.into(), v.into()))
                })
                .collect(),
            None => HashMap::new(),
        };

        // Environment variables rank below the project file, so they bypass the setters.
        let config = compiler.compiler_configuration(i_slint_core::InternalToken);
        config.include_paths = include_paths;
        config.library_paths = library_paths;

        Self { internal: compiler, file_loader: None, output: Default::default() }
    }

    #[napi(setter)]
    pub fn set_include_paths(&mut self, include_paths: Vec<String>) {
        self.internal.set_include_paths(include_paths.iter().map(PathBuf::from).collect());
    }

    #[napi(getter)]
    pub fn include_paths(&self) -> Vec<String> {
        self.internal
            .include_paths()
            .iter()
            .map(|p| p.to_str().unwrap_or_default().to_string())
            .collect()
    }

    #[napi(setter)]
    pub fn set_library_paths(&mut self, paths: HashMap<String, String>) {
        let mut library_paths = HashMap::new();
        for (key, path) in paths {
            library_paths.insert(key, PathBuf::from(path));
        }

        self.internal.set_library_paths(library_paths);
    }

    #[napi(getter)]
    pub fn library_paths(&self) -> HashMap<String, String> {
        let mut library_paths = HashMap::new();

        for (key, path) in self.internal.library_paths() {
            library_paths.insert(key.clone(), path.to_str().unwrap_or_default().to_string());
        }

        library_paths
    }

    #[napi(setter)]
    pub fn set_style(&mut self, style: String) {
        self.internal.set_style(style);
    }

    #[napi(getter)]
    pub fn style(&self) -> Option<String> {
        self.internal.style().cloned()
    }

    #[napi(getter)]
    pub fn diagnostics(&self) -> Vec<JsDiagnostic> {
        self.output.borrow().diagnostics.iter().map(|d| JsDiagnostic::from(d.clone())).collect()
    }

    #[napi(getter)]
    pub fn structs<'a>(&self, env: &'a Env) -> HashMap<String, Unknown<'a>> {
        fn convert_type<'a>(env: &'a Env, ty: &Type) -> Option<(String, Unknown<'a>)> {
            match ty {
                Type::Struct(s) if s.node().is_some() => {
                    let name = s.name.slint_name().unwrap();
                    let struct_instance = crate::to_js_unknown(
                        env,
                        &Value::Struct(slint_interpreter::Struct::from_iter(s.fields.keys().map(
                            |name| {
                                (
                                    name.to_string(),
                                    slint_interpreter::default_value_for_struct_field(s, name),
                                )
                            },
                        ))),
                    );

                    Some((name.to_string(), struct_instance.ok()?))
                }
                _ => None,
            }
        }

        self.output
            .borrow()
            .structs_and_enums
            .iter()
            .filter_map(|ty| convert_type(env, ty))
            .collect::<HashMap<String, Unknown<'a>>>()
    }

    #[napi(getter)]
    pub fn enums<'a>(&self, env: &'a Env) -> HashMap<String, Unknown<'a>> {
        fn convert_type<'a>(env: &'a Env, ty: &Type) -> Option<(String, Unknown<'a>)> {
            match ty {
                Type::Enumeration(en) => {
                    let mut o = Object::new(env).ok()?;

                    for value in en.values.iter() {
                        // The property name has the dashes replaced, the value is the
                        // spelling the runtime accepts.
                        let str_val = env.create_string(value).ok()?;
                        o.set_named_property(&value.replace_smolstr("-", "_"), str_val).ok()?;
                    }
                    Some((en.name.to_string(), o.into_unknown(env).ok()?))
                }
                _ => None,
            }
        }

        self.output
            .borrow()
            .structs_and_enums
            .iter()
            .filter_map(|ty| convert_type(env, ty))
            .collect::<HashMap<String, Unknown<'a>>>()
    }

    #[napi(setter)]
    pub fn set_file_loader(
        &mut self,
        env: &Env,
        #[napi(ts_arg_type = "(path: string) => string | undefined | Promise<string | undefined>")]
        callback: crate::DynFunction<'_>,
    ) -> napi::Result<()> {
        let loader = FileLoader { env: *env, callback: Rc::new(callback.create_ref()?) };
        self.internal.set_file_loader({
            let loader = loader.clone();
            move |path| Box::pin(std::future::ready(loader.load_sync(path)))
        });
        self.file_loader = Some(loader);
        Ok(())
    }

    /// Compile a .slint file into a ComponentDefinition
    ///
    /// Returns the compiled `ComponentDefinition` if there were no errors.
    #[napi]
    pub fn build_from_path(&self, path: String) -> HashMap<String, JsComponentDefinition> {
        let r = spin_on::spin_on(self.internal.build_from_path(PathBuf::from(path)));
        self.output.borrow_mut().take_from(&r)
    }

    /// Compile some .slint code into a ComponentDefinition
    #[napi]
    pub fn build_from_source(
        &self,
        source_code: String,
        path: String,
    ) -> HashMap<String, JsComponentDefinition> {
        let r = spin_on::spin_on(self.internal.build_from_source(source_code, PathBuf::from(path)));
        self.output.borrow_mut().take_from(&r)
    }

    /// Like `build_from_path`, but waits for the promises the file loader returns,
    /// and calls `on_done` with an error or the compiled definitions.
    #[napi]
    pub fn build_from_path_async(
        &self,
        env: &Env,
        path: String,
        #[napi(
            ts_arg_type = "(error: string | null, definitions?: Record<string, ComponentDefinition>) => void"
        )]
        on_done: crate::DynFunction<'_>,
    ) -> napi::Result<()> {
        self.start_async_build(env, on_done, |compiler| async move {
            compiler.build_from_path(PathBuf::from(path)).await
        })
    }

    /// Like `build_from_source`, but waits for the promises the file loader returns,
    /// and calls `on_done` with an error or the compiled definitions.
    #[napi]
    pub fn build_from_source_async(
        &self,
        env: &Env,
        source_code: String,
        path: String,
        #[napi(
            ts_arg_type = "(error: string | null, definitions?: Record<string, ComponentDefinition>) => void"
        )]
        on_done: crate::DynFunction<'_>,
    ) -> napi::Result<()> {
        self.start_async_build(env, on_done, |compiler| async move {
            compiler.build_from_source(source_code, PathBuf::from(path)).await
        })
    }

    fn start_async_build<F: Future<Output = CompilationResult> + 'static>(
        &self,
        env: &Env,
        on_done: crate::DynFunction<'_>,
        build: impl FnOnce(Compiler) -> F,
    ) -> napi::Result<()> {
        let on_done = on_done.create_ref()?;
        let mut compiler = self.internal.clone();
        let build = Rc::new_cyclic(|this: &Weak<AsyncBuild>| {
            if let Some(loader) = self.file_loader.clone() {
                let this = this.clone();
                compiler.set_file_loader(move |path| loader.load_async(&this, path));
            }
            AsyncBuild {
                env: *env,
                future: RefCell::new(Some(Box::pin(build(compiler)))),
                woken: Arc::new(WakeFlag(AtomicBool::new(false))),
                on_done: Some(on_done).into(),
                output: self.output.clone(),
            }
        });
        build.resume();
        Ok(())
    }
}
