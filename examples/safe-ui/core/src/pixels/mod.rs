// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com
// SPDX-License-Identifier: MIT

#[cfg(feature = "pixel-bgra8888")]
mod bgra8888;
#[cfg(feature = "pixel-rgb565")]
mod rgb565;
#[cfg(feature = "pixel-rgb888")]
mod rgb888;

#[cfg(feature = "pixel-bgra8888")]
pub use bgra8888::Bgra8888Pixel;
#[cfg(feature = "pixel-rgb565")]
pub use rgb565::Rgb565Pixel;
#[cfg(feature = "pixel-rgb888")]
pub use rgb888::Rgb888Pixel;

#[cfg(feature = "pixel-bgra8888")]
pub type PlatformPixel = Bgra8888Pixel;
#[cfg(feature = "pixel-rgb565")]
pub type PlatformPixel = Rgb565Pixel;
#[cfg(feature = "pixel-rgb888")]
pub type PlatformPixel = Rgb888Pixel;
