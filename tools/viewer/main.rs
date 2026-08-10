// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]

mod debug;
mod screenshot;

#[cfg(feature = "remote")]
mod remote;

use clap::Parser;
use i_slint_compiler::ComponentSelection;
use itertools::Itertools;
use slint_interpreter::{
    CompilationResult, ComponentDefinition, ComponentHandle, ComponentInstance, Value,
    json::JsonExt,
};
use std::collections::HashMap;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

#[cfg(not(any(
    target_os = "openbsd",
    target_os = "windows",
    target_os = "ios",
    all(target_arch = "aarch64", target_os = "linux")
)))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(any(
    target_os = "openbsd",
    target_os = "windows",
    target_os = "ios",
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
    #[arg(name = "path", action, required_unless_present = "remote")]
    path: Option<std::path::PathBuf>,

    /// Start in remote viewer mode: listen for WebSocket connections from the LSP
    #[arg(long)]
    remote: bool,

    /// Address to listen on in remote mode (default: auto-assigned port on all interfaces)
    #[arg(long, value_name = "address")]
    remote_address: Option<std::net::SocketAddr>,

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

    /// Render the component to an image and exit.
    /// The format follows the extension (e.g. `.png`, `.jpg`);
    /// use `-` to write a PNG to standard output.
    #[arg(long, value_name = "image file", action)]
    screenshot: Option<std::path::PathBuf>,

    /// Size of the `--screenshot` window as `WIDTHxHEIGHT` in logical pixels
    /// (e.g. `360x800`). Defaults to the component's preferred size.
    #[arg(long, value_name = "WxH", action, requires = "screenshot")]
    size: Option<String>,

    /// Compile, print any diagnostics, and exit without opening a window.
    /// Exit status is 1 on errors, 0 otherwise (warnings still print).
    #[arg(
        long,
        action,
        conflicts_with_all = ["auto_reload", "screenshot", "save_data", "load_data", "on", "remote"],
    )]
    check: bool,

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

impl Cli {
    fn path(&self) -> &std::path::Path {
        self.path.as_deref().expect("path is required when not in remote mode")
    }
}

static EXIT_CODE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .log_internal_errors(false)
        .without_time()
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::WARN.into())
                .from_env_lossy(),
        )
        .init();

    tracing_log::LogTracer::init().ok();

    // On iOS the binary is launched as an app without command line arguments, so always
    // start in remote viewer mode.
    #[cfg(all(target_os = "ios", feature = "remote"))]
    let args = Cli::parse_from(["slint-viewer", "--remote"]);
    #[cfg(not(all(target_os = "ios", feature = "remote")))]
    let args = Cli::parse();

    if args.screenshot.is_some() {
        if args.auto_reload {
            eprintln!("Cannot pass both --auto-reload and --screenshot");
            std::process::exit(2);
        }
        if args.save_data.is_some() {
            eprintln!("Cannot pass both --save-data and --screenshot");
            std::process::exit(2);
        }
        #[cfg(feature = "remote")]
        if args.remote {
            eprintln!("Cannot pass both --remote and --screenshot");
            std::process::exit(2);
        }
    }

    if args.remote {
        #[cfg(feature = "remote")]
        {
            remote::run(args.remote_address, true)?;
            return Ok(());
        }
        #[cfg(not(feature = "remote"))]
        {
            eprintln!(
                "Remote mode is not supported in this build, recompile Slint Viewer with the \"remote\" feature enabled."
            );
            return Err(Error("Remote mode not enabled".into()));
        }
    }

    if args.auto_reload && args.save_data.is_some() {
        eprintln!("Cannot pass both --auto-reload and --save-data");
        std::process::exit(2);
    }

    #[cfg(feature = "gettext")]
    if let Some(dirname) = args.translation_dir.clone() {
        i_slint_core::translations::gettext_bindtextdomain(
            args.translation_domain.as_deref().unwrap_or_default(),
            dirname,
        )?;
    };

    if args.screenshot.is_some() {
        if args.backend.is_some() {
            select_backend(args.backend.as_deref())?;
        }
        return screenshot::take_screenshot(&args);
    }

    let compiler = init_compiler(&args);

    if args.check {
        let result = poll_ready(compiler.build_from_path(args.path()));
        result.print_diagnostics();
        std::process::exit(if result.has_errors() { 1 } else { 0 });
    }

    if args.auto_reload {
        select_backend(args.backend.as_deref())?;
        install_log_message_handler()?;

        let live = i_slint_live_preview::live_component::LiveReloadingComponent::new(
            compiler,
            args.path().to_path_buf(),
            args.component.clone(),
        )?;

        reject_non_window_component(&live.borrow().instance().definition());

        setup_instance(live.borrow().instance(), &args.on, args.load_data.as_deref())?;

        {
            let on = args.on.clone();
            let load_data_path = args.load_data.clone();
            live.borrow_mut().set_post_reload_hook(move |instance| {
                let _ = setup_instance(instance, &on, load_data_path.as_deref());
            });
        }

        if let Some(data_path) = &args.load_data
            && let Some(p) = watchable_path(data_path)
        {
            live.borrow_mut().set_extra_watch_paths(vec![p]);
        }

        let instance = live.borrow().instance().clone_strong();
        instance.run()?;
    } else {
        let result = poll_ready(compiler.build_from_path(args.path()));
        result.print_diagnostics();
        if result.has_errors() {
            std::process::exit(-1);
        }
        let Some(c) = extract_component(&result, &args) else {
            std::process::exit(-1);
        };
        reject_non_window_component(&c);

        select_backend(args.backend.as_deref())?;
        install_log_message_handler()?;

        let component = c.create()?;
        setup_instance(&component, &args.on, args.load_data.as_deref())?;

        component.run()?;

        if let Some(data_path) = args.save_data {
            save_data(&component, &data_path)?;
        }
    }

    std::process::exit(EXIT_CODE.load(std::sync::atomic::Ordering::Relaxed))
}

fn select_backend(backend: Option<&str>) -> Result<()> {
    let mut backend_selector = slint_interpreter::BackendSelector::new();
    if let Some(backend) = backend {
        backend_selector = backend_selector.backend_name(backend.to_owned());
    }
    backend_selector.select()?;
    Ok(())
}

fn install_log_message_handler() -> Result<()> {
    let _ = i_slint_backend_selector::with_global_context(|ctx| {
        ctx.set_log_message_handler(Some(Box::new(move |message| {
            debug::log_message_handler(&message);
        })))
    })?;
    Ok(())
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

fn setup_instance(
    instance: &ComponentInstance,
    callbacks: &[String],
    load_data_path: Option<&Path>,
) -> Result<()> {
    init_dialog(instance);
    if let Some(data_path) = load_data_path {
        load_data(instance, data_path)?;
    }
    install_callbacks(instance, callbacks);
    Ok(())
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
    // Filter out `-` for stdin; the file watcher resolves relative paths.
    (path != Path::new("-")).then(|| path.to_path_buf())
}

/// Exit with an error if the component has no window to display (e.g. a `SystemTrayIcon` root).
fn reject_non_window_component(definition: &ComponentDefinition) {
    if !definition.is_window() {
        eprintln!(
            "Component '{}' is a SystemTrayIcon, which the viewer cannot display.",
            definition.name()
        );
        std::process::exit(-1);
    }
}

/// Extract the component to show from the compilation result, and print an error if it cannot be found
fn extract_component(result: &CompilationResult, args: &Cli) -> Option<ComponentDefinition> {
    // If --component is used, result.components contains only one element (filtered out in init_compiler())
    // If no component name is specified, the last defined component is shown
    let component = result.components().next();
    if component.is_none() {
        match &args.component {
            Some(name) => {
                eprintln!("Component '{name}' not found in file '{}'", args.path().display());
            }
            None => {
                eprintln!("No component found in file '{}'", args.path().display());
            }
        }
    }
    component
}

fn load_data(instance: &ComponentInstance, data_path: &std::path::Path) -> Result<()> {
    let c = instance.definition();
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

fn save_data(instance: &ComponentInstance, data_path: &std::path::Path) -> Result<()> {
    let c = instance.definition();
    let mut obj = serde_json::Map::new();
    for (name, _) in c.properties() {
        match instance.get_property(&name).unwrap().to_json() {
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
            match instance.get_global_property(&global_name, &name).unwrap().to_json() {
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

/// Poll a future that is expected to resolve immediately (e.g. the interpreter's
/// `build_from_path` when no async file loader is installed).
fn poll_ready<F: std::future::Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::future::Future::poll(future.as_mut(), &mut cx) {
        std::task::Poll::Ready(result) => result,
        std::task::Poll::Pending => unreachable!("Compiler returned Pending"),
    }
}
