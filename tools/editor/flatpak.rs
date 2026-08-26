// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore flatpak Flatpak vardict

//! Just enough of the Flatpak portal to drive the visual editor's update chrome.
//!
//! `org.freedesktop.portal.Flatpak` hands out an `UpdateMonitor` object, which
//! reports available updates and progress as D-Bus signals and installs one on
//! request. The portal is reachable from inside the sandbox without any
//! `finish-args`: flatpak's session bus proxy lets every app talk to the
//! portals.
//!
//! Two things the interface deliberately does not offer, and the shape of this
//! module follows from them:
//!
//! * There is no way to ask for a check. The monitor polls on the portal's own
//!   timer (30 minutes by default) and never checks when it is created, so
//!   nothing here reports an available update at launch.
//! * `Update` deploys, it does not relaunch. The running process keeps the old
//!   deployment mounted, so a finished update ends in
//!   [`ui::UpdateState::RestartRequired`] rather than in anything that claims
//!   the new version is running.

use crate::preview::ui;
use slint::ComponentHandle as _;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use zbus::MatchRule;
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::message::Type as MessageType;
use zbus::zvariant::{OwnedObjectPath, Value};

const PORTAL_BUS: &str = "org.freedesktop.portal.Flatpak";
const PORTAL_PATH: &str = "/org/freedesktop/portal/Flatpak";
const PORTAL_INTERFACE: &str = "org.freedesktop.portal.Flatpak";
const MONITOR_INTERFACE: &str = "org.freedesktop.portal.Flatpak.UpdateMonitor";

/// The `status` field of the `Progress` signal.
const STATUS_RUNNING: u32 = 0;
const STATUS_EMPTY: u32 = 1;
const STATUS_DONE: u32 = 2;
const STATUS_FAILED: u32 = 3;

/// What the updater is doing. Everything here crosses a thread boundary on its
/// way to the UI.
#[derive(Debug)]
pub enum Event {
    UpdateAvailable {
        version: String,
    },
    /// The update is deployed and waiting for the next launch. Reached both by
    /// finishing an update and by finding one that someone else installed
    /// while the editor was running.
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

/// Where the running instance came from. Everything needed to name this app's
/// ref on the remote, straight out of `/.flatpak-info`, so the same binary is
/// right for every channel and architecture.
#[derive(Debug, Clone)]
pub struct Instance {
    pub app_id: String,
    pub arch: String,
    pub branch: String,
    /// The commit the running process was launched from.
    pub commit: String,
}

/// `/.flatpak-info` exists only inside the sandbox, so this doubles as the test
/// for "are we packaged", the way `is_bundled()` does for Sparkle.
pub fn instance() -> Option<Instance> {
    let info = std::fs::read_to_string("/.flatpak-info").ok()?;

    let mut section = String::new();
    let mut app_id = None;
    let mut arch = None;
    let mut branch = None;
    let mut commit = None;

    for line in info.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|line| line.strip_suffix(']')) {
            section = name.to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match (section.as_str(), key) {
            ("Application", "name") => app_id = Some(value.to_string()),
            ("Instance", "arch") => arch = Some(value.to_string()),
            ("Instance", "branch") => branch = Some(value.to_string()),
            ("Instance", "app-commit") => commit = Some(value.to_string()),
            _ => {}
        }
    }

    Some(Instance { app_id: app_id?, arch: arch?, branch: branch?, commit: commit? })
}

type EventCallback = Arc<dyn Fn(Event) + Send + Sync>;

/// Owns the connection the update monitor belongs to. The portal ties a monitor
/// to the bus name that created it and closes it when that name drops off the
/// bus, so dropping this stops the updates.
pub struct Updater {
    connection: Connection,
    monitor: OwnedObjectPath,
    callback: EventCallback,
}

impl Updater {
    fn new(callback: EventCallback) -> Option<Self> {
        let connection = Connection::session()
            .inspect_err(|error| tracing::warn!("No updates: no session bus: {error}"))
            .ok()?;

        let portal = Proxy::new(&connection, PORTAL_BUS, PORTAL_PATH, PORTAL_INTERFACE)
            .inspect_err(|error| tracing::warn!("No updates: no Flatpak portal: {error}"))
            .ok()?;

        let options: HashMap<&str, Value> = HashMap::new();
        let monitor: OwnedObjectPath = portal
            .call("CreateUpdateMonitor", &(options,))
            .inspect_err(|error| tracing::warn!("No updates: CreateUpdateMonitor failed: {error}"))
            .ok()?;

        Self::watch(&connection, &monitor, callback.clone())?;

        Some(Self { connection, monitor, callback })
    }

    /// Signals arrive on a thread of their own: the blocking iterator parks
    /// until the portal says something, which is exactly what the UI thread
    /// must not do.
    fn watch(
        connection: &Connection,
        monitor: &OwnedObjectPath,
        callback: EventCallback,
    ) -> Option<()> {
        let rule = MatchRule::builder()
            .msg_type(MessageType::Signal)
            .sender(PORTAL_BUS)
            .ok()?
            .interface(MONITOR_INTERFACE)
            .ok()?
            .path(monitor.clone())
            .ok()?
            .build();

        let messages = MessageIterator::for_match_rule(rule, connection, None)
            .inspect_err(|error| tracing::warn!("No updates: cannot watch the monitor: {error}"))
            .ok()?;

        std::thread::spawn(move || {
            for message in messages.flatten() {
                let Some(member) = message.header().member().map(|member| member.to_string())
                else {
                    continue;
                };
                match member.as_str() {
                    "UpdateAvailable" => {
                        if let Ok(info) = message.body().deserialize::<HashMap<String, Value>>() {
                            update_available(&info, &callback);
                        }
                    }
                    "Progress" => {
                        if let Ok(info) = message.body().deserialize::<HashMap<String, Value>>() {
                            progress(&info, &callback);
                        }
                    }
                    _ => {}
                }
            }
        });

        Some(())
    }

    /// Install the update. The portal answers straight away and reports the
    /// work through `Progress`, but it is still a round trip to another
    /// process, so it does not happen on the UI thread.
    ///
    /// This also serves as the only way to ask the portal to look at the
    /// remote: it runs a fresh transaction whatever the monitor last polled,
    /// and reports `STATUS_EMPTY` when there is nothing to install.
    pub fn install(&self) {
        let connection = self.connection.clone();
        let monitor = self.monitor.clone();
        let callback = self.callback.clone();

        std::thread::spawn(move || {
            let options: HashMap<&str, Value> = HashMap::new();
            // The parent window is only used to place the portal's own consent
            // dialog, which it shows the first time an app asks to update
            // itself. Slint does not hand out an X11 or Wayland window
            // identifier in the portal's format, so it goes unparented.
            let result = connection.call_method(
                Some(PORTAL_BUS),
                &monitor,
                Some(MONITOR_INTERFACE),
                "Update",
                &("", options),
            );
            if let Err(error) = result {
                callback(Event::Failed { message: call_error(&error) });
            }
        });
    }
}

fn string<'a>(info: &'a HashMap<String, Value>, key: &str) -> Option<&'a str> {
    match info.get(key)? {
        Value::Str(value) => Some(value.as_str()),
        _ => None,
    }
}

fn number(info: &HashMap<String, Value>, key: &str) -> Option<u32> {
    match info.get(key)? {
        Value::U32(value) => Some(*value),
        _ => None,
    }
}

/// The three commits tell apart the two ways of being out of date: an update
/// waiting on the remote, and one already deployed by `flatpak update` while
/// the editor was running. Only the first is worth downloading.
fn update_available(info: &HashMap<String, Value>, callback: &EventCallback) {
    let running = string(info, "running-commit").unwrap_or_default();
    let local = string(info, "local-commit").unwrap_or_default();
    let remote = string(info, "remote-commit").unwrap_or_default();

    if !local.is_empty() && local == remote && local != running {
        callback(Event::RestartRequired);
    } else {
        callback(Event::UpdateAvailable { version: short_commit(remote) });
    }
}

fn progress(info: &HashMap<String, Value>, callback: &EventCallback) {
    let status = number(info, "status").unwrap_or(STATUS_RUNNING);
    match status {
        STATUS_RUNNING => {
            // `progress` counts the active operation, and an update is
            // typically one operation, so it doubles as the overall progress.
            let percent = number(info, "progress").unwrap_or_default().min(100);
            if percent >= 100 {
                callback(Event::Installing);
            } else {
                callback(Event::Downloading { progress: percent as f32 / 100.0 });
            }
        }
        STATUS_EMPTY => callback(Event::UpToDate),
        STATUS_DONE => callback(Event::RestartRequired),
        STATUS_FAILED => callback(Event::Failed { message: progress_error(info) }),
        _ => {}
    }
}

fn short_commit(commit: &str) -> String {
    commit.chars().take(7).collect()
}

/// The banner is one short line, so the full text goes to the log and only a
/// summary reaches the UI.
fn progress_error(info: &HashMap<String, Value>) -> String {
    let name = string(info, "error").unwrap_or_default();
    let message = string(info, "error_message").unwrap_or_default();
    tracing::warn!("Flatpak update failed: {name}: {message}");

    if name == "org.freedesktop.DBus.Error.NotSupported" {
        // The portal refuses an update that asks for permissions the installed
        // version does not have; only the system tools can approve those.
        return "Update needs new permissions".into();
    }

    summarize(message)
}

fn call_error(error: &zbus::Error) -> String {
    tracing::warn!("Flatpak update could not be started: {error}");
    if let zbus::Error::MethodError(name, message, _) = error {
        if name.as_str() == "org.freedesktop.DBus.Error.NotSupported" {
            return "Update needs new permissions".into();
        }
        return summarize(message.as_deref().unwrap_or_default());
    }
    "Update failed".into()
}

fn summarize(message: &str) -> String {
    let line = message.lines().next().unwrap_or_default().trim();
    if line.is_empty() {
        return "Update failed".into();
    }
    if line.chars().count() > 80 {
        return line.chars().take(79).chain(['…']).collect();
    }
    line.to_string()
}

/// Push an event into the update chrome. Runs on the UI thread.
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

/// Wire the editor's update chrome to the Flatpak portal. Returns `None` when
/// the editor is not running from a Flatpak, which is what `cargo run` does,
/// and the updater is simply inactive there.
///
/// The result has to stay alive for as long as updates should keep working.
pub fn connect(editor: &ui::EditorUi) -> Option<Rc<Updater>> {
    let Some(instance) = instance() else {
        tracing::info!(
            "No updates: not running from a Flatpak. This is expected during development."
        );
        return None;
    };
    tracing::info!(
        "Updates: {} {} on {}, commit {}",
        instance.app_id,
        instance.branch,
        instance.arch,
        short_commit(&instance.commit)
    );

    let weak = editor.as_weak();
    let updater = Rc::new(Updater::new(Arc::new(move |event| {
        let _ = weak.upgrade_in_event_loop(move |editor| apply(&editor, event));
    }))?);

    let api = editor.global::<ui::Api>();
    let clicked = updater.clone();
    let weak = editor.as_weak();
    api.on_check_for_update(move || {
        let Some(editor) = weak.upgrade() else {
            return;
        };
        // In every state but one the banner means "install it": `Update` is
        // also the only way to ask the portal to look at the remote, so it is
        // the right call whatever the banner currently says.
        if editor.global::<ui::Api>().get_update_state() == ui::UpdateState::RestartRequired {
            return;
        }
        clicked.install();
    });

    Some(updater)
}
