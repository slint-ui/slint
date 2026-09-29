// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore HRESULT

//! Drives the editor's update chrome from the Store on Windows.
//!
//! The Store keeps the app up to date on its own, which makes this less about
//! fetching updates than about noticing them: an update applied while the
//! editor is running goes to a package the running process does not have
//! mounted, so it keeps serving the old one until someone launches it again.
//! Without saying so, background updating is worse than no updating, because
//! nothing ever appears to change.
//!
//! Nothing here blocks. WinRT hands results back through completion handlers on
//! threads of its own, which is what the UI thread must not wait on.

use crate::preview::ui;
use slint::ComponentHandle as _;
use std::sync::{Arc, Mutex};
use windows::ApplicationModel::Core::{AppRestartFailureReason, CoreApplication};
use windows::ApplicationModel::{
    Package, PackageCatalog, PackageStatusChangedEventArgs, PackageUpdatingEventArgs,
};
use windows::Foundation::TypedEventHandler;
use windows::Services::Store::{
    StoreContext, StorePackageUpdate, StorePackageUpdateResult, StorePackageUpdateState,
    StorePackageUpdateStatus,
};
use windows::Win32::Foundation::HWND;
use windows_collections::IVectorView;
use windows_future::{
    AsyncOperationCompletedHandler, AsyncOperationProgressHandler,
    AsyncOperationWithProgressCompletedHandler,
};

/// What the updater is doing. Everything here crosses a thread boundary on its
/// way to the UI.
#[derive(Debug)]
pub enum Event {
    UpdateAvailable {
        version: String,
    },
    /// The Store has already installed it and the running process is still on
    /// the old package: only a fresh launch picks it up.
    RestartRequired,
    UpToDate,
    Downloading {
        progress: f32,
    },
    Installing,
    Failed {
        message: String,
    },
}

/// Where the running instance came from. Reading it doubles as the test for
/// "are we packaged", the way `/.flatpak-info` does for Flatpak.
#[derive(Debug, Clone)]
pub struct Instance {
    pub family_name: String,
    pub version: String,
}

pub fn instance() -> Option<Instance> {
    // Outside a package this fails rather than returning something empty, which
    // is exactly the signal wanted.
    let id = Package::Current().ok()?.Id().ok()?;
    let version = id.Version().ok()?;
    Some(Instance {
        family_name: id.FamilyName().ok()?.to_string_lossy(),
        version: format!(
            "{}.{}.{}.{}",
            version.Major, version.Minor, version.Build, version.Revision
        ),
    })
}

type EventCallback = Arc<dyn Fn(Event) + Send + Sync>;

/// Owns the catalog subscription and whatever the last check found. Dropping it
/// unsubscribes, so updates stop being reported.
pub struct Updater {
    catalog: PackageCatalog,
    status_token: i64,
    updating_token: i64,
    callback: EventCallback,
    /// What the last check turned up, kept so that clicking installs the very
    /// updates that were offered rather than going back to ask again.
    pending: Mutex<Option<IVectorView<StorePackageUpdate>>>,
    /// The window the Store hangs its dialogs off. Without it every call that
    /// shows UI fails for a packaged Win32 app.
    window: Mutex<Option<HWND>>,
}

// The handles are used from WinRT's own threads, which is what they are for.
unsafe impl Send for Updater {}
unsafe impl Sync for Updater {}

impl Updater {
    fn new(callback: EventCallback) -> Option<Self> {
        let catalog = PackageCatalog::OpenForCurrentPackage()
            .inspect_err(|error| tracing::warn!("No updates: no package catalog: {error}"))
            .ok()?;

        // Fires once the new package is staged and the running one is stale.
        let status_changed = callback.clone();
        let status_token = catalog
            .PackageStatusChanged(
                &TypedEventHandler::<PackageCatalog, PackageStatusChangedEventArgs>::new(
                    move |_, _| {
                        status_changed(Event::RestartRequired);
                        Ok(())
                    },
                ),
            )
            .ok()?;

        // Reports the install the Store is doing in the background, so the bar
        // says something while it happens rather than jumping to a restart.
        let updating = callback.clone();
        let updating_token = catalog
            .PackageUpdating(&TypedEventHandler::<PackageCatalog, PackageUpdatingEventArgs>::new(
                move |_, args| {
                    let Some(args) = args.as_ref() else {
                        return Ok(());
                    };
                    if args.IsComplete().unwrap_or(false) {
                        updating(Event::RestartRequired);
                    } else {
                        updating(Event::Installing);
                    }
                    Ok(())
                },
            ))
            .ok()?;

        Some(Self {
            catalog,
            status_token,
            updating_token,
            callback,
            pending: Mutex::new(None),
            window: Mutex::new(None),
        })
    }

    /// The Store puts its dialogs over a window, and a packaged Win32 app has to
    /// say which one.
    fn own_dialogs(&self, context: &StoreContext) {
        let Some(hwnd) = *self.window.lock().unwrap() else {
            return;
        };
        use windows::Win32::UI::Shell::IInitializeWithWindow;
        if let Ok(init) = windows_core::Interface::cast::<IInitializeWithWindow>(context) {
            // SAFETY: the handle belongs to the editor's own window, which
            // outlives the dialog it is about to own.
            let _ = unsafe { init.Initialize(hwnd) };
        }
    }

    /// Asks the Store whether anything is waiting. The Store checks on its own
    /// schedule too, so this is about answering sooner, not about being the
    /// only way an update is ever found.
    pub fn check(self: &Arc<Self>) {
        let context = match StoreContext::GetDefault() {
            Ok(context) => context,
            Err(error) => {
                (self.callback)(Event::Failed { message: message(&error) });
                return;
            }
        };

        let operation = match context.GetAppAndOptionalStorePackageUpdatesAsync() {
            Ok(operation) => operation,
            Err(error) => {
                (self.callback)(Event::Failed { message: message(&error) });
                return;
            }
        };

        let updater = self.clone();
        let _ = operation.SetCompleted(&AsyncOperationCompletedHandler::<
            IVectorView<StorePackageUpdate>,
        >::new(move |operation, _| {
            let updates = match operation.ok().and_then(|operation| operation.GetResults()) {
                Ok(updates) => updates,
                Err(error) => {
                    (updater.callback)(Event::Failed { message: message(&error) });
                    return Ok(());
                }
            };

            if updates.Size().unwrap_or(0) == 0 {
                *updater.pending.lock().unwrap() = None;
                (updater.callback)(Event::UpToDate);
                return Ok(());
            }

            let version = updates
                .GetAt(0)
                .and_then(|update| update.Package())
                .and_then(|package| package.Id())
                .and_then(|id| id.Version())
                .map(|v| format!("{}.{}.{}.{}", v.Major, v.Minor, v.Build, v.Revision))
                .unwrap_or_default();

            *updater.pending.lock().unwrap() = Some(updates);
            (updater.callback)(Event::UpdateAvailable { version });
            Ok(())
        }));
    }

    /// Downloads and installs whatever the last check offered. The Store shows
    /// its own confirmation, which is why the window handle matters.
    pub fn install(self: &Arc<Self>) {
        let Some(updates) = self.pending.lock().unwrap().clone() else {
            // Nothing offered yet, so the useful thing is to go and look.
            self.check();
            return;
        };

        let context = match StoreContext::GetDefault() {
            Ok(context) => context,
            Err(error) => {
                (self.callback)(Event::Failed { message: message(&error) });
                return;
            }
        };
        self.own_dialogs(&context);

        let operation = match context.RequestDownloadAndInstallStorePackageUpdatesAsync(&updates) {
            Ok(operation) => operation,
            Err(error) => {
                (self.callback)(Event::Failed { message: message(&error) });
                return;
            }
        };

        let progress = self.callback.clone();
        let _ = operation.SetProgress(&AsyncOperationProgressHandler::new(
            move |_, status: windows_core::Ref<StorePackageUpdateStatus>| {
                let Ok(status) = status.ok() else {
                    return Ok(());
                };
                // The combined call runs 0.0 to 0.8 downloading and 0.8 to 1.0
                // installing. Rescaling the first stretch keeps the bar honest
                // rather than parking it at 80% while the OS installs.
                if status.PackageDownloadProgress < 0.8 {
                    progress(Event::Downloading {
                        progress: (status.PackageDownloadProgress / 0.8) as f32,
                    });
                } else {
                    progress(Event::Installing);
                }
                Ok(())
            },
        ));

        let done = self.callback.clone();
        let _ = operation.SetCompleted(&AsyncOperationWithProgressCompletedHandler::<
            StorePackageUpdateResult,
            StorePackageUpdateStatus,
        >::new(move |operation, _| {
            match operation.ok().and_then(|operation| operation.GetResults()) {
                Ok(result) => match result.OverallState() {
                    Ok(StorePackageUpdateState::Completed) => done(Event::RestartRequired),
                    // Cancelled, out of battery, needs Wi-Fi: all things the
                    // person can retry, so leave the offer standing.
                    Ok(_) => done(Event::UpdateAvailable { version: String::new() }),
                    Err(error) => done(Event::Failed { message: message(&error) }),
                },
                Err(error) => done(Event::Failed { message: message(&error) }),
            }
            Ok(())
        }));
    }

    /// Relaunches into the package the Store already installed.
    ///
    /// Nothing here is automatic: the editor holds at
    /// [`ui::UpdateState::RestartRequired`] until someone clicks, because the
    /// moment to lose a running preview is theirs to pick.
    pub fn restart(self: &Arc<Self>) {
        let arguments = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
        let operation = match CoreApplication::RequestRestartAsync(&arguments.into()) {
            Ok(operation) => operation,
            Err(error) => {
                (self.callback)(Event::Failed { message: message(&error) });
                return;
            }
        };

        // Only ever called back when the restart did not happen: a successful
        // one takes the process with it.
        let callback = self.callback.clone();
        let _ = operation.SetCompleted(
            &AsyncOperationCompletedHandler::<AppRestartFailureReason>::new(move |operation, _| {
                if let Ok(reason) = operation.ok().and_then(|operation| operation.GetResults()) {
                    callback(Event::Failed { message: format!("Could not restart: {reason:?}") });
                }
                Ok(())
            }),
        );
    }
}

impl Drop for Updater {
    fn drop(&mut self) {
        let _ = self.catalog.RemovePackageStatusChanged(self.status_token);
        let _ = self.catalog.RemovePackageUpdating(self.updating_token);
    }
}

/// `Error`'s own formatting carries an HRESULT, which is noise in a status bar.
fn message(error: &windows::core::Error) -> String {
    let message = error.message();
    if message.is_empty() { error.to_string() } else { message }
}

pub fn apply(editor: &ui::EditorUi, event: Event) {
    let api = editor.global::<ui::Api>();
    api.set_update_error(Default::default());
    match event {
        Event::UpdateAvailable { version } => {
            api.set_update_version(version.into());
            api.set_update_download_progress(0.0);
            api.set_update_state(ui::UpdateState::Available);
        }
        Event::UpToDate => {
            api.set_update_version(Default::default());
            api.set_update_state(ui::UpdateState::UpToDate);
        }
        Event::Downloading { progress } => {
            api.set_update_download_progress(progress);
            api.set_update_state(ui::UpdateState::Downloading);
        }
        Event::Installing => {
            api.set_update_download_progress(1.0);
            api.set_update_state(ui::UpdateState::Installing);
        }
        Event::RestartRequired => {
            api.set_update_download_progress(1.0);
            api.set_update_state(ui::UpdateState::RestartRequired);
        }
        Event::Failed { message } => {
            api.set_update_error(message.into());
            api.set_update_state(ui::UpdateState::Error);
        }
    }
}

/// Hands the Store the window its dialogs belong to.
///
/// The winit window only exists once the event loop is running, so this waits
/// for it rather than reaching for a handle that is not there yet.
fn capture_window(editor: slint::Weak<ui::EditorUi>, updater: Arc<Updater>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::winit_030::WinitWindowAccessor;

    slint::spawn_local(async move {
        let editor = editor.upgrade()?;
        let Ok(winit_window) = editor.window().winit_window().await else {
            return None;
        };
        let RawWindowHandle::Win32(handle) = winit_window.window_handle().ok()?.as_raw() else {
            return None;
        };
        *updater.window.lock().unwrap() = Some(HWND(handle.hwnd.get() as *mut core::ffi::c_void));
        Some(())
    })
    .ok();
}

/// Wire the editor's update chrome to the Store. Returns `None` when the editor
/// is not running from a package, which is what `cargo run` does, and the
/// updater is simply inactive there.
///
/// The result has to stay alive for as long as updates should keep working.
pub fn connect(editor: &ui::EditorUi) -> Option<Arc<Updater>> {
    let Some(instance) = instance() else {
        tracing::info!(
            "No updates: not running from a packaged app. This is expected during development."
        );
        return None;
    };
    tracing::info!("Updates: {} {}", instance.family_name, instance.version);

    let weak = editor.as_weak();
    let updater = Arc::new(Updater::new(Arc::new(move |event| {
        let _ = weak.upgrade_in_event_loop(move |editor| apply(&editor, event));
    }))?);

    capture_window(editor.as_weak(), updater.clone());
    updater.check();

    let api = editor.global::<ui::Api>();
    let clicked = updater.clone();
    let weak = editor.as_weak();
    api.on_check_for_update(move || {
        let Some(editor) = weak.upgrade() else {
            return;
        };
        match editor.global::<ui::Api>().get_update_state() {
            ui::UpdateState::RestartRequired => clicked.restart(),
            // Already working on it, and asking again would only stack a second
            // dialog on top of the Store's.
            ui::UpdateState::Downloading | ui::UpdateState::Installing => {}
            _ => clicked.install(),
        }
    });

    Some(updater)
}
