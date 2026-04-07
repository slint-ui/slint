// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(any(feature = "system-tray-ksni", feature = "system-tray-tray-icon"))]
use crate::graphics::Image;

#[cfg(not(any(feature = "system-tray-ksni", feature = "system-tray-tray-icon")))]
compile_error!("Either system-tray-ksni or system-tray-tray-icon must be set");

#[cfg(feature = "system-tray-ksni")]
use ksni::blocking::TrayMethods;

#[cfg(feature = "system-tray-ksni")]
struct KsniTray {
    icon: ksni::Icon,
    title: std::string::String,
}

#[cfg(feature = "system-tray-ksni")]
impl ksni::Tray for KsniTray {
    fn id(&self) -> std::string::String {
        // This cannot be empty.
        "slint-tray".into()
    }

    fn title(&self) -> std::string::String {
        self.title.clone()
    }

    fn icon_pixmap(&self) -> std::vec::Vec<ksni::Icon> {
        std::vec![self.icon.clone()]
    }
}

#[derive(Debug)]
pub enum Event {
    #[cfg(feature = "system-tray-tray-icon")]
    TrayIcon(tray_icon::TrayIconEvent),
    #[cfg(feature = "system-tray-tray-icon")]
    TrayIconMenu(tray_icon::menu::MenuEvent),
}

pub struct Params<'a> {
    pub icon: &'a Image,
    pub title: &'a str,
}

pub struct SystemTray {
    #[cfg(feature = "system-tray-tray-icon")]
    tray_icon: tray_icon::TrayIcon,
    #[cfg(feature = "system-tray-ksni")]
    _tray: ksni::blocking::Handle<KsniTray>,
}

impl SystemTray {
    #[cfg(any(feature = "system-tray-ksni", feature = "system-tray-tray-icon"))]
    pub fn new(params: Params) -> Result<Self, Error> {
        #[cfg(feature = "system-tray-ksni")]
        {
            let pixel_buffer = params.icon.to_rgba8().ok_or(Error::Rgba8)?;

            let mut data = pixel_buffer.as_bytes().to_vec();
            let width = pixel_buffer.width() as i32;
            let height = pixel_buffer.height() as i32;

            for pixel in data.chunks_exact_mut(4) {
                pixel.rotate_right(1) // rgba to argb
            }

            let tray =
                KsniTray { icon: ksni::Icon { width, height, data }, title: params.title.into() }
                    .spawn()
                    .map_err(Error::KsniBuildError)?;
            return Ok(Self { _tray: tray });
        }

        #[cfg(feature = "system-tray-tray-icon")]
        {
            let icon = icon_to_tray_icon(params.icon)?;

            let tray_icon = tray_icon::TrayIconBuilder::new()
                .with_icon(icon)
                .with_title(params.title)
                .build()
                .map_err(Error::BuildError)?;

            return Ok(Self { tray_icon });
        }
    }
}

#[derive(Debug, derive_more::Error, derive_more::Display)]
pub enum Error {
    #[display("Failed to create a rgba8 buffer from an icon image")]
    Rgba8,
    #[cfg(feature = "system-tray-tray-icon")]
    #[display("Bad icon: {}", 0)]
    BadIcon(tray_icon::BadIcon),
    #[cfg(feature = "system-tray-tray-icon")]
    #[display("Build error: {}", 0)]
    BuildError(tray_icon::Error),
    #[cfg(feature = "system-tray-ksni")]
    #[display("Build error: {}", 0)]
    KsniBuildError(ksni::Error),
}

#[cfg(feature = "system-tray-tray-icon")]
fn icon_to_tray_icon(icon: &Image) -> Result<tray_icon::Icon, Error> {
    let pixel_buffer = icon.to_rgba8().ok_or(Error::Rgba8)?;

    let rgba = pixel_buffer.as_bytes();
    let width = pixel_buffer.width() as u32;
    let height = pixel_buffer.height() as u32;

    let tray_icon =
        tray_icon::Icon::from_rgba(rgba.to_vec(), width, height).map_err(Error::BadIcon)?;

    Ok(tray_icon)
}
