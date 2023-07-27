// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_core::{Brush, Color};
use napi::bindgen_prelude::External;

#[napi(js_name = Color)]
pub struct JsColor {
    inner: Color,
}

impl From<Color> for JsColor {
    fn from(color: Color) -> Self {
        Self { inner: color }
    }
}

#[napi]
impl JsColor {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self { inner: Color::default() }
    }

    #[napi(factory)]
    pub fn from_argb_encoded(encoded: u32) -> Self {
        Self { inner: Color::from_argb_encoded(encoded) }
    }

    #[napi(factory)]
    pub fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { inner: Color::from_rgb_u8(red, green, blue) }
    }

    #[napi(factory)]
    pub fn from_argb(alpha: u8, red: u8, green: u8, blue: u8) -> Self {
        Self { inner: Color::from_argb_u8(alpha, red, green, blue) }
    }

    #[napi(getter)]
    pub fn as_argb_encoded(&self) -> u32 {
        self.inner.as_argb_encoded()
    }

    #[napi(getter)]
    pub fn red(&self) -> u8 {
        self.inner.red()
    }

    #[napi(getter)]
    pub fn green(&self) -> u8 {
        self.inner.green()
    }

    #[napi(getter)]
    pub fn blue(&self) -> u8 {
        self.inner.blue()
    }

    #[napi]
    pub fn brighter(&self, factor: f64) -> JsColor {
        JsColor::from(self.inner.brighter(factor as f32))
    }

    #[napi]
    pub fn darker(&self, factor: f64) -> JsColor {
        JsColor::from(self.inner.darker(factor as f32))
    }

    #[napi]
    pub fn transparentize(&self, amount: f64) -> JsColor {
        JsColor::from(self.inner.transparentize(amount as f32))
    }

    #[napi]
    pub fn mix(&self, other: &JsColor, factor: f64) -> JsColor {
        JsColor::from(self.inner.mix(&other.inner, factor as f32))
    }

    #[napi]
    pub fn with_alpha(&self, alpha: f64) -> JsColor {
        JsColor::from(self.inner.with_alpha(alpha as f32))
    }
}

#[napi(js_name = Brush)]
pub struct JsBrush {
    inner: Brush,
}

impl From<Brush> for JsBrush {
    fn from(brush: Brush) -> Self {
        Self { inner: brush }
    }
}

#[napi]
impl JsBrush {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self { inner: Brush::default() }
    }

    #[napi(factory)]
    pub fn from_color(color: &JsColor) -> Self {
        Self { inner: Brush::SolidColor(color.inner) }
    }

    #[napi(getter)]
    pub fn color(&self) -> JsColor {
        self.inner.color().into()
    }

    #[napi(getter)]
    pub fn is_transparent(&self) -> bool {
        self.inner.is_transparent()
    }

    #[napi(getter)]
    pub fn is_opaque(&self) -> bool {
        self.inner.is_opaque()
    }

    #[napi]
    pub fn brighter(&self, factor: f64) -> JsBrush {
        JsBrush::from(self.inner.brighter(factor as f32))
    }

    #[napi]
    pub fn darker(&self, factor: f64) -> JsBrush {
        JsBrush::from(self.inner.darker(factor as f32))
    }

    #[napi]
    pub fn transparentize(&self, amount: f64) -> JsBrush {
        JsBrush::from(self.inner.transparentize(amount as f32))
    }

    #[napi]
    pub fn with_alpha(&self, alpha: f64) -> JsBrush {
        JsBrush::from(self.inner.with_alpha(alpha as f32))
    }

    #[napi(getter)]
    pub fn brush(&self) -> External<Brush> {
        External::new(self.inner.clone())
    }
}
