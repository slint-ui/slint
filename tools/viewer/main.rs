// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]

use clap::Parser;
use i_slint_compiler::ComponentsToGenerate;
use i_slint_core::model::{Model, ModelRc};
use i_slint_core::SharedVector;
use itertools::Itertools;
use slint_interpreter::{
    ComponentDefinition, ComponentHandle, ComponentInstance, SharedString, Value,
};
use std::collections::HashMap;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Include path for other .slint files or images
    #[arg(short = 'I', value_name = "include path", number_of_values = 1, action)]
    include_paths: Vec<std::path::PathBuf>,

    /// Specify Library location of the '@library' in the form 'library=/path/to/library'
    #[arg(short = 'L', value_name = "library=path", number_of_values = 1, action)]
    library_paths: Vec<String>,

    /// The .slint file to load ('-' for stdin)
    #[arg(name = "path", action)]
    path: std::path::PathBuf,

    /// The style name ('native' or 'fluent')
    #[arg(long, value_name = "style name", action)]
    style: Option<String>,

    /// The name of the component to view. If unset, the last exported component of the file is used.
    /// If the component name is not in the .slint file , nothing will be shown
    #[arg(long, value_name = "component name", action)]
    component: Option<String>,

    /// The rendering backend
    #[arg(long, value_name = "backend", action)]
    backend: Option<String>,

    /// Automatically watch the file system, and reload when it changes
    #[arg(long, action)]
    auto_reload: bool,

    /// Load properties from a json file ('-' for stdin)
    #[arg(long, value_name = "json file", action)]
    load_data: Option<std::path::PathBuf>,

    /// Store properties values in a json file at exit ('-' for stdout)
    #[arg(long, value_name = "json file", action)]
    save_data: Option<std::path::PathBuf>,

    /// Specify callbacks handler.
    /// The first argument is the callback name, and the second argument is a string that is going
    /// to be passed to the shell to be executed. Occurrences of `$1` will be replaced by the first argument,
    /// and so on.
    #[arg(long, value_names(&["callback", "handler"]), number_of_values = 2, action)]
    on: Vec<String>,

    #[cfg(feature = "gettext")]
    /// Translation domain
    #[arg(long = "translation-domain", action)]
    translation_domain: Option<String>,

    #[cfg(feature = "gettext")]
    /// Translation directory where the translation files are searched for
    #[arg(long = "translation-dir", action)]
    translation_dir: Option<std::path::PathBuf>,
}

thread_local! {static CURRENT_INSTANCE: std::cell::RefCell<Option<ComponentInstance>> = Default::default();}
static EXIT_CODE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

fn main() -> Result<()> {
    env_logger::init();
    let args = Cli::parse();

    if args.auto_reload && args.save_data.is_some() {
        eprintln!("Cannot pass both --auto-reload and --save-data");
        std::process::exit(-1);
    }

    if let Some(backend) = &args.backend {
        std::env::set_var("SLINT_BACKEND", backend);
    }

    #[cfg(feature = "gettext")]
    if let Some(dirname) = args.translation_dir.clone() {
        i_slint_core::translations::gettext_bindtextdomain(
            args.translation_domain.as_ref().map(String::as_str).unwrap_or_default(),
            dirname,
        )?;
    };

    let fswatcher = if args.auto_reload { Some(start_fswatch_thread(args.clone())?) } else { None };
    let compiler = init_compiler(&args, fswatcher);
    let r = spin_on::spin_on(compiler.build_from_path(&args.path));
    slint_interpreter::print_diagnostics(&r.diagnostics().collect::<Vec<_>>());
    if r.has_error() {
        std::process::exit(-1);
    }
    let Some(c) = r.component_names().next().and_then(|n| r.component(n)) else {
        match args.component {
            Some(name) => {
                eprintln!("Component '{name}' not found in file '{}'", args.path.display());
            }
            None => {
                eprintln!("No component found in file '{}'", args.path.display());
            }
        }
        std::process::exit(-1);
    };

    let component = c.create().unwrap();
    init_dialog(&component);

    if let Some(data_path) = args.load_data {
        load_data(&c, &component, &data_path)?;
    }
    install_callbacks(&component, &args.on);

    if args.auto_reload {
        CURRENT_INSTANCE.with(|current| current.replace(Some(component.clone_strong())));
    }

    component.run().unwrap();

    if let Some(data_path) = args.save_data {
        let mut obj = serde_json::Map::new();
        for (name, _) in c.properties() {
            fn to_json(val: slint_interpreter::Value) -> Option<serde_json::Value> {
                match val {
                    slint_interpreter::Value::Number(x) => Some(x.into()),
                    slint_interpreter::Value::String(x) => Some(x.as_str().into()),
                    slint_interpreter::Value::Bool(x) => Some(x.into()),
                    slint_interpreter::Value::Model(model) => {
                        let mut res = Vec::with_capacity(model.row_count());
                        for i in 0..model.row_count() {
                            res.push(to_json(model.row_data(i).unwrap())?);
                        }
                        Some(serde_json::Value::Array(res))
                    }
                    slint_interpreter::Value::Struct(st) => {
                        let mut obj = serde_json::Map::new();
                        for (k, v) in st.iter() {
                            obj.insert(k.into(), to_json(v.clone())?);
                        }
                        Some(obj.into())
                    }
                    slint_interpreter::Value::EnumerationValue(_class, value) => {
                        Some(value.as_str().into())
                    }
                    _ => None,
                }
            }
            if let Some(v) = to_json(component.get_property(&name).unwrap()) {
                obj.insert(name, v);
            }
        }
        if data_path == std::path::Path::new("-") {
            serde_json::to_writer_pretty(std::io::stdout(), &obj)?;
        } else {
            serde_json::to_writer_pretty(BufWriter::new(std::fs::File::create(data_path)?), &obj)?;
        }
    }

    std::process::exit(EXIT_CODE.load(std::sync::atomic::Ordering::Relaxed))
}

fn init_compiler(
    args: &Cli,
    fswatcher: Option<Arc<Mutex<notify::RecommendedWatcher>>>,
) -> slint_interpreter::Compiler {
    let mut compiler = slint_interpreter::Compiler::new();
    #[cfg(feature = "gettext")]
    if let Some(domain) = args.translation_domain.clone() {
        compiler.set_translation_domain(domain);
    }
    compiler.set_include_paths(args.include_paths.clone());
    compiler.set_library_paths(
        args.library_paths
            .iter()
            .filter_map(|entry| entry.split('=').collect_tuple().map(|(k, v)| (k.into(), v.into())))
            .collect(),
    );
    if let Some(style) = &args.style {
        compiler.set_style(style.clone());
    }
    if let Some(watcher) = fswatcher {
        watch_with_retry(&args.path, &watcher);
        if let Some(data_path) = &args.load_data {
            watch_with_retry(data_path, &watcher);
        }
        compiler.set_file_loader(move |path| {
            watch_with_retry(&path.into(), &watcher);
            Box::pin(async { None })
        })
    }

    compiler.compiler_configuration(i_slint_core::InternalToken).components_to_generate =
        match &args.component {
            Some(component) => ComponentsToGenerate::ComponentWithName(component.clone()),
            None => ComponentsToGenerate::LastComponent,
        };

    compiler
}

fn watch_with_retry(path: &PathBuf, watcher: &Arc<Mutex<notify::RecommendedWatcher>>) {
    notify::Watcher::watch(
        &mut *watcher.lock().unwrap(),
        path,
        notify::RecursiveMode::NonRecursive,
    )
    .unwrap_or_else(|err| match err.kind {
        notify::ErrorKind::PathNotFound | notify::ErrorKind::Generic(_) => {
            let path = path.clone();
            let watcher = watcher.clone();
            static RETRY_DURATION: u64 = 100;
            i_slint_core::timers::Timer::single_shot(
                std::time::Duration::from_millis(RETRY_DURATION),
                move || {
                    notify::Watcher::watch(
                        &mut *watcher.lock().unwrap(),
                        &path,
                        notify::RecursiveMode::NonRecursive,
                    )
                    .unwrap_or_else(|err| {
                        eprintln!(
                            "Warning: error while watching missing path {}: {:?}",
                            path.display(),
                            err
                        )
                    });
                },
            );
        }
        _ => eprintln!("Warning: error while watching {}: {:?}", path.display(), err),
    });
}

fn init_dialog(instance: &ComponentInstance) {
    for cb in instance.definition().callbacks() {
        let exit_code = match cb.as_str() {
            "ok-clicked" | "yes-clicked" | "close-clicked" => 0,
            "cancel-clicked" | "no-clicked" => 1,
            _ => continue,
        };
        // this is a dialog, so clicking the "x" should cancel
        EXIT_CODE.store(1, std::sync::atomic::Ordering::Relaxed);
        instance
            .set_callback(&cb, move |_| {
                EXIT_CODE.store(exit_code, std::sync::atomic::Ordering::Relaxed);
                i_slint_core::api::quit_event_loop().unwrap();
                Default::default()
            })
            .unwrap();
    }
}

static PENDING_EVENTS: AtomicU32 = AtomicU32::new(0);

fn start_fswatch_thread(args: Cli) -> Result<Arc<Mutex<notify::RecommendedWatcher>>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let w = Arc::new(Mutex::new(notify::recommended_watcher(tx)?));
    let w2 = w.clone();
    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            use notify::EventKind::*;
            if let Ok(event) = event {
                if (matches!(event.kind, Modify(_) | Remove(_) | Create(_)))
                    && PENDING_EVENTS.load(Ordering::SeqCst) == 0
                {
                    PENDING_EVENTS.fetch_add(1, Ordering::SeqCst);
                    let args = args.clone();
                    let w2 = w2.clone();
                    i_slint_core::api::invoke_from_event_loop(move || {
                        i_slint_core::future::spawn_local(reload(args, w2)).unwrap();
                    })
                    .unwrap();
                }
            }
        }
    });
    Ok(w)
}

async fn reload(args: Cli, fswatcher: Arc<Mutex<notify::RecommendedWatcher>>) {
    let compiler = init_compiler(&args, Some(fswatcher));
    let r = compiler.build_from_path(&args.path).await;
    slint_interpreter::print_diagnostics(&r.diagnostics().collect::<Vec<_>>());
    if let Some(c) = r.component_names().next().and_then(|n| r.component(n)) {
        CURRENT_INSTANCE.with(|current| {
            let mut current = current.borrow_mut();
            if let Some(handle) = current.take() {
                let window = handle.window();
                let new_handle = c.create_with_existing_window(window).unwrap();
                init_dialog(&new_handle);
                current.replace(new_handle);
            } else {
                let handle = c.create().unwrap();
                init_dialog(&handle);
                handle.show().unwrap();
                current.replace(handle);
            }
            if let Some(data_path) = args.load_data {
                let _ = load_data(&c, current.as_ref().unwrap(), &data_path);
            }
            eprintln!("Successful reload of {}", args.path.display());
        });
    } else if !r.has_error() {
        match &args.component {
            Some(name) => println!("Component {name} not found"),
            None => println!("No component found"),
        }
    }

    PENDING_EVENTS.fetch_sub(1, Ordering::SeqCst);
}

fn load_data(
    c: &ComponentDefinition,
    instance: &ComponentInstance,
    data_path: &std::path::Path,
) -> Result<()> {
    let json: serde_json::Value = if data_path == std::path::Path::new("-") {
        serde_json::from_reader(std::io::stdin())?
    } else {
        serde_json::from_reader(BufReader::new(std::fs::File::open(data_path)?))?
    };

    let types = c.properties_and_callbacks().collect::<HashMap<_, _>>();
    let obj = json.as_object().ok_or("The data is not a JSON object")?;
    for (name, v) in obj {
        fn from_json(
            t: &i_slint_compiler::langtype::Type,
            v: &serde_json::Value,
        ) -> slint_interpreter::Value {
            match v {
                serde_json::Value::Null => slint_interpreter::Value::Void,
                serde_json::Value::Bool(b) => (*b).into(),
                serde_json::Value::Number(n) => {
                    slint_interpreter::Value::Number(n.as_f64().unwrap_or(f64::NAN))
                }
                serde_json::Value::String(s) => match t {
                    i_slint_compiler::langtype::Type::Enumeration(e) => {
                        if e.values.contains(s) {
                            slint_interpreter::Value::EnumerationValue(
                                e.name.to_string(),
                                s.to_string(),
                            )
                        } else {
                            eprintln!("Warning: Unexpected value for enum '{}': {}", e.name, s);
                            slint_interpreter::Value::Void
                        }
                    }
                    i_slint_compiler::langtype::Type::String => {
                        SharedString::from(s.as_str()).into()
                    }
                    _ => slint_interpreter::Value::Void,
                },
                serde_json::Value::Array(array) => match t {
                    i_slint_compiler::langtype::Type::Array(it) => slint_interpreter::Value::Model(
                        ModelRc::new(i_slint_core::model::SharedVectorModel::from(
                            array.iter().map(|v| from_json(it, v)).collect::<SharedVector<Value>>(),
                        )),
                    ),
                    _ => slint_interpreter::Value::Void,
                },
                serde_json::Value::Object(obj) => match t {
                    i_slint_compiler::langtype::Type::Struct { fields, .. } => obj
                        .iter()
                        .filter_map(|(k, v)| match fields.get(k) {
                            Some(t) => Some((k.clone(), from_json(t, v))),
                            None => {
                                eprintln!("Warning: ignoring unknown property: {}", k);
                                None
                            }
                        })
                        .collect::<slint_interpreter::Struct>()
                        .into(),
                    _ => slint_interpreter::Value::Void,
                },
            }
        }

        match types.get(name) {
            Some(t) => {
                match instance.set_property(name, from_json(t, v)) {
                    Ok(()) => (),
                    Err(e) => {
                        eprintln!("Warning: cannot set property '{}' from data file: {:?}", name, e)
                    }
                };
            }
            None => eprintln!("Warning: ignoring unknown property: {}", name),
        }
    }
    Ok(())
}

fn install_callbacks(instance: &ComponentInstance, callbacks: &[String]) {
    assert!(callbacks.len() % 2 == 0);
    for chunk in callbacks.chunks(2) {
        if let [callback, cmd] = chunk {
            let cmd = cmd.clone();
            match instance.set_callback(callback, move |args| {
                match execute_cmd(&cmd, args) {
                    Ok(()) => (),
                    Err(e) => eprintln!("Error: {}", e),
                }
                Value::Void
            }) {
                Ok(()) => (),
                Err(e) => {
                    eprintln!("Warning: cannot set callback handler for '{}': {}", callback, e)
                }
            }
        }
    }
}

fn execute_cmd(cmd: &str, callback_args: &[Value]) -> Result<()> {
    let cmd_args = shlex::split(cmd).ok_or("Could not parse the command string")?;
    let program_name = cmd_args.first().ok_or("Missing program name")?;
    let mut command = std::process::Command::new(program_name);
    let callback_args = callback_args
        .iter()
        .map(|v| {
            Ok(match v {
                Value::Number(x) => x.to_string(),
                Value::String(x) => x.to_string(),
                Value::Bool(x) => x.to_string(),
                Value::Image(img) => {
                    img.path().map(|p| p.to_string_lossy()).unwrap_or_default().into()
                }
                _ => return Err(format!("Cannot convert argument to string: {:?}", v).into()),
            })
        })
        .collect::<Result<Vec<String>>>()?;
    for mut a in cmd_args.into_iter().skip(1) {
        for (idx, cb_a) in callback_args.iter().enumerate() {
            a = a.replace(&format!("${}", idx + 1), cb_a);
        }
        command.arg(a);
    }
    command.spawn()?;
    Ok(())
}
