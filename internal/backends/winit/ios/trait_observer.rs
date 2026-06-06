// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Shared scaffolding for observing UIKit trait changes (color scheme, content
// size category, ...) on a view. Callers pass the trait class to watch and a
// plain closure; the block plumbing and (de)registration live here.

use std::ptr::NonNull;

use block2::RcBlock;
use objc2::{
    Message, available, msg_send,
    rc::Retained,
    runtime::{AnyClass, ProtocolObject},
};
use objc2_foundation::NSArray;
use objc2_ui_kit::{
    UITraitChangeObservable, UITraitChangeRegistration, UITraitCollection, UITraitEnvironment,
    UIView,
};

pub(crate) struct TraitChangeObserver {
    view: Retained<UIView>,
    registration: Retained<ProtocolObject<dyn UITraitChangeRegistration>>,
}

impl Drop for TraitChangeObserver {
    fn drop(&mut self) {
        self.view.unregisterForTraitChanges(&self.registration);
    }
}

/// Invokes `handler` with the changed trait environment whenever `trait_class`
/// changes on `view`. `registerForTraitChanges:withHandler:` is iOS 17+; on older
/// iOS this returns `None` and callers fall back to their initial one-shot query.
pub(crate) fn install_trait_change_observer(
    view: &UIView,
    trait_class: &AnyClass,
    handler: impl Fn(&ProtocolObject<dyn UITraitEnvironment>) + 'static,
) -> Option<TraitChangeObserver> {
    if !available!(ios = 17.0) {
        return None;
    }

    let handler = RcBlock::new(
        move |env: NonNull<ProtocolObject<dyn UITraitEnvironment>>,
              _prev: NonNull<UITraitCollection>| {
            handler(unsafe { env.as_ref() });
        },
    );

    let traits: Retained<NSArray<AnyClass>> = NSArray::from_slice(&[trait_class]);

    let registration: Retained<ProtocolObject<dyn UITraitChangeRegistration>> = unsafe {
        msg_send![
            view,
            registerForTraitChanges: &*traits,
            withHandler: &*handler,
        ]
    };

    Some(TraitChangeObserver { view: view.retain(), registration })
}
