// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::{OnceCell, RefCell};

use block2::RcBlock;
use objc2::{
    define_class, msg_send, rc::Retained, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_foundation::{NSDefaultRunLoopMode, NSObject, NSObjectProtocol, NSRect, NSRunLoop};
use objc2_quartz_core::{CADisplayLink, CATransaction};
use objc2_ui_kit::{UIView, UIViewAnimating as _, UIViewAnimationCurve, UIViewPropertyAnimator};

struct DisplayLinkTargetIvars {
    view: Retained<UIView>,
    callback: Box<dyn Fn(NSRect)>,
    animator: RefCell<Option<Retained<UIViewPropertyAnimator>>>,
    display_link: OnceCell<Retained<CADisplayLink>>,
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
            let this = self.ivars();
            if let Some(layer) = unsafe { this.view.layer().presentationLayer() } {
                (this.callback)(layer.frame());
            }
            let mut animator_ref = this.animator.borrow_mut();
            if let Some(false) = animator_ref.as_ref().map(|animator| animator.isRunning()) {
                display_link.setPaused(true);
                *animator_ref = None;
            }
        }
    }
);

impl DisplayLinkTarget {
    fn new(
        mtm: MainThreadMarker,
        view: Retained<UIView>,
        callback: impl Fn(NSRect) + 'static,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm)
            .set_ivars(DisplayLinkTargetIvars {
                view,
                callback: Box::new(callback),
                animator: Default::default(),
                display_link: OnceCell::new(),
            });
        unsafe { msg_send![super(this), init] }
    }

    fn set_display_link(&self, display_link: Retained<CADisplayLink>) {
        self.ivars().display_link.set(display_link).unwrap();
    }

    fn stop(&self) {
        let ivars = self.ivars();
        ivars.display_link.get().unwrap().setPaused(true);
        if let Some(animator) = ivars.animator.borrow_mut().take() {
            animator.stopAnimation(true);
        }
    }

    fn start(&self, animator: Retained<UIViewPropertyAnimator>) {
        let ivars = self.ivars();
        animator.startAnimation();
        if let Some(old_animator) = ivars.animator.borrow_mut().replace(animator) {
            old_animator.stopAnimation(true);
        }
        ivars.display_link.get().unwrap().setPaused(false);
    }
}

/// A helper to sample keyboard animation curves.
/// Since the iOS keyboard animation is not directly accessible, we create a hidden UIView
/// and animate its frame using the same parameters as the keyboard animation.
/// The animation curve used by iOS is private and not documented, but using UIViewPropertyAnimator
/// with the same duration and curve produces identical results.
pub(crate) struct KeyboardCurveSampler {
    view: Retained<UIView>,
    target: Retained<DisplayLinkTarget>,
    mtm: MainThreadMarker,
}

impl KeyboardCurveSampler {
    pub(crate) fn new(content_view: &UIView, sampler: impl Fn(NSRect) + 'static) -> Self {
        let mtm = MainThreadMarker::new().expect("Must be created on main thread");
        let view = UIView::new(mtm);
        content_view.addSubview(&view);

        let target = DisplayLinkTarget::new(mtm, view.clone(), sampler);
        let display_link =
            unsafe { CADisplayLink::displayLinkWithTarget_selector(&target, objc2::sel!(tick:)) };

        unsafe {
            display_link.addToRunLoop_forMode(&NSRunLoop::currentRunLoop(), NSDefaultRunLoopMode);
        }

        display_link.setPaused(true);
        target.set_display_link(display_link);

        Self { view, target, mtm }
    }

    pub(crate) fn start(
        &self,
        duration: f64,
        curve: UIViewAnimationCurve,
        begin: NSRect,
        end: NSRect,
    ) {
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        self.target.stop();
        self.view.setFrame(begin);
        CATransaction::commit();

        let view = self.view.clone();
        let animations = RcBlock::new(move || {
            view.setFrame(end);
        });

        let animator = UIViewPropertyAnimator::initWithDuration_curve_animations(
            UIViewPropertyAnimator::alloc(self.mtm),
            duration,  // duration is already in seconds
            curve,
            Some(&animations),
        );

        self.target.start(animator);
    }
}

impl Drop for DisplayLinkTargetIvars {
    fn drop(&mut self) {
        if let Some(display_link) = self.display_link.get() {
            display_link.invalidate();
        }
        if let Some(animator) = self.animator.borrow_mut().take() {
            animator.stopAnimation(true);
        }
        self.view.removeFromSuperview();
    }
}
