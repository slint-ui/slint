// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::BorderRadius;
use crate::Coord;
/// This type is used as a tagging type for use with [`euclid::Scale`] to convert
/// between physical and logical pixels.
pub struct PhysicalPx;

/// This type is used as a tagging type for use with [`euclid::Scale`] to convert
/// between physical and logical pixels.
pub struct LogicalPx;
pub type LogicalLength = euclid::Length<Coord, LogicalPx>;
pub type LogicalRect = euclid::Rect<Coord, LogicalPx>;
pub type LogicalPoint = euclid::Point2D<Coord, LogicalPx>;
pub type LogicalSize = euclid::Size2D<Coord, LogicalPx>;
pub type LogicalVector = euclid::Vector2D<Coord, LogicalPx>;
pub type LogicalBorderRadius = BorderRadius<Coord, LogicalPx>;
pub type ItemTransform = euclid::Transform2D<f32, LogicalPx, LogicalPx>;

pub type ScaleFactor = euclid::Scale<f32, LogicalPx, PhysicalPx>;

pub trait SizeLengths {
    type LengthType;
    fn width_length(&self) -> Self::LengthType;
    fn height_length(&self) -> Self::LengthType;
}

impl<T: Copy, U> SizeLengths for euclid::Size2D<T, U> {
    type LengthType = euclid::Length<T, U>;
    fn width_length(&self) -> Self::LengthType {
        euclid::Length::new(self.width)
    }
    fn height_length(&self) -> Self::LengthType {
        euclid::Length::new(self.height)
    }
}

pub trait PointLengths {
    type LengthType;
    fn x_length(&self) -> Self::LengthType;
    fn y_length(&self) -> Self::LengthType;
}

impl<T: Copy, U> PointLengths for euclid::Point2D<T, U> {
    type LengthType = euclid::Length<T, U>;
    fn x_length(&self) -> Self::LengthType {
        euclid::Length::new(self.x)
    }
    fn y_length(&self) -> Self::LengthType {
        euclid::Length::new(self.y)
    }
}

impl<T: Copy, U> PointLengths for euclid::Vector2D<T, U> {
    type LengthType = euclid::Length<T, U>;
    fn x_length(&self) -> Self::LengthType {
        euclid::Length::new(self.x)
    }
    fn y_length(&self) -> Self::LengthType {
        euclid::Length::new(self.y)
    }
}

pub trait RectLengths {
    type SizeType;
    type LengthType;
    fn size_length(&self) -> Self::SizeType;
    fn width_length(&self) -> Self::LengthType;
    fn height_length(&self) -> Self::LengthType;
}

impl<T: Copy, U> RectLengths for euclid::Rect<T, U> {
    type LengthType = euclid::Length<T, U>;
    type SizeType = euclid::Size2D<T, U>;
    fn size_length(&self) -> Self::SizeType {
        euclid::Size2D::new(self.size.width, self.size.height)
    }
    fn width_length(&self) -> Self::LengthType {
        self.size_length().width_length()
    }
    fn height_length(&self) -> Self::LengthType {
        self.size_length().height_length()
    }
}

/// Convert from the api size to the internal size
/// (This doesn't use the `From` trait because it would expose the conversion to euclid in the public API)
pub fn logical_size_from_api(size: crate::api::LogicalSize) -> LogicalSize {
    size.to_euclid()
}

pub fn logical_point_from_api(position: crate::api::LogicalPosition) -> LogicalPoint {
    position.to_euclid()
}

pub fn logical_position_to_api(pos: LogicalPoint) -> crate::api::LogicalPosition {
    crate::api::LogicalPosition::from_euclid(pos)
}

pub fn logical_size_to_api(size: LogicalSize) -> crate::api::LogicalSize {
    crate::api::LogicalSize::from_euclid(size)
}

/// An inset represented in the coordinate space of logical pixels. That is the thickness
/// of the border between the safe area and the edges of the window.
#[derive(Debug, Default, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct LogicalInset {
    /// The top inset in logical pixels.
    pub top: f32,
    /// The bottom in logical pixels.
    pub bottom: f32,
    /// The left inset in logical pixels.
    pub left: f32,
    /// The right inset in logical pixels.
    pub right: f32,
}

impl LogicalInset {
    /// Construct a new logical inset from the given border values, that are assumed to be
    /// in the logical coordinate space.
    pub const fn new(top: f32, bottom: f32, left: f32, right: f32) -> Self {
        Self { top, bottom, left, right }
    }

    /// Converts the top inset to logical pixels.
    #[inline]
    pub const fn top(&self) -> LogicalLength {
        LogicalLength::new(self.top as crate::Coord)
    }
    /// Converts the bottom inset to logical pixels.
    #[inline]
    pub const fn bottom(&self) -> LogicalLength {
        LogicalLength::new(self.bottom as crate::Coord)
    }
    /// Converts the left inset to logical pixels.
    #[inline]
    pub const fn left(&self) -> LogicalLength {
        LogicalLength::new(self.left as crate::Coord)
    }
    /// Converts the right inset to logical pixels.
    #[inline]
    pub const fn right(&self) -> LogicalLength {
        LogicalLength::new(self.right as crate::Coord)
    }
}

/// An inset represented in the coordinate space of physical pixels. That is the thickness
/// of the border between the safe area and the edges of the window.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhysicalInset {
    /// The top inset in physical pixels.
    pub top: i32,
    /// The bottom in physical pixels.
    pub bottom: i32,
    /// The left inset in physical pixels.
    pub left: i32,
    /// The right inset in physical pixels.
    pub right: i32,
}

impl PhysicalInset {
    /// Construct a new logical inset from the given border values, that are assumed to be
    /// in the physical coordinate space.
    pub const fn new(top: i32, bottom: i32, left: i32, right: i32) -> Self {
        Self { top, bottom, left, right }
    }

    /// Convert a given logical inset to a physical inset by dividing the lengths by the
    /// specified scale factor.
    #[inline]
    pub const fn to_logical(&self, scale_factor: f32) -> LogicalInset {
        LogicalInset::new(
            self.top_to_logical(scale_factor).0 as f32,
            self.bottom_to_logical(scale_factor).0 as f32,
            self.left_to_logical(scale_factor).0 as f32,
            self.right_to_logical(scale_factor).0 as f32,
        )
    }

    /// Convert the top logical inset to a physical inset by dividing the length by the
    /// specified scale factor.
    #[inline]
    pub const fn top_to_logical(&self, scale_factor: f32) -> LogicalLength {
        LogicalLength::new((self.top as f32 / scale_factor) as crate::Coord)
    }

    /// Convert the bottom logical inset to a physical inset by dividing the length by the
    /// specified scale factor.
    #[inline]
    pub const fn bottom_to_logical(&self, scale_factor: f32) -> LogicalLength {
        LogicalLength::new((self.bottom as f32 / scale_factor) as crate::Coord)
    }

    #[inline]
    /// Convert the left logical inset to a physical inset by dividing the length by the
    /// specified scale factor.
    pub const fn left_to_logical(&self, scale_factor: f32) -> LogicalLength {
        LogicalLength::new((self.left as f32 / scale_factor) as crate::Coord)
    }

    /// Convert the right logical inset to a physical inset by dividing the length by the
    /// specified scale factor.
    #[inline]
    pub const fn right_to_logical(&self, scale_factor: f32) -> LogicalLength {
        LogicalLength::new((self.right as f32 / scale_factor) as crate::Coord)
    }
}
