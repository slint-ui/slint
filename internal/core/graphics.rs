// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![warn(missing_docs)]
/*!
    Graphics Abstractions.

    This module contains the abstractions and convenience types used for rendering.
*/
extern crate alloc;
use crate::lengths::LogicalLength;
use crate::Coord;
use crate::SharedString;

pub use euclid;
/// 2D Rectangle
pub type Rect = euclid::default::Rect<Coord>;
/// 2D Rectangle with integer coordinates
pub type IntRect = euclid::default::Rect<i32>;
/// 2D Point
pub type Point = euclid::default::Point2D<Coord>;
/// 2D Size
pub type Size = euclid::default::Size2D<Coord>;
/// 2D Size in integer coordinates
pub type IntSize = euclid::default::Size2D<u32>;
/// 2D Transform
pub type Transform = euclid::default::Transform2D<Coord>;

pub(crate) mod color;
pub use color::*;

#[cfg(feature = "std")]
mod path;
#[cfg(feature = "std")]
pub use path::*;

mod brush;
pub use brush::*;

pub(crate) mod image;
pub use self::image::*;

pub(crate) mod bitmapfont;
pub use self::bitmapfont::*;

#[cfg(feature = "std")]
pub mod rendering_metrics_collector;

#[cfg(feature = "box-shadow-cache")]
pub mod boxshadowcache;

/// FontRequest collects all the developer-configurable properties for fonts, such as family, weight, etc.
/// It is submitted as a request to the platform font system (i.e. CoreText on macOS) and in exchange the
/// backend returns a `Box<dyn Font>`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FontRequest {
    /// The name of the font family to be used, such as "Helvetica". An empty family name means the system
    /// default font family should be used.
    pub family: Option<SharedString>,
    /// If the weight is None, the system default font weight should be used.
    pub weight: Option<i32>,
    /// If the pixel size is None, the system default font size should be used.
    pub pixel_size: Option<LogicalLength>,
    /// The additional spacing (or shrinking if negative) between glyphs. This is usually not submitted to
    /// the font-subsystem but collected here for API convenience
    pub letter_spacing: Option<LogicalLength>,
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]

    /// Expand Rect so that cbindgen can see it. ( is in fact euclid::default::Rect<f32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    }

    /// Expand IntRect so that cbindgen can see it. ( is in fact euclid::default::Rect<i32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct IntRect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }

    /// Expand Point so that cbindgen can see it. ( is in fact euclid::default::Point2D<f32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Point {
        x: f32,
        y: f32,
    }

    pub use super::path::ffi::*;
}
