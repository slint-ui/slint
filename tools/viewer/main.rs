// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]

use clap::Parser;
use i_slint_compiler::ComponentSelection;
use i_slint_core::timers::Timer;
use itertools::Itertools;
use slint_interpreter::{
    CompilationResult, ComponentDefinition, ComponentHandle, ComponentInstance, FileWatcher, Value,
    json::JsonExt,
};
use std::collections::HashMap;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

#[cfg(not(any(
    target_os = "openbsd",
    target_os = "windows",
    all(target_arch = "aarch64", target_os = "linux")
)))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(any(
    target_os = "openbsd",
    target_os = "windows",
    all(target_arch = "aarch64", target_os = "linux")
)))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

struct Error(Box<dyn std::error::Error>);
impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Use the Display impl of the error instead of the error
        write!(f, "{}", self.0)
    }
}

impl<T> From<T> for Error
where
    T: Into<Box<dyn std::error::Error>> + 'static,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

type Result<T> = std::result::Result<T, Error>;

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

    /// The style name. Defaults to 'fluent' if not specified
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

    #[cfg(feature = "gettext")]
    /// Disable the default to use the component name as translation context when none is specified in `@tr`
    #[arg(long = "no-default-translation-context")]
    no_default_translation_context: bool,
}

struct Viewer {
    instance: ComponentInstance,
    file_watcher: FileWatcher,
    args: Cli,
    // The reload timer, used to debounce multiple file change events into a single reload
    // if --auto-reload is enabled
    reload_timer: i_slint_core::timers::Timer,
}

thread_local! {static SLINT_VIEWER: std::cell::RefCell<Option<Viewer>> = Default::default();}
static EXIT_CODE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

fn main() -> Result<()> {
    env_logger::init();
    let args = Cli::parse();

    if args.auto_reload && args.save_data.is_some() {
        eprintln!("Cannot pass both --auto-reload and --save-data");
        std::process::exit(-1);
    }

    if let Some(backend) = &args.backend {
        slint_interpreter::BackendSelector::new().backend_name(backend.clone()).select()?;
    }

    #[cfg(feature = "gettext")]
    if let Some(dirname) = args.translation_dir.clone() {
        i_slint_core::translations::gettext_bindtextdomain(
            args.translation_domain.as_deref().unwrap_or_default(),
            dirname,
        )?;
    };

    let mut file_watcher = if args.auto_reload { Some(start_file_watcher()?) } else { None };
    if let Some(file_watcher) = file_watcher.as_mut() {
        file_watcher.update_watched_paths(initial_watched_paths(&args))?;
    }
    let result = load(&args, file_watcher.as_mut());
    if result.has_errors() {
        std::process::exit(-1);
    }
    let Some(c) = extract_component(&result, &args) else {
        // extract_component already prints an error message, so we just need to exit with an error code here
        std::process::exit(-1);
    };

    let component = c.create()?;
    init_dialog(&component);

    if let Some(data_path) = args.load_data.as_ref() {
        load_data(&c, &component, data_path)?;
    }
    install_callbacks(&component, &args.on);

    if let Some(file_watcher) = file_watcher {
        let args = args.clone();
        let component = component.clone_strong();
        SLINT_VIEWER.with(move |viewer| {
            viewer.replace(Some(Viewer {
                instance: component,
                file_watcher,
                args,
                reload_timer: Timer::default(),
            }))
        });
    }

    // Show the preview and running the event loop. Closing the window will make it continue
    component.run()?;

    if let Some(data_path) = args.save_data {
        let mut obj = serde_json::Map::new();
        for (name, _) in c.properties() {
            match component.get_property(&name).unwrap().to_json() {
                Ok(v) => {
                    obj.insert(name, v);
                }
                Err(e) => {
                    eprintln!("Failed to turn property {name} into JSON: {e}");
                }
            }
        }
        for global_name in c.globals() {
            let mut g_obj = serde_json::Map::new();
            for (name, _) in c.global_properties(&global_name).unwrap() {
                match component.get_global_property(&global_name, &name).unwrap().to_json() {
                    Ok(v) => {
                        g_obj.insert(name, v);
                    }
                    Err(e) => {
                        eprintln!("Failed to turn property {global_name}.{name} into JSON: {e}");
                    }
                }
            }
            if !g_obj.is_empty() {
                obj.insert(global_name, serde_json::Value::Object(g_obj));
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

fn init_compiler(args: &Cli) -> slint_interpreter::Compiler {
    let mut compiler = slint_interpreter::Compiler::new();
    #[cfg(feature = "gettext")]
    if let Some(domain) = args.translation_domain.clone() {
        compiler.set_translation_domain(domain);
    }
    #[cfg(feature = "gettext")]
    if args.no_default_translation_context {
        compiler
            .set_default_translation_context(slint_interpreter::DefaultTranslationContext::None);
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

    compiler.compiler_configuration(i_slint_core::InternalToken).components_to_generate =
        match &args.component {
            Some(component) => ComponentSelection::Named(component.clone()),
            None => ComponentSelection::LastExported,
        };

    compiler
}

/// Init dialog if `instance` is a Dialog
/// Initializes the callbacks for `ok`, `yes`, `close`, `cancel` or `no` to quit the event loop
/// When one of those callbacks gets triggered the preview gets closed as well
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

fn watchable_path(path: &Path) -> Option<PathBuf> {
    // Filter out `-` for stdin
    (path != Path::new("-")).then(|| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|current_dir| current_dir.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    })
}

fn initial_watched_paths(args: &Cli) -> Vec<PathBuf> {
    std::iter::once(args.path.clone())
        .chain(args.load_data.iter().cloned())
        .filter_map(|path| watchable_path(&path))
        .collect()
}

fn watched_paths(args: &Cli, result: &CompilationResult) -> Vec<PathBuf> {
    result
        .watch_paths(i_slint_core::InternalToken)
        .iter()
        .cloned()
        .chain(args.load_data.iter().cloned())
        .filter_map(|path| watchable_path(&path))
        .collect()
}

// When a lot of files changes (e.g. git checkout) we might get multiple events in a short time,
// so we use a timer to debounce them into a single reload.
const RELOAD_DEBOUNCE_DELAY: std::time::Duration = std::time::Duration::from_millis(100);

fn start_file_watcher() -> Result<FileWatcher> {
    let watcher = FileWatcher::start(
        move |_event| {
            // We need to get back onto the main thread, which we do with invoke_from_event_loop
            // for our debounce-timer to work.
            // Then we debounce multiple file-watcher changes, with the timer.
            slint_interpreter::invoke_from_event_loop(|| {
                SLINT_VIEWER.with_borrow_mut(|viewer| {
                    let viewer = viewer
                        .as_mut()
                        .expect("Viewer must have been initialized with reload support");
                    viewer.reload_timer.start(
                        i_slint_core::timers::TimerMode::SingleShot,
                        RELOAD_DEBOUNCE_DELAY,
                        reload,
                    )
                })
            })
            .map_err(|err| eprintln!("Warning: Failed to schedule reload on file change: {err}"))
            .ok();
        },
        move |err| eprintln!("Warning: file watcher error: {err}"),
    )?;

    Ok(watcher)
}

fn load(args: &Cli, file_watcher: Option<&mut FileWatcher>) -> CompilationResult {
    let compiler = init_compiler(args);

    // In theory, the compiler can be async, but in practice it is not because we have not
    // configured an open import callback.
    // That means we can just block here.
    let result = spin_on::spin_on(compiler.build_from_path(args.path.clone()));

    result.print_diagnostics();
    if let Some(file_watcher) = file_watcher {
        file_watcher.update_watched_paths(watched_paths(args, &result)).unwrap_or_else(|err| {
            eprintln!("Warning: Failed to update file watcher paths: {err}");
        });
    }
    result
}

/// Extract the component to show from the compilation result, and print an error if it cannot be found
fn extract_component(result: &CompilationResult, args: &Cli) -> Option<ComponentDefinition> {
    // If --component is used, result.compents contains only one element (filtered out in init_compiler())
    // If no component name is specified, the last defined component is shown
    let component = result.components().next();
    if component.is_none() {
        match &args.component {
            Some(name) => {
                eprintln!("Component '{name}' not found in file '{}'", args.path.display());
            }
            None => {
                eprintln!("No component found in file '{}'", args.path.display());
            }
        }
    }
    component
}

fn reload() {
    SLINT_VIEWER.with_borrow_mut(|viewer| {
        let Some(viewer) =
            viewer.as_mut() else {
            eprintln!("Warning: File changes detected, but the viewer is not initialized to support reloading");
            return;
        };
        eprintln!("File changes detected, reloading {}", viewer.args.path.display());

        let result = load(&viewer.args, Some(&mut viewer.file_watcher));

        if result.has_errors() {
            return;
        }

        let Some(component) = extract_component(&result, &viewer.args) else {
            return;
        };

        let window = viewer.instance.window();
        let new_instance = component.create_with_existing_window(window).unwrap();
        init_dialog(&new_instance);
        install_callbacks(&new_instance, &viewer.args.on);
        if let Some(data_path) = &viewer.args.load_data {
            let _ = load_data(&component, &new_instance, data_path);
        }
        viewer.instance = new_instance;

        eprintln!("Successful reload of {}", viewer.args.path.display());
    });
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
    let globals = c.globals();
    let globals_types = globals
        .filter_map(|g| {
            c.global_properties_and_callbacks(&g).map(|iter| (g, iter.collect::<HashMap<_, _>>()))
        })
        .collect::<HashMap<_, _>>();
    let obj = json.as_object().ok_or("The data is not a JSON object")?;
    for (name, v) in obj {
        match types.get(name.as_str()) {
            Some((t, _)) => match slint_interpreter::Value::from_json(t, v) {
                Ok(v) => match instance.set_property(name, v) {
                    Ok(()) => (),
                    Err(e) => {
                        eprintln!("Warning: cannot set property '{name}' from data file: {e}")
                    }
                },
                Err(e) => eprintln!("Warning: cannot set property '{name}' from data file: {e}"),
            },
            None => match name.split_once('.') {
                Some((global_name, prop_name)) => {
                    match globals_types.get(global_name).and_then(|m| m.get(prop_name)) {
                        Some((t, _)) => match slint_interpreter::Value::from_json(t, v) {
                            Ok(v) => {
                                match instance.set_global_property(global_name, prop_name, v) {
                                    Ok(()) => (),
                                    Err(e) => {
                                        eprintln!(
                                            "Warning: cannot set property '{name}' from data file: {e}"
                                        )
                                    }
                                }
                            }
                            Err(e) => eprintln!(
                                "Warning: cannot set property '{name}' from data file: {e}"
                            ),
                        },
                        None => eprintln!("Warning: ignoring unknown property: {name}"),
                    }
                }
                None => match globals_types.get(name.as_str()) {
                    Some(global_types) => match v {
                        serde_json::Value::Object(map) => {
                            for (inner_name, v) in map {
                                match global_types.get(inner_name.as_str()) {
                                    Some((t, _)) => match slint_interpreter::Value::from_json(t, v)
                                    {
                                        Ok(v) => match instance
                                            .set_global_property(name, inner_name, v)
                                        {
                                            Ok(()) => (),
                                            Err(e) => {
                                                eprintln!(
                                                    "Warning: cannot set property '{name}.{inner_name}' from data file: {e}"
                                                )
                                            }
                                        },
                                        Err(e) => eprintln!(
                                            "Warning: cannot set property '{name}.{inner_name}' from data file: {e}"
                                        ),
                                    },
                                    None => eprintln!(
                                        "Warning: ignoring unknown property: {name}.{inner_name}"
                                    ),
                                }
                            }
                        }
                        _ => {
                            eprintln!(
                                "Warning: cannot set global '{name}' properties: The data is not a JSON object"
                            )
                        }
                    },
                    None => eprintln!("Warning: ignoring unknown property: {name}"),
                },
            },
        }
    }
    Ok(())
}

fn install_callbacks(instance: &ComponentInstance, callbacks: &[String]) {
    assert!(callbacks.len().is_multiple_of(2));
    for chunk in callbacks.chunks(2) {
        if let [callback, cmd] = chunk {
            let cmd = cmd.clone();
            match instance.set_callback(callback, move |args| {
                match execute_cmd(&cmd, args) {
                    Ok(()) => (),
                    Err(e) => eprintln!("Error: {e:?}"),
                }
                Value::Void
            }) {
                Ok(()) => (),
                Err(e) => {
                    eprintln!("Warning: cannot set callback handler for '{callback}': {e}")
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
                Value::Struct(st) => {
                    let mut obj = serde_json::Map::new();
                    for (k, v) in st.iter() {
                        match v.to_json() {
                            Ok(v) => {
                                obj.insert(k.into(), v);
                            }
                            Err(e) => {
                                eprintln!("Failed to convert field {k} to JSON: {e}");
                            }
                        }
                    }
                    serde_json::to_string_pretty(&obj)?
                }
                _ => return Err(format!("Cannot convert argument to string: {v:?}").into()),
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
