// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod keyboard_animator;
mod virtual_keyboard;

pub(crate) use keyboard_animator::KeyboardCurveSampler;
pub(crate) use virtual_keyboard::{register_keyboard_notifications, KeyboardNotifications};
