// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod clipboard;
mod color_scheme;
mod keyboard_animator;
mod virtual_keyboard;

pub(crate) use clipboard::UiPasteboardClipboard;
pub(crate) use color_scheme::{
    ColorSchemeObserver, current_color_scheme, install_color_scheme_observer,
};
pub(crate) use keyboard_animator::KeyboardCurveSampler;
pub(crate) use virtual_keyboard::{KeyboardNotifications, register_keyboard_notifications};
