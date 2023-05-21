// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::cell::RefCell;
use std::rc::Rc;

use i_slint_core::platform::PlatformError;

use crate::vulkanwindowadapter::VulkanWindowAdapter;
use i_slint_core::window::WindowAdapter;

pub struct Backend {
    window: RefCell<Option<Rc<VulkanWindowAdapter>>>,
}

impl Backend {
    pub fn new() -> Self {
        Backend { window: Default::default() }
    }
}

impl i_slint_core::platform::Platform for Backend {
    fn create_window_adapter(
        &self,
    ) -> Result<
        std::rc::Rc<dyn i_slint_core::window::WindowAdapter>,
        i_slint_core::platform::PlatformError,
    > {
        let adapter = VulkanWindowAdapter::new()?;

        *self.window.borrow_mut() = Some(adapter.clone());

        Ok(adapter)
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        let adapter = self.window.borrow().as_ref().unwrap().clone();

        loop {
            // Render first frame
            adapter.render_if_needed()?;

            let next_timeout = if adapter.window().has_active_animations() {
                Some(std::time::Duration::from_millis(16))
            } else {
                i_slint_core::platform::duration_until_next_timer_update()
            };

            if let Some(timeout) = next_timeout {
                std::thread::sleep(timeout);
            }

            i_slint_core::platform::update_timers_and_animations();
        }
    }
}
