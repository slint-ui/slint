// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod clipboard;
mod color_scheme;
mod font_size;
mod keyboard_animator;
mod scene;
mod trait_observer;
mod virtual_keyboard;

pub(crate) use clipboard::UiPasteboardClipboard;
pub(crate) use color_scheme::{current_color_scheme, install_color_scheme_observer};
pub(crate) use font_size::{current_default_font_size, install_font_size_observer};
pub(crate) use keyboard_animator::KeyboardCurveSampler;
pub(crate) use scene::{
    SceneLifecycle, attach_window_to_scene, content_view, install_scene_lifecycle,
};
pub(crate) use trait_observer::TraitChangeObserver;
pub(crate) use virtual_keyboard::{KeyboardNotifications, register_keyboard_notifications};

/// Metal rejects GPU work from an app in the background, see #14086.
pub(crate) fn is_in_background() -> bool {
    let mtm = objc2::MainThreadMarker::new().expect("UIKit is only accessible on the main thread");
    objc2_ui_kit::UIApplication::sharedApplication(mtm).applicationState()
        == objc2_ui_kit::UIApplicationState::Background
}
