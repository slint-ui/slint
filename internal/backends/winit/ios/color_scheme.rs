// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Stopgap until winit ships iOS appearance support (rust-windowing/winit#4570).
// Once that lands, `winit_window.theme()` and `WindowEvent::ThemeChanged` will
// work on iOS and this whole module — plus the `ios_color_scheme_observer`
// wiring in winitwindowadapter.rs — can be deleted.

use std::rc::Weak;

use objc2::ClassType;
use objc2_ui_kit::{
    UITraitEnvironment as _, UITraitUserInterfaceStyle, UIUserInterfaceStyle, UIView,
};

use i_slint_core::items::ColorScheme;

use super::trait_observer::{TraitChangeObserver, install_trait_change_observer};
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

pub(crate) fn install_color_scheme_observer(
    view: &UIView,
    adapter: Weak<WinitWindowAdapter>,
) -> Option<TraitChangeObserver> {
    install_trait_change_observer(view, UITraitUserInterfaceStyle::class(), move |env| {
        let Some(adapter) = adapter.upgrade() else { return };
        let scheme = style_to_color_scheme(unsafe { env.traitCollection().userInterfaceStyle() });
        adapter.set_color_scheme(scheme);
    })
}
