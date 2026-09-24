// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Windows settings that the backend mirrors into the `SlintContext`.

use i_slint_core::MotionPreference;
use windows::Foundation::TypedEventHandler;
use windows::UI::ViewManagement::{UISettings, UISettingsAnimationsEnabledChangedEventArgs};

/// The "Animation effects" switch under Settings > Accessibility > Visual effects.
/// Defaults to animations on when the setting cannot be read.
pub fn motion_preference() -> MotionPreference {
    let animations_enabled =
        UISettings::new().and_then(|settings| settings.AnimationsEnabled()).unwrap_or(true);
    if animations_enabled { MotionPreference::NoPreference } else { MotionPreference::Reduced }
}

/// Calls back whenever the "Animation effects" setting changes.
///
/// Windows raises the change on a thread of its own, so `notify` has to hop back to the
/// event loop before touching the context. The subscription ends when this is dropped.
pub struct MotionPreferenceObserver {
    settings: UISettings,
    token: i64,
}

impl MotionPreferenceObserver {
    /// `None` on Windows versions without the change event (before Windows 10 1809),
    /// where the setting is read once at startup only.
    pub fn new(notify: impl Fn() + Send + 'static) -> Option<Self> {
        let settings = UISettings::new().ok()?;
        let handler =
            TypedEventHandler::<UISettings, UISettingsAnimationsEnabledChangedEventArgs>::new(
                move |_, _| {
                    notify();
                    Ok(())
                },
            );
        let token = settings.AnimationsEnabledChanged(&handler).ok()?;
        Some(Self { settings, token })
    }
}

impl Drop for MotionPreferenceObserver {
    fn drop(&mut self) {
        let _ = self.settings.RemoveAnimationsEnabledChanged(self.token);
    }
}
