// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Weak;

use i_slint_core as corelib;
use i_slint_core::platform::WindowAdapter as _;
use objc2::rc::Retained;
use objc2::runtime::AnyClass;
use objc2::{
    ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSRunLoop, NSRunLoopCommonModes};
use objc2_quartz_core::CADisplayLink;

use crate::winitwindowadapter::WinitWindowAdapter;

struct DisplayLinkTargetIvars {
    window_adapter: Weak<WinitWindowAdapter>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = DisplayLinkTargetIvars]
    struct DisplayLinkTarget;

    unsafe impl NSObjectProtocol for DisplayLinkTarget {}

    impl DisplayLinkTarget {
        #[unsafe(method(tick:))]
        fn tick(&self, display_link: &CADisplayLink) {
            corelib::platform::update_timers_and_animations();
            if let Some(adapter) = self.ivars().window_adapter.upgrade() {
                // Call draw() directly rather than request_redraw(), because
                // during modal tracking loops (e.g. context menus) winit's
                // event loop is blocked and would never process RedrawRequested.
                if let Err(e) = adapter.draw() {
                    i_slint_core::debug_log!("Error rendering during modal loop: {e}");
                    display_link.setPaused(true);
                    return;
                }
                if !adapter.window().has_active_animations() && !adapter.pending_redraw() {
                    display_link.setPaused(true);
                }
            }
        }
    }
);

impl DisplayLinkTarget {
    fn new(mtm: MainThreadMarker, window_adapter: Weak<WinitWindowAdapter>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DisplayLinkTargetIvars { window_adapter });
        unsafe { msg_send![super(this), init] }
    }
}

struct CADisplayLinkFrameThrottle {
    // The display link retains its target per Apple docs, so storing both here
    // without a back-reference from target to display link avoids a retain cycle.
    _target: Retained<DisplayLinkTarget>,
    display_link: Retained<CADisplayLink>,
}

impl Drop for CADisplayLinkFrameThrottle {
    fn drop(&mut self) {
        self.display_link.invalidate();
    }
}

impl super::FrameThrottle for CADisplayLinkFrameThrottle {
    fn request_throttled_redraw(&self) {
        self.display_link.setPaused(false);
    }
}

pub(super) fn try_create(
    window_adapter: Weak<WinitWindowAdapter>,
) -> Option<Box<dyn super::FrameThrottle>> {
    // CADisplayLink on macOS requires 14.0+; check at runtime.
    AnyClass::get(c"CADisplayLink")?;

    let mtm = MainThreadMarker::new().expect("frame throttle must be created on main thread");

    let target = DisplayLinkTarget::new(mtm, window_adapter);
    // Use msg_send! instead of the typed wrapper because the wrapper panics
    // on NULL, which happens in headless CI environments without a display.
    let display_link: Option<Retained<CADisplayLink>> = unsafe {
        msg_send![CADisplayLink::class(), displayLinkWithTarget: &*target, selector: sel!(tick:)]
    };
    let display_link = display_link?;

    // Use NSRunLoopCommonModes so the callback fires during modal tracking loops
    // (context menus, window resize, etc.)
    unsafe {
        display_link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes);
    }

    display_link.setPaused(true);

    Some(Box::new(CADisplayLinkFrameThrottle { _target: target, display_link }))
}
