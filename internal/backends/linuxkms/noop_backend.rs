// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::platform::PlatformError;
pub struct Backend {}

impl Backend {
    pub fn build(_builder: super::BackendBuilder) -> Result<Self, PlatformError> {
        Ok(Backend {})
    }
}

impl i_slint_core::platform::Platform for Backend {
    fn create_window_adapter(
        &self,
    ) -> Result<
        std::rc::Rc<dyn i_slint_core::window::WindowAdapter>,
        i_slint_core::platform::PlatformError,
    > {
        Err(format!("The linuxkms backend is only supported on Linux").into())
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        unimplemented!()
    }
}
