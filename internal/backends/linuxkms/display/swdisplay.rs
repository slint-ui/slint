// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

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
    fn as_presenter(self: Rc<Self>) -> Rc<dyn super::Presenter>;
}

mod dumbbuffer;
mod linuxfb;

pub fn new(
    device_opener: &crate::DeviceOpener,
) -> Result<Rc<dyn SoftwareBufferDisplay>, PlatformError> {
    dumbbuffer::DumbBufferDisplay::new(device_opener)
        .or_else(|_| linuxfb::LinuxFBDisplay::new(device_opener))
}
