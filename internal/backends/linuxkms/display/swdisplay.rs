// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::sync::Arc;

use i_slint_core::platform::PlatformError;

pub trait SoftwareBufferDisplay {
    fn size(&self) -> (u32, u32);
    fn map_back_buffer(
        &self,
        callback: &mut dyn FnMut(
            &'_ mut [u8],
            u8,
            drm::buffer::DrmFourcc,
        ) -> Result<(), PlatformError>,
    ) -> Result<(), PlatformError>;
    fn as_presenter(self: Arc<Self>) -> Arc<dyn super::Presenter>;
}

mod dumbbuffer;
mod linuxfb;

#[derive(Debug, Clone)]
pub struct FormatNegotiation {
    /// Formats supported by the renderer, in order of preference (best first)
    pub renderer_formats: Vec<drm::buffer::DrmFourcc>,
    /// Formats supported by the display backend
    pub display_formats: Vec<drm::buffer::DrmFourcc>,
}

impl FormatNegotiation {
    pub fn new(renderer_formats: &[drm::buffer::DrmFourcc]) -> Self {
        Self { renderer_formats: renderer_formats.to_vec(), display_formats: Vec::new() }
    }

    pub fn negotiate(&self) -> Option<drm::buffer::DrmFourcc> {
        for renderer_format in &self.renderer_formats {
            if self.display_formats.contains(renderer_format) {
                return Some(*renderer_format);
            }
        }
        None
    }

    pub fn add_display_formats(&mut self, formats: &[drm::buffer::DrmFourcc]) {
        self.display_formats.extend_from_slice(formats);
    }
}

pub fn new(
    device_opener: &crate::DeviceOpener,
    renderer_formats: &[drm::buffer::DrmFourcc],
) -> Result<Arc<dyn SoftwareBufferDisplay>, PlatformError> {
    let mut negotiation = FormatNegotiation::new(renderer_formats);

    if std::env::var_os("SLINT_BACKEND_LINUXFB").is_some() {
        return linuxfb::LinuxFBDisplay::new(device_opener, &mut negotiation);
    }
    dumbbuffer::DumbBufferDisplay::new(device_opener, &mut negotiation)
        .or_else(|_| linuxfb::LinuxFBDisplay::new(device_opener, &mut negotiation))
}
