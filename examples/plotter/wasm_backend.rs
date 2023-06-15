// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: MIT

use plotters_backend::*;

pub struct BackendWithoutText<ForwardedBackend: DrawingBackend> {
    pub backend: ForwardedBackend,
}

impl<ForwardedBackend: DrawingBackend> DrawingBackend for BackendWithoutText<ForwardedBackend> {
    type ErrorType = ForwardedBackend::ErrorType;

    fn get_size(&self) -> (u32, u32) {
        self.backend.get_size()
    }

    fn ensure_prepared(&mut self) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
        self.backend.ensure_prepared()
    }

    fn present(&mut self) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
        self.backend.present()
    }

    fn draw_pixel(
        &mut self,
        point: BackendCoord,
        color: BackendColor,
    ) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
        self.backend.draw_pixel(point, color)
    }

    fn draw_line<S: BackendStyle>(
        &mut self,
        from: BackendCoord,
        to: BackendCoord,
        style: &S,
    ) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
        self.backend.draw_line(from, to, style)
    }

    fn draw_rect<S: BackendStyle>(
        &mut self,
        upper_left: BackendCoord,
        bottom_right: BackendCoord,
        style: &S,
        fill: bool,
    ) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
        self.backend.draw_rect(upper_left, bottom_right, style, fill)
    }

    fn draw_path<S: BackendStyle, I: IntoIterator<Item = BackendCoord>>(
        &mut self,
        path: I,
        style: &S,
    ) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
        self.backend.draw_path(path, style)
    }

    fn draw_circle<S: BackendStyle>(
        &mut self,
        center: BackendCoord,
        radius: u32,
        style: &S,
        fill: bool,
    ) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
        self.backend.draw_circle(center, radius, style, fill)
    }

    fn fill_polygon<S: BackendStyle, I: IntoIterator<Item = BackendCoord>>(
        &mut self,
        vert: I,
        style: &S,
    ) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
        self.backend.fill_polygon(vert, style)
    }

    fn draw_text<TStyle: BackendTextStyle>(
        &mut self,
        _text: &str,
        _style: &TStyle,
        _pos: BackendCoord,
    ) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
        Ok(())
    }

    fn estimate_text_size<TStyle: BackendTextStyle>(
        &self,
        _text: &str,
        _style: &TStyle,
    ) -> Result<(u32, u32), DrawingErrorKind<Self::ErrorType>> {
        Ok((0, 0))
    }

    fn blit_bitmap<'b>(
        &mut self,
        pos: BackendCoord,
        (iw, ih): (u32, u32),
        src: &'b [u8],
    ) -> Result<(), DrawingErrorKind<Self::ErrorType>> {
        self.backend.blit_bitmap(pos, (iw, ih), src)
    }
}
