// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! macOS and Windows system tray backend using the `tray-icon` crate (muda-based).

use super::{Error, Params};
use crate::graphics::Image;

fn icon_to_tray_icon(icon: &Image) -> Result<::tray_icon::Icon, Error> {
    let pixel_buffer = icon.to_rgba8().ok_or(Error::Rgba8)?;

    let rgba = pixel_buffer.as_bytes();
    let width = pixel_buffer.width() as u32;
    let height = pixel_buffer.height() as u32;

    let tray_icon =
        ::tray_icon::Icon::from_rgba(rgba.to_vec(), width, height).map_err(Error::BadIcon)?;

    Ok(tray_icon)
}

pub struct PlatformTray {
    _tray_icon: ::tray_icon::TrayIcon,
}

impl PlatformTray {
    pub fn new(params: Params) -> Result<Self, Error> {
        let icon = icon_to_tray_icon(params.icon)?;

        let tray_icon = ::tray_icon::TrayIconBuilder::new()
            .with_icon(icon)
            .with_title(params.title)
            .build()
            .map_err(Error::BuildError)?;

        Ok(Self { _tray_icon: tray_icon })
    }
}
