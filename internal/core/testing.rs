// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Helpers shared by the unit tests.

use crate::api::PlatformError;

/// A platform that can't create windows, for unit tests that only need a context.
pub(crate) struct NoWindowPlatform;

impl crate::platform::Platform for NoWindowPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<alloc::rc::Rc<dyn crate::window::WindowAdapter>, PlatformError> {
        Err(PlatformError::Other("this test needs no window".into()))
    }
}
