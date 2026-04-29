// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub mod text {
    pub const PLAIN: &str = "text/plain";
    pub const PLAIN_UTF_8: &str = "text/plain;charset=utf-8";
}

pub use text::PLAIN as TEXT_PLAIN;
pub use text::PLAIN_UTF_8 as TEXT_PLAIN_UTF_8;

pub mod image {
    pub const JPEG: &str = "image/jpeg";
    pub const GIF: &str = "image/gif";
    pub const PNG: &str = "image/png";
    pub const BMP: &str = "image/bmp";
    pub const SVG: &str = "image/svg+xml";
}

pub use image::BMP as IMAGE_BMP;
pub use image::GIF as IMAGE_GIF;
pub use image::JPEG as IMAGE_JPEG;
pub use image::PNG as IMAGE_PNG;
pub use image::SVG as IMAGE_SVG;

pub const PLAINTEXT: &[&str] = &[TEXT_PLAIN, TEXT_PLAIN_UTF_8];
pub const IMAGE: &[&str] = &[IMAGE_BMP, IMAGE_GIF, IMAGE_JPEG, IMAGE_PNG, IMAGE_SVG];
