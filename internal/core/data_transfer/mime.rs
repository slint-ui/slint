// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Standardized MIME types supported by Slint.

/// `text/*` namespace
pub mod text {
    /// `text/plain`
    pub const PLAIN: &str = "text/plain";
    /// `text/plain;charset=utf-8`
    pub const PLAIN_UTF_8: &str = "text/plain;charset=utf-8";
}

/// `image/*` namespace
pub mod image {
    /// `image/jpeg`
    pub const JPEG: &str = "image/jpeg";
    /// `image/gif`
    pub const GIF: &str = "image/gif";
    /// `image/png`
    pub const PNG: &str = "image/png";
    /// `image/bmp`
    pub const BMP: &str = "image/bmp";
    // TODO: Should we support non-standard variants of this, e.g. `image/svg`?
    // GZIP-compressed SVG (`svgz`) appears to still use the `image/svg+xml` MIME type.
    /// `image/svg+xml`
    pub const SVG: &str = "image/svg+xml";
}

/// All plaintext MIME types
pub const PLAINTEXT: &[&str] = &[text::PLAIN, text::PLAIN_UTF_8];
/// All image MIME types
pub const IMAGE: &[&str] = &[image::BMP, image::GIF, image::JPEG, image::PNG, image::SVG];
/// All image MIME types that store data as pixels - i.e., at time of writing, all images other than .svg
pub const PIXMAP_IMAGE: &[&str] = &[image::BMP, image::GIF, image::JPEG, image::PNG];
