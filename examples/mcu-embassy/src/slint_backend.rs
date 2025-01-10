// Copyright © 2025 David Haig
// SPDX-License-Identifier: MIT

use alloc::rc::Rc;
use embassy_time::Instant;
use slint::{
    platform::{
        software_renderer::{self, MinimalSoftwareWindow},
        Platform, WindowAdapter,
    },
    PlatformError,
};

pub const DISPLAY_WIDTH: usize = 800;
pub const DISPLAY_HEIGHT: usize = 480;
pub type TargetPixelType = software_renderer::Rgb565Pixel;

pub struct StmBackend {
    window: Rc<MinimalSoftwareWindow>,
}

impl StmBackend {
    pub fn new(window: Rc<MinimalSoftwareWindow>) -> Self {
        Self { window }
    }
}

impl Platform for StmBackend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let window = self.window.clone();
        crate::info!("create_window_adapter called");
        Ok(window)
    }

    fn duration_since_start(&self) -> core::time::Duration {
        Instant::now().duration_since(Instant::from_secs(0)).into()
    }
}
