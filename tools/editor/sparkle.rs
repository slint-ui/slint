// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore appcast Appcast SPUUpdater SPUStandard localized

//! Just enough of Sparkle to drive the visual editor's update chrome.
//!
//! Sparkle's `SPUUpdater` reports progress through an Objective-C delegate,
//! which the `Delegate` class below implements for the handful of selectors we
//! care about. Sparkle dispatches with `respondsToSelector:`, so the ones we
//! leave out simply fall back to its defaults.

use crate::preview::ui;
use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{
    DefinedClass as _, MainThreadMarker, MainThreadOnly, define_class, extern_class,
    extern_methods, msg_send,
};
use objc2_foundation::{NSBundle, NSError, NSString};
use slint::ComponentHandle as _;
use std::rc::Rc;

/// Sparkle reports "there is nothing to install" as an error on the update
/// cycle, so the one code that actually means success has to be filtered out or
/// every check finishes looking like a failure.
/// SUErrors.h: SUNoUpdateError = 1001, in SUSparkleErrorDomain.
const SU_NO_UPDATE_ERROR: isize = 1001;
const SU_ERROR_DOMAIN: &str = "SUSparkleErrorDomain";

fn is_no_update(error: &NSError) -> bool {
    error.code() == SU_NO_UPDATE_ERROR && error.domain().to_string() == SU_ERROR_DOMAIN
}

/// The message a person should see. `NSError`'s own formatting carries the
/// domain and code, which is noise in a status banner.
fn error_message(error: &NSError) -> String {
    error.localizedDescription().to_string()
}

/// What the updater is doing, reported from the main thread.
#[derive(Debug)]
pub enum Event {
    UpdateAvailable { version: String },
    UpToDate,
    Downloading,
    Downloaded,
    Installing,
    Failed { message: String },
}

extern_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SPUStandardUpdaterController"]
    struct StandardUpdaterController;
);

impl StandardUpdaterController {
    extern_methods!(
        /// The delegates are weak references, so the caller keeps ours alive.
        #[unsafe(method(initWithStartingUpdater:updaterDelegate:userDriverDelegate:))]
        fn init_with_starting_updater(
            this: objc2::rc::Allocated<Self>,
            starting_updater: bool,
            updater_delegate: Option<&NSObject>,
            user_driver_delegate: Option<&NSObject>,
        ) -> Retained<Self>;

        #[unsafe(method(updater))]
        fn updater(&self) -> Retained<Updater>;
    );
}

extern_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SPUUpdater"]
    struct Updater;
);

impl Updater {
    extern_methods!(
        #[unsafe(method(startUpdater:_))]
        fn start_updater(&self) -> Result<(), Retained<NSError>>;

        #[unsafe(method(checkForUpdateInformation))]
        fn check_for_update_information(&self);

        #[unsafe(method(checkForUpdates))]
        fn check_for_updates(&self);

        #[unsafe(method(setAutomaticallyChecksForUpdates:))]
        fn set_automatically_checks_for_updates(&self, value: bool);

        #[unsafe(method(setAutomaticallyDownloadsUpdates:))]
        fn set_automatically_downloads_updates(&self, value: bool);
    );
}

extern_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SUAppcastItem"]
    struct AppcastItem;
);

impl AppcastItem {
    extern_methods!(
        #[unsafe(method(displayVersionString))]
        fn display_version_string(&self) -> Option<Retained<NSString>>;
    );
}

type EventCallback = Box<dyn Fn(Event)>;

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SlintSparkleDelegate"]
    #[ivars = EventCallback]
    struct Delegate;

    impl Delegate {
        #[unsafe(method(updater:didFindValidUpdate:))]
        fn did_find_valid_update(&self, _updater: &NSObject, item: &AppcastItem) {
            let version = item
                .display_version_string()
                .map(|version| version.to_string())
                .unwrap_or_default();
            self.emit(Event::UpdateAvailable { version });
        }

        #[unsafe(method(updaterDidNotFindUpdate:))]
        fn did_not_find_update(&self, _updater: &NSObject) {
            self.emit(Event::UpToDate);
        }

        #[unsafe(method(updater:willDownloadUpdate:withRequest:))]
        fn will_download_update(
            &self,
            _updater: &NSObject,
            _item: &AppcastItem,
            _request: &NSObject,
        ) {
            self.emit(Event::Downloading);
        }

        #[unsafe(method(updater:didDownloadUpdate:))]
        fn did_download_update(&self, _updater: &NSObject, _item: &AppcastItem) {
            self.emit(Event::Downloaded);
        }

        #[unsafe(method(updater:willInstallUpdate:))]
        fn will_install_update(&self, _updater: &NSObject, _item: &AppcastItem) {
            self.emit(Event::Installing);
        }

        /// Returning true lets Sparkle keep its own update cycle going. The
        /// installation block is untyped here because we never call it.
        #[unsafe(method(updater:willInstallUpdateOnQuit:immediateInstallationBlock:))]
        fn will_install_update_on_quit(
            &self,
            _updater: &NSObject,
            _item: &AppcastItem,
            _install: &NSObject,
        ) -> bool {
            self.emit(Event::Installing);
            true
        }

        #[unsafe(method(updater:failedToDownloadUpdate:error:))]
        fn failed_to_download_update(
            &self,
            _updater: &NSObject,
            _item: &AppcastItem,
            error: &NSError,
        ) {
            self.emit(Event::Failed { message: error_message(error) });
        }

        #[unsafe(method(updater:didAbortWithError:))]
        fn did_abort_with_error(&self, _updater: &NSObject, error: &NSError) {
            if is_no_update(error) {
                return;
            }
            self.emit(Event::Failed { message: error_message(error) });
        }

        #[unsafe(method(updater:didFinishUpdateCycleForUpdateCheck:error:))]
        fn did_finish_update_cycle(
            &self,
            _updater: &NSObject,
            _check: isize,
            error: Option<&NSError>,
        ) {
            let Some(error) = error else {
                return;
            };
            if is_no_update(error) {
                return;
            }
            self.emit(Event::Failed { message: error_message(error) });
        }
    }
);

impl Delegate {
    fn new(mtm: MainThreadMarker, callback: EventCallback) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(callback);
        unsafe { msg_send![super(this), init] }
    }

    fn emit(&self, event: Event) {
        (self.ivars())(event)
    }
}

/// Load Sparkle.framework out of the application bundle.
///
/// The classes are only ever reached through the Objective-C runtime, so there
/// is nothing to link against and the framework is loaded here instead. That
/// also keeps a missing framework from stopping the editor from starting.
fn load_framework() -> bool {
    let Some(frameworks) = NSBundle::mainBundle().privateFrameworksPath() else {
        tracing::warn!("No updates: the application bundle has no Frameworks directory");
        return false;
    };
    let path = format!("{frameworks}/Sparkle.framework");

    let Some(bundle) = NSBundle::bundleWithPath(&NSString::from_str(&path)) else {
        tracing::warn!("No updates: Sparkle.framework is missing from {path}");
        return false;
    };

    // SAFETY: loading a framework runs its initializers, which is what we want
    // before touching any of its classes.
    if !unsafe { bundle.load() } {
        tracing::warn!("No updates: Sparkle.framework at {path} failed to load");
        return false;
    }

    true
}

/// Sparkle only works from a real application bundle, so this stays `None` for
/// `cargo run` and the updater is simply inactive there.
fn is_bundled() -> bool {
    NSBundle::mainBundle()
        .bundleIdentifier()
        .map(|identifier| identifier.to_string())
        .is_some_and(|identifier| !identifier.is_empty() && identifier != "com.apple.dt.Xcode.tool")
}

/// Owns the updater and the delegate it reports to. Sparkle keeps only a weak
/// reference to the delegate, so dropping this stops the updates.
pub struct Sparkle {
    _controller: Retained<StandardUpdaterController>,
    _delegate: Retained<Delegate>,
    updater: Retained<Updater>,
}

impl Sparkle {
    /// Returns `None` outside of an application bundle. Must run on the main
    /// thread, which is where the preview builds its window.
    pub fn new(callback: impl Fn(Event) + 'static) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;

        if !is_bundled() {
            tracing::info!(
                "No updates: not running from an application bundle. \
                 This is expected during development."
            );
            return None;
        }

        // Has to happen before any Sparkle class is touched.
        if !load_framework() {
            return None;
        }

        let delegate = Delegate::new(mtm, Box::new(callback));
        let controller = StandardUpdaterController::init_with_starting_updater(
            StandardUpdaterController::alloc(mtm),
            false,
            Some(&delegate),
            None,
        );

        let updater = controller.updater();
        if let Err(error) = updater.start_updater() {
            tracing::warn!("Sparkle updater failed to start: {error}");
            return None;
        }

        // The editor drives the update itself, through the events above.
        updater.set_automatically_checks_for_updates(false);
        updater.set_automatically_downloads_updates(false);

        Some(Self { _controller: controller, _delegate: delegate, updater })
    }

    /// Look for an update without showing anything, reporting through the
    /// event callback so the editor can render its own chrome.
    pub fn check_for_update_information(&self) {
        self.updater.check_for_update_information();
    }

    /// Look for an update using Sparkle's own dialog, which is what a user
    /// gesture should do. It also focuses an update already in progress, so it
    /// works as the action behind the banner in every state.
    pub fn check_for_updates(&self) {
        self.updater.check_for_updates();
    }
}

/// Wire the editor's update chrome to Sparkle. The result has to stay alive for
/// as long as updates should keep working.
pub fn connect(editor: &ui::EditorUi) -> Option<Rc<Sparkle>> {
    let api = editor.global::<ui::Api>();
    let api_weak = <ui::Api as slint::Global<'_, ui::EditorUi>>::as_weak(&api);

    let updater = Rc::new(Sparkle::new(move |event| {
        let Some(api) = api_weak.upgrade() else {
            return;
        };
        api.set_update_error(Default::default());
        match event {
            Event::UpdateAvailable { version } => {
                api.set_update_version(version.into());
                api.set_update_state(ui::UpdateState::Available);
            }
            Event::UpToDate => {
                api.set_update_version(Default::default());
                api.set_update_state(ui::UpdateState::UpToDate);
            }
            Event::Downloading => api.set_update_state(ui::UpdateState::Downloading),
            Event::Downloaded => {
                api.set_update_download_progress(1.0);
                api.set_update_state(ui::UpdateState::ReadyToInstall);
            }
            Event::Installing => api.set_update_state(ui::UpdateState::Installing),
            Event::Failed { message } => {
                api.set_update_error(message.into());
                api.set_update_state(ui::UpdateState::Error);
            }
        }
    })?);

    // The banner reports progress from the events above, but the download and
    // install themselves run in Sparkle's dialog, which is what clicking it
    // opens. Sparkle focuses an update it's already working on, so this is the
    // right call whatever the banner currently says.
    let sparkle = updater.clone();
    api.on_check_for_update(move || sparkle.check_for_updates());

    // scripts/local_sparkle_update_test.sh asks for that dialog at startup.
    if std::env::var_os("SLINT_SPARKLE_INTERACTIVE").is_some() {
        updater.check_for_updates();
    } else {
        updater.check_for_update_information();
    }

    Some(updater)
}
