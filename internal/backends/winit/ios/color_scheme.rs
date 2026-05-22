// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Stopgap until winit ships iOS appearance support (rust-windowing/winit#4570).
// Once that lands, `winit_window.theme()` and `WindowEvent::ThemeChanged` will
// work on iOS and this whole module — plus the `ios_color_scheme_observer`
// wiring in winitwindowadapter.rs — can be deleted.

use std::ptr::NonNull;
use std::rc::Weak;

use block2::RcBlock;
use objc2::{
    ClassType, Message, available, msg_send,
    rc::Retained,
    runtime::{AnyClass, ProtocolObject},
};
use objc2_foundation::NSArray;
use objc2_ui_kit::{
    UITraitChangeObservable, UITraitChangeRegistration, UITraitCollection, UITraitEnvironment,
    UITraitUserInterfaceStyle, UIUserInterfaceStyle, UIView,
};

use i_slint_core::items::ColorScheme;

use crate::winitwindowadapter::WinitWindowAdapter;

fn style_to_color_scheme(style: UIUserInterfaceStyle) -> ColorScheme {
    match style {
        UIUserInterfaceStyle::Dark => ColorScheme::Dark,
        UIUserInterfaceStyle::Light => ColorScheme::Light,
        _ => ColorScheme::Unknown,
    }
}

pub(crate) fn current_color_scheme(view: &UIView) -> ColorScheme {
    style_to_color_scheme(unsafe { view.traitCollection().userInterfaceStyle() })
}

pub(crate) struct ColorSchemeObserver {
    view: Retained<UIView>,
    registration: Retained<ProtocolObject<dyn UITraitChangeRegistration>>,
}

impl Drop for ColorSchemeObserver {
    fn drop(&mut self) {
        self.view.unregisterForTraitChanges(&self.registration);
    }
}

pub(crate) fn install_color_scheme_observer(
    view: &UIView,
    adapter: Weak<WinitWindowAdapter>,
) -> Option<ColorSchemeObserver> {
    // `registerForTraitChanges:withHandler:` is iOS 17+. Older iOS still gets the
    // initial scheme via `current_color_scheme`, but no live updates.
    if !available!(ios = 17.0) {
        return None;
    }

    let handler = RcBlock::new(
        move |env: NonNull<ProtocolObject<dyn UITraitEnvironment>>,
              _prev: NonNull<UITraitCollection>| {
            let Some(adapter) = adapter.upgrade() else { return };
            let env = unsafe { env.as_ref() };
            let scheme =
                style_to_color_scheme(unsafe { env.traitCollection().userInterfaceStyle() });
            adapter.set_color_scheme(scheme);
        },
    );

    let traits: Retained<NSArray<AnyClass>> =
        NSArray::from_slice(&[UITraitUserInterfaceStyle::class()]);

    let registration: Retained<ProtocolObject<dyn UITraitChangeRegistration>> = unsafe {
        msg_send![
            view,
            registerForTraitChanges: &*traits,
            withHandler: &*handler,
        ]
    };

    Some(ColorSchemeObserver { view: view.retain(), registration })
}
