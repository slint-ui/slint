// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com
// SPDX-License-Identifier: MIT

#[cfg(feature = "pixel-bgra8888")]
mod bgra8888;

#[cfg(feature = "pixel-bgra8888")]
pub use bgra8888::Bgra8888Pixel;
#[cfg(feature = "pixel-rgb888")]
pub use slint::Rgb8Pixel;
#[cfg(feature = "pixel-rgb565")]
pub use slint::platform::software_renderer::Rgb565Pixel;

#[cfg(feature = "pixel-bgra8888")]
pub type PlatformPixel = Bgra8888Pixel;
#[cfg(feature = "pixel-rgb565")]
pub type PlatformPixel = Rgb565Pixel;
#[cfg(feature = "pixel-rgb888")]
pub type PlatformPixel = Rgb8Pixel;
