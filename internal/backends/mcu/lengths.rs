// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

pub struct PhysicalPx;
pub type PhysicalLength = euclid::Length<i16, PhysicalPx>;
pub type PhysicalRect = euclid::Rect<i16, PhysicalPx>;
pub type PhysicalSize = euclid::Size2D<i16, PhysicalPx>;
pub type PhysicalPoint = euclid::Point2D<i16, PhysicalPx>;

pub struct LogicalPx;
pub type LogicalLength = euclid::Length<f32, LogicalPx>;
pub type LogicalRect = euclid::Rect<f32, LogicalPx>;
pub type LogicalPoint = euclid::Point2D<f32, LogicalPx>;
pub type LogicalSize = euclid::Size2D<f32, LogicalPx>;

pub type ScaleFactor = euclid::Scale<f32, LogicalPx, PhysicalPx>;

pub trait SizeLengths<LengthType> {
    fn width_length(&self) -> LengthType;
    fn height_length(&self) -> LengthType;
}

impl SizeLengths<LogicalLength> for LogicalSize {
    fn width_length(&self) -> LogicalLength {
        LogicalLength::new(self.width)
    }

    fn height_length(&self) -> LogicalLength {
        LogicalLength::new(self.height)
    }
}

impl SizeLengths<PhysicalLength> for PhysicalSize {
    fn width_length(&self) -> PhysicalLength {
        PhysicalLength::new(self.width)
    }

    fn height_length(&self) -> PhysicalLength {
        PhysicalLength::new(self.height)
    }
}

pub trait PointLengths<LengthType> {
    fn x_length(&self) -> LengthType;
    fn y_length(&self) -> LengthType;
}

impl PointLengths<LogicalLength> for LogicalPoint {
    fn x_length(&self) -> LogicalLength {
        LogicalLength::new(self.x)
    }

    fn y_length(&self) -> LogicalLength {
        LogicalLength::new(self.y)
    }
}

impl PointLengths<PhysicalLength> for PhysicalPoint {
    fn x_length(&self) -> PhysicalLength {
        PhysicalLength::new(self.x)
    }

    fn y_length(&self) -> PhysicalLength {
        PhysicalLength::new(self.y)
    }
}

pub trait RectLengths<SizeType> {
    fn size_length(&self) -> SizeType;
}

impl RectLengths<LogicalSize> for LogicalRect {
    fn size_length(&self) -> LogicalSize {
        let size = self.size;
        LogicalSize::from_lengths(size.width_length(), size.height_length())
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
