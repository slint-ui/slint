// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{Cli, Result, setup_instance, watchable_path};
use i_slint_core::component_factory::FactoryContext;
use i_slint_live_preview::file_watcher::FileWatcher;
use slint::{ComponentFactory, ComponentHandle};
use slint_interpreter::{CompilationResult, Compiler, ComponentDefinition, ComponentInstance};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::task::Waker;

slint::include_modules!();

pub fn run(compiler: Compiler, args: &Cli) -> Result<()> {
    let live =
        LiveReloadingDefinition::new(compiler, args.path().to_path_buf(), args.component.clone())?;
    if let Some(data_path) = &args.load_data
        && let Some(path) = watchable_path(data_path)
    {
        live.borrow_mut().set_extra_watch_paths(vec![path]);
    }

    let ui = SimulatorWindow::new()?;
    install_presentation_behavior(&ui);
    install_factory(&ui, live.borrow().definition().clone(), args);

    {
        let ui = ui.as_weak();
        let callbacks = args.on.clone();
        let load_data_path = args.load_data.clone();
        live.borrow_mut().set_post_reload_hook(move |definition| {
            let Some(ui) = ui.upgrade() else { return };
            install_factory_parts(&ui, definition, &callbacks, load_data_path.as_deref());
        });
    }

    ui.run()?;
    Ok(())
}

fn install_presentation_behavior(ui: &SimulatorWindow) {
    let desktop_size = Rc::new(std::cell::Cell::new(None::<slint::PhysicalSize>));
    let ui_weak = ui.as_weak();
    ui.on_presentation_changed(move |frame_enabled| {
        let Some(ui) = ui_weak.upgrade() else { return };
        if frame_enabled {
            if desktop_size.get().is_none() {
                desktop_size.set(Some(ui.window().size()));
            }
            ui.window().set_size(slint::LogicalSize::new(388., 826.));
        } else if let Some(size) = desktop_size.take() {
            ui.window().set_size(size);
        }
    });
}

fn install_factory(ui: &SimulatorWindow, definition: ComponentDefinition, args: &Cli) {
    install_factory_parts(ui, definition, &args.on, args.load_data.as_deref());
}

fn install_factory_parts(
    ui: &SimulatorWindow,
    definition: ComponentDefinition,
    callbacks: &[String],
    load_data_path: Option<&std::path::Path>,
) {
    let callbacks = callbacks.to_vec();
    let load_data_path = load_data_path.map(ToOwned::to_owned);
    let instance = Rc::new(RefCell::new(None::<ComponentInstance>));
    let retained_instance = instance.clone();
    let factory = ComponentFactory::new(move |ctx: FactoryContext| {
        let embedded = match definition.create_embedded(ctx) {
            Ok(instance) => instance,
            Err(error) => {
                eprintln!("Error while creating the simulator component: {error}");
                return None;
            }
        };
        if let Err(error) = setup_instance(&embedded, &callbacks, load_data_path.as_deref()) {
            eprintln!("Error while setting up the simulator component: {error:?}");
        }
        *retained_instance.borrow_mut() = Some(embedded.clone_strong());
        Some(embedded)
    });
    ui.set_preview_factory(factory);
}

struct LiveReloadingDefinition {
    definition: Option<ComponentDefinition>,
    watcher: Arc<Mutex<Watcher>>,
    compiler: Compiler,
    file_name: PathBuf,
    component_name: Option<String>,
    post_reload_hook: Option<Box<dyn Fn(ComponentDefinition)>>,
    extra_watch_paths: Vec<PathBuf>,
}

impl LiveReloadingDefinition {
    fn new(
        mut compiler: Compiler,
        file_name: PathBuf,
        component_name: Option<String>,
    ) -> Result<Rc<RefCell<Self>>> {
        compiler.set_embed_resources(i_slint_compiler::EmbedResourcesKind::ListAllResources);
        let live = Rc::<RefCell<Self>>::new_cyclic(move |weak| {
            RefCell::new(Self {
                definition: None,
                watcher: Watcher::new(weak.clone()),
                compiler,
                file_name,
                component_name,
                post_reload_hook: None,
                extra_watch_paths: Vec::new(),
            })
        });

        let mut live_mut = live.borrow_mut();
        let result = live_mut.build();
        result.print_diagnostics();
        if result.has_errors() {
            return Err(format!("Could not compile {}", live_mut.file_name.display()).into());
        }
        live_mut.definition = live_mut.find_component(&result);
        if live_mut.definition.is_none() {
            return Err(format!("No component found in {}", live_mut.file_name.display()).into());
        }
        drop(live_mut);
        Ok(live)
    }

    fn reload(&mut self) -> bool {
        let result = self.build();
        result.print_diagnostics();
        if result.has_errors() {
            return false;
        }
        let Some(definition) = self.find_component(&result) else {
            eprintln!(
                "Component {} not found",
                self.component_name.as_deref().unwrap_or("<default>")
            );
            return false;
        };
        self.definition = Some(definition.clone());
        eprintln!(
            "Reloaded component {} from {}",
            self.component_name.as_deref().unwrap_or("<default>"),
            self.file_name.display()
        );
        if let Some(hook) = &self.post_reload_hook {
            hook(definition);
        }
        true
    }

    fn build(&self) -> CompilationResult {
        let mut future = core::pin::pin!(self.compiler.build_from_path(&self.file_name));
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let std::task::Poll::Ready(result) =
            std::future::Future::poll(future.as_mut(), &mut context)
        else {
            unreachable!("Compiler returned Pending")
        };
        Watcher::update_watched_paths(
            &self.watcher,
            std::iter::once(self.file_name.clone())
                .chain(result.watch_paths(i_slint_core::InternalToken).iter().cloned())
                .chain(self.extra_watch_paths.iter().cloned()),
        );
        result
    }

    fn find_component(&self, result: &CompilationResult) -> Option<ComponentDefinition> {
        match &self.component_name {
            Some(name) => result.component(name),
            None => result.components().next(),
        }
    }

    fn definition(&self) -> &ComponentDefinition {
        self.definition.as_ref().expect("set after the first successful build")
    }

    fn set_post_reload_hook(&mut self, hook: impl Fn(ComponentDefinition) + 'static) {
        self.post_reload_hook = Some(Box::new(hook));
    }

    fn set_extra_watch_paths(&mut self, paths: Vec<PathBuf>) {
        self.extra_watch_paths = paths;
    }
}

enum WatcherState {
    Starting,
    Changed,
    Waiting(Waker),
}

struct Watcher {
    watcher: Option<FileWatcher>,
    state: WatcherState,
}

impl Watcher {
    fn new(definition: std::rc::Weak<RefCell<LiveReloadingDefinition>>) -> Arc<Mutex<Self>> {
        let watcher = Arc::new(Mutex::new(Self { watcher: None, state: WatcherState::Starting }));
        let watcher_weak = Arc::downgrade(&watcher);
        let result = slint_interpreter::spawn_local(std::future::poll_fn(move |context| {
            let (Some(definition), Some(watcher)) = (definition.upgrade(), watcher_weak.upgrade())
            else {
                return std::task::Poll::Ready(());
            };
            let state = std::mem::replace(
                &mut watcher.lock().unwrap().state,
                WatcherState::Waiting(context.waker().clone()),
            );
            if matches!(state, WatcherState::Changed) {
                definition.borrow_mut().reload();
            }
            std::task::Poll::Pending
        }));
        if result.is_err() {
            return watcher;
        }

        let watcher_weak = Arc::downgrade(&watcher);
        watcher.lock().unwrap().watcher = FileWatcher::start(
            move |_event| {
                let Some(watcher) = watcher_weak.upgrade() else { return };
                if let WatcherState::Waiting(waker) =
                    std::mem::replace(&mut watcher.lock().unwrap().state, WatcherState::Changed)
                {
                    std::thread::sleep(i_slint_live_preview::REBUILD_DEBOUNCE);
                    waker.wake();
                }
            },
            move |error| eprintln!("Warning: file watcher error: {error}"),
        )
        .ok();
        watcher
    }

    fn update_watched_paths<I>(watcher: &Mutex<Self>, paths: I)
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let mut locked = watcher.lock().unwrap();
        let Some(mut file_watcher) = locked.watcher.take() else { return };
        drop(locked);
        if let Err(error) = file_watcher.update_watched_paths(paths) {
            eprintln!("Warning: error while updating file watcher paths: {error:?}");
        }
        watcher.lock().unwrap().watcher = Some(file_watcher);
    }
}
