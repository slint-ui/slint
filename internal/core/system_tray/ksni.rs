// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Linux system tray backend using the `ksni` crate (StatusNotifierItem/AppIndicator).

use super::{Error, Params};
use ::ksni::TrayMethods;

struct KsniTray {
    icon: ::ksni::Icon,
    title: std::string::String,
}

impl ::ksni::Tray for KsniTray {
    fn id(&self) -> std::string::String {
        // This cannot be empty.
        "slint-tray".into()
    }

    fn title(&self) -> std::string::String {
        self.title.clone()
    }

    fn icon_pixmap(&self) -> std::vec::Vec<::ksni::Icon> {
        std::vec![self.icon.clone()]
    }

    fn menu(&self) -> std::vec::Vec<::ksni::MenuItem<KsniTray>> {
        std::vec::Vec::new()
    }
}

pub struct PlatformTray {
    _tray: crate::future::JoinHandle<::ksni::Handle<KsniTray>>,
}

impl PlatformTray {
    pub fn new(params: Params) -> Result<Self, Error> {
        let pixel_buffer = params.icon.to_rgba8().ok_or(Error::Rgba8)?;

        let mut data = pixel_buffer.as_bytes().to_vec();
        let width = pixel_buffer.width() as i32;
        let height = pixel_buffer.height() as i32;

        for pixel in data.chunks_exact_mut(4) {
            pixel.rotate_right(1) // rgba to argb
        }

        let tray =
            KsniTray { icon: ::ksni::Icon { width, height, data }, title: params.title.into() };

        let tray = crate::context::with_global_context(
            || panic!(""),
            |ctx| {
                ctx.spawn_local(async move {
                    match tray.spawn().await {
                        Ok(handle) => handle,
                        Err(error) => {
                            panic!("{}", error);
                        }
                    }
                })
            },
        )
        .map_err(Error::PlatformError)?
        .map_err(Error::EventLoopError)?;
        Ok(Self { _tray: tray })
    }
}
