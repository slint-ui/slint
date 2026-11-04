// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore NSURL

//! Receives the files macOS asks the editor to open, such as on a double-click in Finder.
//!
//! macOS doesn't pass these on the command line.
//! AppKit calls `application:openURLs:` on the application delegate instead,
//! both at launch and while the app is running.
//! winit 0.30 installs a delegate without that method, so `install` adds it to winit's class.
//! winit 0.31 installs no delegate and asks apps to set their own, so `install` does that.

use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Imp, NSObject, ProtocolObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSApplication, NSApplicationDelegate};
use objc2_foundation::{NSArray, NSObjectProtocol, NSURL};

type OpenUrls = extern "C-unwind" fn(&AnyObject, Sel, &NSApplication, &NSArray<NSURL>);

thread_local! {
    static OPEN_FILE: RefCell<Option<Box<dyn Fn(PathBuf)>>> = RefCell::new(None);
    // `NSApplication` doesn't retain its delegate.
    static OWN_DELEGATE: OnceCell<Retained<AppDelegate>> = const { OnceCell::new() };
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SlintEditorAppDelegate"]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(application:openURLs:))]
        fn application_open_urls(&self, _application: &NSApplication, urls: &NSArray<NSURL>) {
            open_first_file(urls);
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

/// Calls `open_file` with the file macOS asks the editor to open.
///
/// When several files are opened at once, only the first one is passed.
///
/// Call this after the winit event loop exists and before it runs,
/// or the file that launched the app is missed.
pub fn install(open_file: impl Fn(PathBuf) + 'static) {
    let mtm =
        MainThreadMarker::new().expect("The editor sets up the event loop on the main thread");
    let application = NSApplication::sharedApplication(mtm);
    match application.delegate() {
        Some(delegate) => {
            let delegate: &AnyObject = delegate.as_ref();
            let class: *const AnyClass = delegate.class();
            // SAFETY: `winit_delegate_open_urls` matches the `v@:@@` encoding of
            // `application:openURLs:`, a void method taking the application and an array of URLs.
            // Adding a method to a registered class is allowed, and doesn't replace an existing one.
            let added = unsafe {
                objc2::ffi::class_addMethod(
                    class.cast_mut(),
                    sel!(application:openURLs:),
                    std::mem::transmute::<OpenUrls, Imp>(winit_delegate_open_urls),
                    c"v@:@@".as_ptr(),
                )
            };
            if !added.as_bool() {
                tracing::warn!("The application delegate already implements application:openURLs:");
                return;
            }
        }
        None => {
            let delegate = AppDelegate::new(mtm);
            application.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            OWN_DELEGATE.with(|own_delegate| own_delegate.set(delegate).ok());
        }
    }
    OPEN_FILE.set(Some(Box::new(open_file)));
}

extern "C-unwind" fn winit_delegate_open_urls(
    _delegate: &AnyObject,
    _cmd: Sel,
    _application: &NSApplication,
    urls: &NSArray<NSURL>,
) {
    open_first_file(urls);
}

fn open_first_file(urls: &NSArray<NSURL>) {
    let Some(path) = urls.iter().find_map(|url| url.to_file_path()) else {
        return;
    };
    // At launch, AppKit calls this from inside `finishLaunching`, before winit is done starting up.
    slint::Timer::single_shot(std::time::Duration::ZERO, move || {
        OPEN_FILE.with_borrow(|open_file| {
            if let Some(open_file) = open_file {
                open_file(path);
            }
        })
    });
}
