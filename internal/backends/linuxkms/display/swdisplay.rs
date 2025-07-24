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

pub fn negotiate_format(
    renderer_formats: &[drm::buffer::DrmFourcc],
    display_formats: &[drm::buffer::DrmFourcc],
) -> Option<drm::buffer::DrmFourcc> {
    for &renderer_format in renderer_formats {
        if display_formats.contains(&renderer_format) {
            return Some(renderer_format);
        }
    }
    None
}

pub fn new(
    device_opener: &crate::DeviceOpener,
    renderer_formats: &[drm::buffer::DrmFourcc],
) -> Result<Arc<dyn SoftwareBufferDisplay>, PlatformError> {
    if std::env::var_os("SLINT_BACKEND_LINUXFB").is_some() {
        return linuxfb::LinuxFBDisplay::new(device_opener, renderer_formats);
    }
    dumbbuffer::DumbBufferDisplay::new(device_opener, renderer_formats)
        .or_else(|_| linuxfb::LinuxFBDisplay::new(device_opener, renderer_formats))
}
