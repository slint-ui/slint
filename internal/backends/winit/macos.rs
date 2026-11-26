// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Integrates macOS window presentation and application termination.
//!
//! Keeps a freshly shown window invisible until its first frame is submitted.
//!
//! wgpu's Metal surface hands out no drawable while the hosting `NSWindow` reports itself
//! as not visible (gfx-rs/wgpu#8309).
//! A window that hasn't been ordered front yet counts as such.
//! So the frame rendered before showing the window is dropped,
//! and AppKit maps the window with an empty content layer.
//! The title bar shows up alone until `occlusionState` catches up and a redraw lands.
//! Ordering the window front with an alpha of zero hides that gap.
//! The window server still reports such a window as visible, so the redraw goes through,
//! and the alpha is restored once a frame is submitted.
//!
//! wgpu fixes this in <https://github.com/gfx-rs/wgpu/pull/10302>.
//! Remove `RevealOnFirstFrame` once every wgpu version Slint builds against contains that fix.

use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindow};

/// Makes a window fully transparent for as long as this value lives.
/// Drop it when the first frame is submitted.
pub(crate) struct RevealOnFirstFrame {
    ns_window: Retained<NSWindow>,
    alpha: f64,
}

impl RevealOnFirstFrame {
    pub(crate) fn new(winit_window: &winit::window::Window) -> Option<Self> {
        let ns_window = ns_view(winit_window)?.window()?;
        let alpha = ns_window.alphaValue();
        ns_window.setAlphaValue(0.0);
        Some(Self { ns_window, alpha })
    }
}

impl Drop for RevealOnFirstFrame {
    fn drop(&mut self) {
        self.ns_window.setAlphaValue(self.alpha);
    }
}

pub(crate) fn ns_view(winit_window: &winit::window::Window) -> Option<&NSView> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let RawWindowHandle::AppKit(handle) = winit_window.window_handle().ok()?.as_raw() else {
        return None;
    };
    Some(unsafe { handle.ns_view.cast().as_ref() })
}

// Winit 0.30 requires its own delegate class. Replace these patches with a delegate for winit 0.31.
pub(crate) fn install_application_delegate_methods() {
    use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
    use objc2::sel;
    use objc2_app_kit::{NSApplication, NSApplicationTerminateReply};

    unsafe extern "C-unwind" fn should_terminate(
        _this: *mut AnyObject,
        _cmd: Sel,
        app: &NSApplication,
    ) -> NSApplicationTerminateReply {
        if app.isRunning()
            && !is_session_termination(
                objc2_foundation::NSAppleEventManager::sharedAppleEventManager()
                    .currentAppleEvent()
                    .as_deref(),
            )
            && i_slint_core::api::quit_event_loop().is_ok()
        {
            NSApplicationTerminateReply::TerminateCancel
        } else {
            NSApplicationTerminateReply::TerminateNow
        }
    }

    unsafe extern "C-unwind" fn should_not_localize(
        _this: *mut AnyObject,
        _cmd: Sel,
        _app: &NSApplication,
    ) -> Bool {
        Bool::NO
    }

    unsafe fn add_method(
        class: &AnyClass,
        selector: Sel,
        implementation: Imp,
        encoding: &std::ffi::CStr,
    ) {
        if class.instance_method(selector).is_none() {
            unsafe {
                objc2::ffi::class_addMethod(
                    (class as *const AnyClass).cast_mut(),
                    selector,
                    implementation,
                    encoding.as_ptr(),
                );
            }
        }
    }

    if let Some(class) = AnyClass::get(c"WinitApplicationDelegate") {
        unsafe {
            add_method(
                class,
                sel!(applicationShouldTerminate:),
                core::mem::transmute::<
                    unsafe extern "C-unwind" fn(
                        *mut AnyObject,
                        Sel,
                        &NSApplication,
                    ) -> NSApplicationTerminateReply,
                    Imp,
                >(should_terminate),
                c"Q@:@",
            );
            add_method(
                class,
                sel!(applicationShouldAutomaticallyLocalizeKeyEquivalents:),
                core::mem::transmute::<
                    unsafe extern "C-unwind" fn(*mut AnyObject, Sel, &NSApplication) -> Bool,
                    Imp,
                >(should_not_localize),
                c"B@:@",
            );
        }
    }
}

fn is_session_termination(event: Option<&objc2_foundation::NSAppleEventDescriptor>) -> bool {
    use objc2_core_services::{
        kAELogOut, kAEQuitApplication, kAEQuitReason, kAEReallyLogOut, kAERestart, kAEShutDown,
        kCoreEventClass,
    };

    let Some(event) = event else { return false };
    if event.eventClass() != kCoreEventClass || event.eventID() != kAEQuitApplication {
        return false;
    }
    // Canceling a session's Quit event also cancels logout or shutdown.
    event.paramDescriptorForKeyword(kAEQuitReason).is_some_and(|reason| {
        [kAELogOut, kAEReallyLogOut, kAERestart, kAEShutDown].contains(&reason.enumCodeValue())
    })
}

#[cfg(test)]
mod tests {
    use super::is_session_termination;
    use objc2::rc::Retained;
    use objc2_core_services::{
        kAELogOut, kAEQuitAll, kAEQuitApplication, kAEQuitReason, kAEReallyLogOut, kAERestart,
        kAEShutDown, kCoreEventClass,
    };
    use objc2_foundation::NSAppleEventDescriptor;

    fn quit_event(reason: Option<u32>) -> Retained<NSAppleEventDescriptor> {
        let event = NSAppleEventDescriptor::appleEventWithEventClass_eventID_targetDescriptor_returnID_transactionID(
            kCoreEventClass, kAEQuitApplication, None, -1, 0,
        );
        if let Some(reason) = reason {
            event.setParamDescriptor_forKeyword(
                &NSAppleEventDescriptor::descriptorWithEnumCode(reason),
                kAEQuitReason,
            );
        }
        event
    }

    #[test]
    fn session_quit_preserves_system_termination() {
        for reason in [kAELogOut, kAEReallyLogOut, kAERestart, kAEShutDown] {
            assert!(is_session_termination(Some(&quit_event(Some(reason)))));
        }
    }

    #[test]
    fn ordinary_quit_returns_to_the_event_loop() {
        assert!(!is_session_termination(None));
        for reason in [None, Some(kAEQuitAll), Some(u32::from_be_bytes(*b"test"))] {
            assert!(!is_session_termination(Some(&quit_event(reason))));
        }
        for (class, id) in [
            (u32::from_be_bytes(*b"test"), kAEQuitApplication),
            (kCoreEventClass, u32::from_be_bytes(*b"test")),
        ] {
            let event = NSAppleEventDescriptor::appleEventWithEventClass_eventID_targetDescriptor_returnID_transactionID(class, id, None, -1, 0);
            event.setParamDescriptor_forKeyword(
                &NSAppleEventDescriptor::descriptorWithEnumCode(kAEShutDown),
                kAEQuitReason,
            );
            assert!(!is_session_termination(Some(&event)));
        }
    }
}
