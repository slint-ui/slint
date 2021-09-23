/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use sixtyfps_interpreter::{ComponentInstance, SharedString};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Wake;
use std::time::Duration;
use structopt::StructOpt;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(StructOpt, Clone)]
struct Cli {
    #[structopt(short = "I", name = "include path for other .60 files", number_of_values = 1)]
    include_paths: Vec<std::path::PathBuf>,

    /// The .60 file to load ('-' for stdin)
    #[structopt(name = "path to .60 file", parse(from_os_str))]
    path: std::path::PathBuf,

    /// The style name ('native', or 'ulgy')
    #[structopt(long, name = "style name", default_value)]
    style: String,

    /// The rendering backend
    #[structopt(long, name = "backend", default_value)]
    backend: String,

    /// Automatically watch the file system, and reload when it changes
    #[structopt(long)]
    auto_reload: bool,

    /// Load properties from a json file ('-' for stdin)
    #[structopt(long, name = "load data file")]
    load_data: Option<std::path::PathBuf>,

    /// Store properties values in a json file at exit ('-' for stdout)
    #[structopt(long, name = "save data file")]
    save_data: Option<std::path::PathBuf>,
}

thread_local! {static CURRENT_INSTANCE: std::cell::RefCell<Option<ComponentInstance>> = Default::default();}

fn main() -> Result<()> {
    let args = Cli::from_args();

    if args.auto_reload && (args.save_data.is_some()) {
        eprintln!("Cannot pass both --auto-reload and --save-data");
        std::process::exit(-1);
    }

    if !args.backend.is_empty() {
        std::env::set_var("SIXTYFPS_BACKEND", &args.backend);
    }

    let fswatcher = if args.auto_reload { Some(start_fswatch_thread(args.clone())?) } else { None };
    let mut compiler = init_compiler(&args, fswatcher);

    let c = spin_on::spin_on(compiler.build_from_path(args.path));
    sixtyfps_interpreter::print_diagnostics(compiler.diagnostics());

    let c = match c {
        Some(c) => c,
        None => std::process::exit(-1),
    };

    let component = c.create();

    if let Some(data_path) = args.load_data {
        load_data(&component, &data_path)?;
    }

    if args.auto_reload {
        CURRENT_INSTANCE.with(|current| current.replace(Some(component.clone_strong())));
    }

    component.run();

    if let Some(data_path) = args.save_data {
        let mut obj = serde_json::Map::new();
        for (name, _) in c.properties() {
            fn to_json(val: sixtyfps_interpreter::Value) -> Option<serde_json::Value> {
                match val {
                    sixtyfps_interpreter::Value::Number(x) => Some(x.into()),
                    sixtyfps_interpreter::Value::String(x) => Some(x.as_str().into()),
                    sixtyfps_interpreter::Value::Bool(x) => Some(x.into()),
                    sixtyfps_interpreter::Value::Array(arr) => {
                        let mut res = Vec::with_capacity(arr.len());
                        for x in arr {
                            res.push(to_json(x)?);
                        }
                        Some(serde_json::Value::Array(res))
                    }
                    sixtyfps_interpreter::Value::Struct(st) => {
                        let mut obj = serde_json::Map::new();
                        for (k, v) in st.iter() {
                            obj.insert(k.into(), to_json(v.clone())?);
                        }
                        Some(obj.into())
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
            serde_json::to_writer_pretty(std::fs::File::create(data_path)?, &obj)?;
        }
    }

    Ok(())
}

fn init_compiler(
    args: &Cli,
    fswatcher: Option<Arc<Mutex<notify::RecommendedWatcher>>>,
) -> sixtyfps_interpreter::ComponentCompiler {
    let mut compiler = sixtyfps_interpreter::ComponentCompiler::default();
    compiler.set_include_paths(args.include_paths.clone());
    if !args.style.is_empty() {
        compiler.set_style(args.style.clone());
    }
    if let Some(watcher) = fswatcher {
        notify::Watcher::watch(
            &mut *watcher.lock().unwrap(),
            &args.path,
            notify::RecursiveMode::NonRecursive,
        )
        .unwrap_or_else(|err| {
            eprintln!("Warning: error while watching {}: {:?}", args.path.display(), err)
        });
        if let Some(data_path) = &args.load_data {
            notify::Watcher::watch(
                &mut *watcher.lock().unwrap(),
                data_path,
                notify::RecursiveMode::NonRecursive,
            )
            .unwrap_or_else(|err| {
                eprintln!("Warning: error while watching {}: {:?}", data_path.display(), err)
            });
        }
        compiler.set_file_loader(move |path| {
            notify::Watcher::watch(
                &mut *watcher.lock().unwrap(),
                &path,
                notify::RecursiveMode::NonRecursive,
            )
            .unwrap_or_else(|err| {
                eprintln!("Warning: error while watching {}: {:?}", path.display(), err)
            });
            Box::pin(async { None })
        })
    }
    compiler
}

static PENDING_EVENTS: AtomicU32 = AtomicU32::new(0);

fn start_fswatch_thread(args: Cli) -> Result<Arc<Mutex<notify::RecommendedWatcher>>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let w = Arc::new(Mutex::new(notify::watcher(tx, Duration::from_millis(400))?));
    let w2 = w.clone();
    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            use notify::DebouncedEvent::*;
            if (matches!(event, Write(_) | Remove(_) | Create(_)))
                && PENDING_EVENTS.load(Ordering::SeqCst) == 0
            {
                PENDING_EVENTS.fetch_add(1, Ordering::SeqCst);
                run_in_ui_thread(Box::pin(reload(args.clone(), w2.clone())));
            }
        }
    });
    Ok(w)
}

async fn reload(args: Cli, fswatcher: Arc<Mutex<notify::RecommendedWatcher>>) {
    let mut compiler = init_compiler(&args, Some(fswatcher));
    let c = compiler.build_from_path(&args.path).await;
    sixtyfps_interpreter::print_diagnostics(compiler.diagnostics());

    if let Some(c) = c {
        CURRENT_INSTANCE.with(|current| {
            let mut current = current.borrow_mut();
            if let Some(handle) = current.take() {
                let window = handle.window();
                current.replace(c.create_with_existing_window(window));
            } else {
                let handle = c.create();
                handle.show();
                current.replace(handle);
            }
            if let Some(data_path) = args.load_data {
                let _ = load_data(current.as_ref().unwrap(), &data_path);
            }
            eprintln!("Successful reload of {}", args.path.display());
        });
    }

    PENDING_EVENTS.fetch_sub(1, Ordering::SeqCst);
}

fn load_data(instance: &ComponentInstance, data_path: &std::path::Path) -> Result<()> {
    let json: serde_json::Value = if data_path == std::path::Path::new("-") {
        serde_json::from_reader(std::io::stdin())?
    } else {
        serde_json::from_reader(std::fs::File::open(data_path)?)?
    };

    let obj = json.as_object().ok_or_else(|| "The data is not a JSON object")?;
    for (name, v) in obj {
        fn from_json(v: &serde_json::Value) -> sixtyfps_interpreter::Value {
            match v {
                serde_json::Value::Null => sixtyfps_interpreter::Value::Void,
                serde_json::Value::Bool(b) => (*b).into(),
                serde_json::Value::Number(n) => {
                    sixtyfps_interpreter::Value::Number(n.as_f64().unwrap_or(f64::NAN))
                }
                serde_json::Value::String(s) => SharedString::from(s.as_str()).into(),
                serde_json::Value::Array(array) => {
                    sixtyfps_interpreter::Value::Array(array.iter().map(|v| from_json(v)).collect())
                }
                serde_json::Value::Object(obj) => obj
                    .iter()
                    .map(|(k, v)| (k.clone(), from_json(v)))
                    .collect::<sixtyfps_interpreter::Struct>()
                    .into(),
            }
        }

        match instance.set_property(name, from_json(v)) {
            Ok(()) => (),
            Err(e) => eprintln!("Warning: cannot set property '{}' from data file: {:?}", name, e),
        };
    }
    Ok(())
}

/// This type is duplicated with lsp/preview.rs
struct FutureRunner {
    fut: Mutex<Option<Pin<Box<dyn Future<Output = ()>>>>>,
}

/// Safety: the future is only going to be run in the UI thread
unsafe impl Send for FutureRunner {}
/// Safety: the future is only going to be run in the UI thread
unsafe impl Sync for FutureRunner {}

impl Wake for FutureRunner {
    fn wake(self: Arc<Self>) {
        sixtyfps_rendering_backend_default::backend().post_event(Box::new(move || {
            let waker = self.clone().into();
            let mut cx = std::task::Context::from_waker(&waker);
            let mut fut_opt = self.fut.lock().unwrap();
            if let Some(fut) = &mut *fut_opt {
                match fut.as_mut().poll(&mut cx) {
                    std::task::Poll::Ready(_) => *fut_opt = None,
                    std::task::Poll::Pending => {}
                }
            }
        }));
    }
}

fn run_in_ui_thread(fut: Pin<Box<dyn Future<Output = ()>>>) {
    Arc::new(FutureRunner { fut: Mutex::new(Some(fut)) }).wake()
}
