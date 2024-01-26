// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use crate::api::PlatformError;
use crate::platform::Platform;

thread_local! {
    pub(crate) static GLOBAL_CONTEXT : once_cell::unsync::OnceCell<SlintContext>
        = once_cell::unsync::OnceCell::new()
}

/// This context is meant to hold the state and the backend.
/// Currently it is not possible to have several platform at the same time in one process, but in the future it might be.
/// See issue #4294
pub struct SlintContext {
    pub(crate) platform: alloc::boxed::Box<dyn Platform>,
    pub window_count: core::cell::RefCell<isize>,
}

/// Internal function to access the platform abstraction.
/// The factory function is called if the platform abstraction is not yet
/// initialized, and should be given by the platform_selector
pub fn with_platform<R>(
    factory: impl FnOnce() -> Result<alloc::boxed::Box<dyn Platform + 'static>, PlatformError>,
    f: impl FnOnce(&dyn Platform) -> Result<R, PlatformError>,
) -> Result<R, PlatformError> {
    GLOBAL_CONTEXT.with(|p| match p.get() {
        Some(ctx) => f(&*ctx.platform),
        None => {
            crate::platform::set_platform(factory()?).map_err(PlatformError::SetPlatformError)?;
            f(&*p.get().unwrap().platform)
        }
    })
}
