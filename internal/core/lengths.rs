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
