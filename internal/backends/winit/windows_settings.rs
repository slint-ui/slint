// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore GETCLIENTAREAANIMATION SETTINGCHANGE

//! Windows settings that the backend mirrors into the `SlintContext`.

use i_slint_core::MotionPreference;
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};

/// The "Animation effects" switch under Settings > Accessibility > Visual effects.
/// Defaults to animations on when the setting cannot be read.
///
/// There is no change notification a winit window can receive for this setting
/// (`WM_SETTINGCHANGE` is consumed by winit), so callers read it at startup and
/// again whenever a window gains focus.
pub fn motion_preference() -> MotionPreference {
    let mut animations_enabled = windows::core::BOOL::from(true);
    let read = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut animations_enabled as *mut _ as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if read.is_err() || animations_enabled.as_bool() {
        MotionPreference::NoPreference
    } else {
        MotionPreference::Reduced
    }
}
