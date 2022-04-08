// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_core::Coord;
pub struct PhysicalPx;
pub type PhysicalLength = euclid::Length<i16, PhysicalPx>;
pub type PhysicalRect = euclid::Rect<i16, PhysicalPx>;
pub type PhysicalSize = euclid::Size2D<i16, PhysicalPx>;
pub type PhysicalPoint = euclid::Point2D<i16, PhysicalPx>;

pub struct LogicalPx;
pub type LogicalLength = euclid::Length<Coord, LogicalPx>;
pub type LogicalRect = euclid::Rect<Coord, LogicalPx>;
pub type LogicalPoint = euclid::Point2D<Coord, LogicalPx>;
pub type LogicalSize = euclid::Size2D<Coord, LogicalPx>;

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

pub trait LogicalItemGeometry {
    fn logical_geometry(self: core::pin::Pin<&Self>) -> LogicalRect;
}

impl<T: i_slint_core::items::Item> LogicalItemGeometry for T {
    fn logical_geometry(self: core::pin::Pin<&Self>) -> LogicalRect {
        LogicalRect::from_untyped(&self.geometry())
    }
}
