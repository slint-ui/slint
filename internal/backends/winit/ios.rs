// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod clipboard;
mod color_scheme;
mod font_size;
mod keyboard_animator;
mod touch_finger_id;
mod trait_observer;
mod virtual_keyboard;

pub(crate) use clipboard::UiPasteboardClipboard;
pub(crate) use color_scheme::{current_color_scheme, install_color_scheme_observer};
pub(crate) use font_size::{current_default_font_size, install_font_size_observer};
pub(crate) use keyboard_animator::KeyboardCurveSampler;
pub(crate) use touch_finger_id::TouchFingerIdAllocator;
pub(crate) use trait_observer::TraitChangeObserver;
pub(crate) use virtual_keyboard::{KeyboardNotifications, register_keyboard_notifications};
