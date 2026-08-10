// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Feeds the system's preferred body text size (Dynamic Type) into the
// SlintContext, where it serves as the default font size for windows that
// don't set `default-font-size` themselves (and as the basis for `rem`).

use std::rc::Weak;

use objc2::ClassType;
use objc2_ui_kit::{
    UIFont, UIFontTextStyleBody, UITraitCollection, UITraitEnvironment as _,
    UITraitPreferredContentSizeCategory, UIView,
};

use i_slint_core::lengths::LogicalLength;

use super::trait_observer::{TraitChangeObserver, install_trait_change_observer};
use crate::winitwindowadapter::WinitWindowAdapter;

/// Returns the preferred size for body text under the given trait collection's
/// content size category. iOS points map 1:1 to Slint's logical pixels.
fn body_font_size(traits: &UITraitCollection) -> LogicalLength {
    let font = UIFont::preferredFontForTextStyle_compatibleWithTraitCollection(
        unsafe { UIFontTextStyleBody },
        Some(traits),
    );
    LogicalLength::new(unsafe { font.pointSize() } as f32)
}

pub(crate) fn current_default_font_size(view: &UIView) -> LogicalLength {
    body_font_size(&view.traitCollection())
}

pub(crate) fn install_font_size_observer(
    view: &UIView,
    adapter: Weak<WinitWindowAdapter>,
) -> Option<TraitChangeObserver> {
    install_trait_change_observer(view, UITraitPreferredContentSizeCategory::class(), move |env| {
        let Some(adapter) = adapter.upgrade() else { return };
        adapter.set_platform_default_font_size(body_font_size(&env.traitCollection()));
    })
}
