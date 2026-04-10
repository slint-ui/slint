// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::graphics::Image;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use ksni::TrayMethods;

struct MenuItem {
    label: std::string::String,
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
struct KsniTray {
    icon: ksni::Icon,
    title: std::string::String,
    menu: std::vec::Vec<MenuItem>,
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
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

    fn menu(&self) -> std::vec::Vec<ksni::MenuItem<KsniTray>> {
        self.menu
            .iter()
            .map(|item| {
                ksni::menu::StandardItem { label: item.label.clone(), ..Default::default() }.into()
            })
            .collect()
    }
}

pub struct Params<'a> {
    pub icon: &'a Image,
    pub title: &'a str,
}

pub struct SystemTray {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    _tray_icon: tray_icon::TrayIcon,
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    _tray: crate::future::JoinHandle<ksni::Handle<KsniTray>>,
}

impl SystemTray {
    pub fn new(params: Params) -> Result<Self, Error> {
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let pixel_buffer = params.icon.to_rgba8().ok_or(Error::Rgba8)?;

            let mut data = pixel_buffer.as_bytes().to_vec();
            let width = pixel_buffer.width() as i32;
            let height = pixel_buffer.height() as i32;

            for pixel in data.chunks_exact_mut(4) {
                pixel.rotate_right(1) // rgba to argb
            }

            let tray = KsniTray {
                icon: ksni::Icon { width, height, data },
                title: params.title.into(),
                menu: std::vec![
                    MenuItem { label: std::format!("Item A") },
                    MenuItem { label: std::format!("Item B") },
                    MenuItem { label: std::format!("Item B") }
                ],
            };

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

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            fn icon_to_tray_icon(icon: &Image) -> Result<tray_icon::Icon, Error> {
                let pixel_buffer = icon.to_rgba8().ok_or(Error::Rgba8)?;

                let rgba = pixel_buffer.as_bytes();
                let width = pixel_buffer.width() as u32;
                let height = pixel_buffer.height() as u32;

                let tray_icon = tray_icon::Icon::from_rgba(rgba.to_vec(), width, height)
                    .map_err(Error::BadIcon)?;

                Ok(tray_icon)
            }

            let icon = icon_to_tray_icon(params.icon)?;

            let tray_icon = tray_icon::TrayIconBuilder::new()
                .with_icon(icon)
                .with_title(params.title)
                .build()
                .map_err(Error::BuildError)?;

            Ok(Self { _tray_icon: tray_icon })
        }
    }
}

#[derive(Debug, derive_more::Error, derive_more::Display)]
pub enum Error {
    #[display("Failed to create a rgba8 buffer from an icon image")]
    Rgba8,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[display("Bad icon: {}", 0)]
    BadIcon(tray_icon::BadIcon),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[display("Build error: {}", 0)]
    BuildError(tray_icon::Error),
    #[display("{}", 0)]
    PlatformError(crate::platform::PlatformError),
    #[display("{}", 0)]
    EventLoopError(crate::api::EventLoopError),
}
