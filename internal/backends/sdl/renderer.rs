// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! [`ItemRenderer`] implementation using SDL_Renderer.
//!
//! This renderer draws Slint items using SDL3's 2D rendering API. It supports:
//! - Solid-color and bordered rectangles
//! - Text rendering via SDL_ttf
//! - Image rendering via SDL textures
//! - Rectangular clipping via SDL_SetRenderClipRect
//! - Translation and opacity
//!
//! ## Features not yet implemented
//!
//! - **Gradients** (linear, radial, conic): SDL_Renderer has no gradient primitive.
//!   Implementation would require rasterizing the gradient to a texture on the CPU.
//!   Currently falls back to the first color stop of the gradient.
//!
//! - **Paths**: SDL_Renderer has no path/bezier primitive. Would need a rasterization
//!   library (like lyon or zeno) to tessellate paths into triangles or rasterize to a
//!   pixel buffer, then upload as a texture.
//!
//! - **Box shadows**: Requires Gaussian blur which SDL_Renderer doesn't support natively.
//!   Would need multi-pass rendering to offscreen textures with a blur shader, or a
//!   CPU-side blur of a cached shadow texture.
//!
//! - **Rotation/scale transforms**: SDL_RenderTextureRotated can rotate individual
//!   textures, but general item sub-tree rotation needs render-to-texture. Deferred.
//!
//! - **Rounded-rectangle clipping**: SDL_SetRenderClipRect only supports axis-aligned
//!   rectangles. Rounded clipping would need a stencil buffer (not available in
//!   SDL_Renderer) or per-pixel masking via an offscreen texture.
//!
//! - **Layer compositing**: Would need render-to-texture support for correct layer
//!   blending. Currently layers are rendered inline without isolation.

use crate::fonts::FontManager;
use crate::sdl3_bindings::*;
use core::pin::Pin;
use i_slint_core::graphics::{FontRequest, Image};
use i_slint_core::item_rendering::{
    CachedRenderingData, ItemRenderer, RenderBorderRectangle, RenderImage, RenderRectangle,
    RenderText,
};
use i_slint_core::items::{
    self, BoxShadow, ItemRc, Layer, Opacity, Path, TextInput, TextWrap,
};
use i_slint_core::lengths::{
    LogicalBorderRadius, LogicalLength, LogicalPoint, LogicalRect, LogicalSize, LogicalVector,
};
use i_slint_core::window::WindowInner;
use i_slint_core::{Brush, Color};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_int;

/// Graphics state that is saved/restored.
#[derive(Clone, Debug)]
struct RenderState {
    /// Current translation offset (logical pixels).
    offset: LogicalVector,
    /// Current clip rectangle (logical pixels).
    clip: LogicalRect,
    /// Accumulated opacity (0.0–1.0).
    opacity: f32,
}

/// The SDL item renderer. Created per-frame to render the Slint item tree.
#[allow(dead_code)]
pub(crate) struct SdlItemRenderer<'a> {
    renderer: *mut SDL_Renderer,
    text_engine: *mut sdl3_ttf_sys::ttf::TTF_TextEngine,
    font_manager: &'a FontManager,
    scale_factor: f32,
    window_inner: &'a WindowInner,
    /// Texture cache: maps (ItemRc component pointer, index) to an SDL_Texture.
    /// Textures are created on-demand and freed when the component is destroyed.
    texture_cache: &'a RefCell<HashMap<(usize, u32), CachedTexture>>,

    /// State stack for save/restore.
    state: RenderState,
    state_stack: Vec<RenderState>,
}

/// A cached SDL texture.
pub(crate) struct CachedTexture {
    pub texture: *mut SDL_Texture,
}

impl Drop for CachedTexture {
    fn drop(&mut self) {
        if !self.texture.is_null() {
            unsafe { SDL_DestroyTexture(self.texture) };
        }
    }
}

impl<'a> SdlItemRenderer<'a> {
    pub fn new(
        renderer: *mut SDL_Renderer,
        text_engine: *mut sdl3_ttf_sys::ttf::TTF_TextEngine,
        font_manager: &'a FontManager,
        scale_factor: f32,
        window_inner: &'a WindowInner,
        window_size: LogicalSize,
        texture_cache: &'a RefCell<HashMap<(usize, u32), CachedTexture>>,
    ) -> Self {
        Self {
            renderer,
            text_engine,
            font_manager,
            scale_factor,
            window_inner,
            texture_cache,
            state: RenderState {
                offset: LogicalVector::default(),
                clip: LogicalRect::new(LogicalPoint::default(), window_size),
                opacity: 1.0,
            },
            state_stack: Vec::new(),
        }
    }

    /// Convert a logical rect to a physical SDL_FRect, applying current translation.
    fn to_physical_frect(&self, x: f32, y: f32, w: f32, h: f32) -> SDL_FRect {
        let sf = self.scale_factor;
        SDL_FRect {
            x: (x + self.state.offset.x) * sf,
            y: (y + self.state.offset.y) * sf,
            w: w * sf,
            h: h * sf,
        }
    }

    /// Apply the current clip to the SDL renderer.
    fn apply_clip(&self) {
        let sf = self.scale_factor;
        let clip = &self.state.clip;
        let sdl_rect = SDL_Rect {
            x: (clip.origin.x * sf) as c_int,
            y: (clip.origin.y * sf) as c_int,
            w: (clip.size.width * sf).ceil() as c_int,
            h: (clip.size.height * sf).ceil() as c_int,
        };
        unsafe {
            SDL_SetRenderClipRect(self.renderer, &sdl_rect);
        }
    }

    /// Set draw color with current opacity applied.
    fn set_color(&self, color: Color) {
        let a = (color.alpha() as f32 * self.state.opacity) as u8;
        unsafe {
            SDL_SetRenderDrawColor(self.renderer, color.red(), color.green(), color.blue(), a);
        }
    }

    /// Resolve a Brush to a solid Color. For gradients, returns the first color stop.
    fn brush_to_color(&self, brush: &Brush) -> Color {
        match brush {
            Brush::SolidColor(c) => *c,
            Brush::LinearGradient(g) => {
                // Gradient not supported by SDL_Renderer; use first stop color.
                g.stops()
                    .next()
                    .map_or(Color::from_argb_encoded(0), |s| s.color)
            }
            Brush::RadialGradient(g) => {
                g.stops()
                    .next()
                    .map_or(Color::from_argb_encoded(0), |s| s.color)
            }
            Brush::ConicGradient(g) => {
                g.stops()
                    .next()
                    .map_or(Color::from_argb_encoded(0), |s| s.color)
            }
            _ => Color::from_argb_encoded(0),
        }
    }

    /// Render text using the SDL_ttf 3.x renderer text engine API.
    ///
    /// Uses `TTF_CreateText` / `TTF_DrawRendererText` which lets SDL_ttf cache
    /// glyph textures internally, avoiding the per-frame surface→texture upload
    /// that `TTF_RenderText_Blended` + `SDL_CreateTextureFromSurface` would need.
    ///
    /// `phys_offset` is an additional offset in physical pixels applied after
    /// the logical→physical conversion. Used to compensate for TTF_SetFontOutline
    /// expanding glyphs outward.
    fn render_text_to_renderer(
        &self,
        text: &str,
        font: *mut TTF_Font,
        color: Color,
        x: f32,
        y: f32,
        max_width: Option<f32>,
        phys_offset: (f32, f32),
    ) {
        if font.is_null() || text.is_empty() || self.text_engine.is_null() {
            return;
        }

        let a = (color.alpha() as f32 * self.state.opacity) as u8;
        if a == 0 {
            return;
        }

        let sf = self.scale_factor;

        let c_text = match CString::new(text) {
            Ok(s) => s,
            Err(_) => {
                // Text contains interior NULs; render up to the first one.
                let truncated = text.split('\0').next().unwrap_or("");
                match CString::new(truncated) {
                    Ok(s) => s,
                    Err(_) => return,
                }
            }
        };

        let ttf_text = unsafe {
            TTF_CreateText(self.text_engine, font, c_text.as_ptr(), text.len())
        };
        if ttf_text.is_null() {
            return;
        }

        unsafe {
            // Set color with opacity
            TTF_SetTextColor(ttf_text, color.red(), color.green(), color.blue(), a);

            // Set wrap width if wrapping is requested
            if let Some(max_w) = max_width {
                TTF_SetTextWrapWidth(ttf_text, (max_w * sf) as c_int);
            }

            // Draw at the translated position (in physical pixels)
            let phys_x = (x + self.state.offset.x) * sf + phys_offset.0;
            let phys_y = (y + self.state.offset.y) * sf + phys_offset.1;
            TTF_DrawRendererText(ttf_text, phys_x, phys_y);

            TTF_DestroyText(ttf_text);
        }
    }

    /// Create an SDL texture from Slint image pixel data.
    fn create_texture_from_image(&self, image: &Image) -> *mut SDL_Texture {
        // Use the public API to get RGBA8 pixel data
        if let Some(pixels) = image.to_rgba8() {
            let w = pixels.width() as c_int;
            let h = pixels.height() as c_int;
            if w == 0 || h == 0 {
                return std::ptr::null_mut();
            }
            unsafe {
                let texture = SDL_CreateTexture(
                    self.renderer,
                    SDL_PIXELFORMAT_RGBA32,
                    SDL_TEXTUREACCESS_STATIC,
                    w,
                    h,
                );
                if texture.is_null() {
                    return std::ptr::null_mut();
                }
                SDL_SetTextureBlendMode(texture, SDL_BLENDMODE_BLEND);
                SDL_UpdateTexture(
                    texture,
                    std::ptr::null(),
                    pixels.as_bytes().as_ptr() as *const _,
                    w * 4,
                );
                texture
            }
        } else {
            std::ptr::null_mut()
        }
    }

}

impl<'a> ItemRenderer for SdlItemRenderer<'a> {
    fn draw_rectangle(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _self_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let brush = rect.background();
        let color = self.brush_to_color(&brush);
        if color.alpha() == 0 {
            return;
        }

        self.set_color(color);
        let frect = self.to_physical_frect(0.0, 0.0, size.width, size.height);
        unsafe {
            SDL_SetRenderDrawBlendMode(self.renderer, SDL_BLENDMODE_BLEND);
            SDL_RenderFillRect(self.renderer, &frect);
        }
    }

    fn draw_border_rectangle(
        &mut self,
        rect: Pin<&dyn RenderBorderRectangle>,
        _self_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let bg = rect.background();
        let border_color = rect.border_color();
        let border_width = rect.border_width().get();
        let _border_radius = rect.border_radius();

        // Note: Rounded corners are not supported by SDL_Renderer's FillRect/Rect.
        // A full implementation would rasterize rounded corners to a texture or use
        // SDL_RenderGeometry with triangles. For now, we draw sharp rectangles.

        // Draw background fill (inset by border width)
        let bg_color = self.brush_to_color(&bg);
        if bg_color.alpha() > 0 {
            self.set_color(bg_color);
            let frect = self.to_physical_frect(
                border_width,
                border_width,
                (size.width - 2.0 * border_width).max(0.0),
                (size.height - 2.0 * border_width).max(0.0),
            );
            unsafe {
                SDL_SetRenderDrawBlendMode(self.renderer, SDL_BLENDMODE_BLEND);
                SDL_RenderFillRect(self.renderer, &frect);
            }
        }

        // Draw border
        if border_width > 0.0 {
            let bc = self.brush_to_color(&border_color);
            if bc.alpha() > 0 {
                self.set_color(bc);
                let sf = self.scale_factor;
                let phys_bw = (border_width * sf).max(1.0);
                let ox = (self.state.offset.x) * sf;
                let oy = (self.state.offset.y) * sf;
                let pw = size.width * sf;
                let ph = size.height * sf;

                unsafe {
                    SDL_SetRenderDrawBlendMode(self.renderer, SDL_BLENDMODE_BLEND);

                    // Top border
                    SDL_RenderFillRect(self.renderer, &SDL_FRect {
                        x: ox, y: oy, w: pw, h: phys_bw,
                    });
                    // Bottom border
                    SDL_RenderFillRect(self.renderer, &SDL_FRect {
                        x: ox, y: oy + ph - phys_bw, w: pw, h: phys_bw,
                    });
                    // Left border
                    SDL_RenderFillRect(self.renderer, &SDL_FRect {
                        x: ox, y: oy + phys_bw, w: phys_bw, h: ph - 2.0 * phys_bw,
                    });
                    // Right border
                    SDL_RenderFillRect(self.renderer, &SDL_FRect {
                        x: ox + pw - phys_bw, y: oy + phys_bw, w: phys_bw, h: ph - 2.0 * phys_bw,
                    });
                }
            }
        }
    }

    fn draw_window_background(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        self_rc: &ItemRc,
        size: LogicalSize,
        cache: &CachedRenderingData,
    ) {
        self.draw_rectangle(rect, self_rc, size, cache);
    }

    fn draw_image(
        &mut self,
        image: Pin<&dyn RenderImage>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let source = image.source();
        let target_size = image.target_size();

        if target_size.width <= 0.0 || target_size.height <= 0.0 {
            return;
        }

        let texture = self.create_texture_from_image(&source);
        if texture.is_null() {
            return;
        }

        let a = (255.0 * self.state.opacity) as u8;
        unsafe {
            SDL_SetTextureAlphaMod(texture, a);
            SDL_SetTextureBlendMode(texture, SDL_BLENDMODE_BLEND);
        }

        // Apply colorization if requested
        let colorize = image.colorize();
        let colorize_color = self.brush_to_color(&colorize);
        if colorize_color != Color::default() && colorize_color.alpha() > 0 {
            unsafe {
                SDL_SetTextureColorMod(
                    texture,
                    colorize_color.red(),
                    colorize_color.green(),
                    colorize_color.blue(),
                );
            }
        }

        let dst = self.to_physical_frect(0.0, 0.0, target_size.width, target_size.height);

        // Handle source clipping
        let src_rect = image.source_clip().map(|clip| SDL_FRect {
            x: clip.origin.x as f32,
            y: clip.origin.y as f32,
            w: clip.size.width as f32,
            h: clip.size.height as f32,
        });

        unsafe {
            SDL_RenderTexture(
                self.renderer,
                texture,
                src_rect
                    .as_ref()
                    .map_or(std::ptr::null(), |r| r as *const _),
                &dst,
            );
            SDL_DestroyTexture(texture);
        }
    }

    fn draw_text(
        &mut self,
        text: Pin<&dyn RenderText>,
        self_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let string = match text.text() {
            i_slint_core::item_rendering::PlainOrStyledText::Plain(s) => s.to_string(),
            i_slint_core::item_rendering::PlainOrStyledText::Styled(s) => {
                i_slint_core::styled_text::get_raw_text(&s).into_owned()
            }
        };

        if string.is_empty() {
            return;
        }

        let color = self.brush_to_color(&text.color());
        if color.alpha() == 0 {
            return;
        }

        let font_request = text.font_request(self_rc);
        let font = self.font_manager.font_for_request(&font_request, self.scale_factor, 0);
        if font.is_null() {
            return;
        }

        let (text_w, text_h) = self.font_manager.text_size(
            font,
            &string,
            if text.wrap() != TextWrap::NoWrap {
                Some(size.width * self.scale_factor)
            } else {
                None
            },
        );

        // Compute alignment offset
        let (halign, valign) = text.alignment();
        let x_offset = match halign {
            items::TextHorizontalAlignment::Left => 0.0,
            items::TextHorizontalAlignment::Center => {
                (size.width - text_w / self.scale_factor) / 2.0
            }
            items::TextHorizontalAlignment::Right => size.width - text_w / self.scale_factor,
            _ => 0.0,
        };
        let y_offset = match valign {
            items::TextVerticalAlignment::Top => 0.0,
            items::TextVerticalAlignment::Center => {
                (size.height - text_h / self.scale_factor) / 2.0
            }
            items::TextVerticalAlignment::Bottom => size.height - text_h / self.scale_factor,
            _ => 0.0,
        };

        let max_width = if text.wrap() != TextWrap::NoWrap {
            Some(size.width)
        } else {
            None
        };

        // Handle text stroke (outline). TTF_SetFontOutline renders an outline
        // around each glyph. We draw the outline pass first, then the fill on top.
        let (stroke_brush, stroke_width, stroke_style) = text.stroke();
        let stroke_color = self.brush_to_color(&stroke_brush);
        let stroke_px = stroke_width.get() * self.scale_factor;

        if stroke_color.alpha() > 0 && stroke_px > 0.0 {
            let outline = match stroke_style {
                // Outside: the full stroke width is outside the glyph edge
                items::TextStrokeStyle::Outside => stroke_px.round() as i32,
                // Center: half the stroke is outside, half inside
                items::TextStrokeStyle::Center => (stroke_px / 2.0).round().max(1.0) as i32,
                _ => stroke_px.round() as i32,
            };
            let outlined_font = self.font_manager.font_for_request(&font_request, self.scale_factor, outline);
            let px = -((2 * outline) as f32);
            self.render_text_to_renderer(&string, outlined_font, stroke_color, x_offset, y_offset, max_width, (px, px));
        }

        self.render_text_to_renderer(&string, font, color, x_offset, y_offset, max_width, (0.0, 0.0));
    }

    fn draw_text_input(
        &mut self,
        text_input: Pin<&TextInput>,
        self_rc: &ItemRc,
        _size: LogicalSize,
    ) {
        let font_request = text_input.font_request(self_rc);
        let font = self
            .font_manager
            .font_for_request(&font_request, self.scale_factor, 0);

        let visual = text_input.visual_representation(None);
        let text = &visual.text;

        if !text.is_empty() {
            let color = self.brush_to_color(&text_input.color());
            self.render_text_to_renderer(text, font, color, 0.0, 0.0, None, (0.0, 0.0));
        }

        // Draw cursor
        if text_input.cursor_visible() && text_input.enabled() {
            if let Some(cursor_pos) = visual.cursor_position {
                if !font.is_null() {
                    let cursor_pos = cursor_pos.min(visual.text.len());
                    let x = self
                        .font_manager
                        .x_for_byte_offset(font, &visual.text, cursor_pos)
                        / self.scale_factor;
                    let cursor_width = text_input.text_cursor_width().get();
                    let font_height =
                        unsafe { TTF_GetFontHeight(font) } as f32 / self.scale_factor;

                    let cursor_color = visual.cursor_color;
                    self.set_color(cursor_color);
                    let frect = self.to_physical_frect(x, 0.0, cursor_width, font_height);
                    unsafe {
                        SDL_SetRenderDrawBlendMode(self.renderer, SDL_BLENDMODE_BLEND);
                        SDL_RenderFillRect(self.renderer, &frect);
                    }
                }
            }
        }

        // Draw selection background
        if visual.selection_range.start != visual.selection_range.end && !font.is_null() {
            let sel_start = self
                .font_manager
                .x_for_byte_offset(font, &visual.text, visual.selection_range.start)
                / self.scale_factor;
            let sel_end = self
                .font_manager
                .x_for_byte_offset(font, &visual.text, visual.selection_range.end)
                / self.scale_factor;
            let font_height =
                unsafe { TTF_GetFontHeight(font) } as f32 / self.scale_factor;

            let sel_color = text_input.selection_background_color();
            self.set_color(sel_color);
            let frect = self.to_physical_frect(
                sel_start.min(sel_end),
                0.0,
                (sel_end - sel_start).abs(),
                font_height,
            );
            unsafe {
                SDL_SetRenderDrawBlendMode(self.renderer, SDL_BLENDMODE_BLEND);
                SDL_RenderFillRect(self.renderer, &frect);
            }

            // Re-render selected text with selection foreground color
            let sel_fg = text_input.selection_foreground_color();
            let range = visual.selection_range.clone();
            let start = range.start.min(visual.text.len());
            let end = range.end.min(visual.text.len());
            if start < end {
                let selected_text = &visual.text[start..end];
                self.render_text_to_renderer(
                    selected_text,
                    font,
                    sel_fg,
                    sel_start.min(sel_end),
                    0.0,
                    None,
                    (0.0, 0.0),
                );
            }
        }
    }

    fn draw_path(&mut self, _path: Pin<&Path>, _self_rc: &ItemRc, _size: LogicalSize) {
        // Path rendering is not supported by SDL_Renderer.
        // A full implementation would need to rasterize the path (e.g., using the zeno or lyon
        // crate) into a pixel buffer, upload it as a texture, and render that texture.
        log::warn!("Path rendering is not implemented in the SDL backend");
    }

    fn draw_box_shadow(
        &mut self,
        _box_shadow: Pin<&BoxShadow>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) {
        // Box shadow rendering requires Gaussian blur which SDL_Renderer doesn't support.
        // A full implementation would:
        // 1. Render the shadow shape (with border-radius) to an offscreen texture
        // 2. Apply a multi-pass Gaussian blur (horizontal + vertical)
        // 3. Render the blurred texture at the shadow offset
        // This is non-trivial and deferred for now.
        log::debug!("Box shadow rendering is not implemented in the SDL backend");
    }

    fn visit_opacity(
        &mut self,
        opacity_item: Pin<&Opacity>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> i_slint_core::items::RenderingResult {
        self.apply_opacity(opacity_item.opacity());
        i_slint_core::items::RenderingResult::ContinueRenderingChildren
    }

    fn visit_layer(
        &mut self,
        _layer_item: Pin<&Layer>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> i_slint_core::items::RenderingResult {
        // Layer compositing would require rendering children to an offscreen texture,
        // then compositing that texture back. This is a significant feature that
        // requires SDL_SetRenderTarget support. Deferred for now.
        i_slint_core::items::RenderingResult::ContinueRenderingChildren
    }

    fn combine_clip(
        &mut self,
        rect: LogicalRect,
        _radius: LogicalBorderRadius,
        _border_width: LogicalLength,
    ) -> bool {
        // Note: rounded-rectangle clipping is not supported; only axis-aligned rect clipping.
        let clip_rect = LogicalRect::new(
            LogicalPoint::new(
                rect.origin.x + self.state.offset.x,
                rect.origin.y + self.state.offset.y,
            ),
            rect.size,
        );

        if let Some(intersection) = self.state.clip.intersection(&clip_rect) {
            self.state.clip = intersection;
            self.apply_clip();
            true
        } else {
            self.state.clip =
                LogicalRect::new(self.state.clip.origin, LogicalSize::new(0.0, 0.0));
            self.apply_clip();
            false
        }
    }

    fn get_current_clip(&self) -> LogicalRect {
        // Return clip in the current translation coordinate system
        LogicalRect::new(
            LogicalPoint::new(
                self.state.clip.origin.x - self.state.offset.x,
                self.state.clip.origin.y - self.state.offset.y,
            ),
            self.state.clip.size,
        )
    }

    fn translate(&mut self, distance: LogicalVector) {
        self.state.offset.x += distance.x;
        self.state.offset.y += distance.y;
    }

    fn translation(&self) -> LogicalVector {
        self.state.offset
    }

    fn rotate(&mut self, _angle_in_degrees: f32) {
        // General rotation of item sub-trees is not supported by SDL_Renderer's clip/fill API.
        // Individual texture rendering can be rotated via SDL_RenderTextureRotated, but
        // rotating an entire sub-tree requires rendering to an offscreen target first.
        log::debug!("Rotation transforms are not implemented in the SDL backend");
    }

    fn scale(&mut self, _scale_x: f32, _scale_y: f32) {
        // Similar to rotation, general scaling of item sub-trees would need render-to-texture.
        log::debug!("Scale transforms are not implemented in the SDL backend");
    }

    fn apply_opacity(&mut self, opacity: f32) {
        self.state.opacity *= opacity;
    }

    fn save_state(&mut self) {
        self.state_stack.push(self.state.clone());
    }

    fn restore_state(&mut self) {
        if let Some(prev) = self.state_stack.pop() {
            self.state = prev;
            self.apply_clip();
        }
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    fn draw_cached_pixmap(
        &mut self,
        _item_cache: &ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        let mut data: Option<(u32, u32, Vec<u8>)> = None;
        update_fn(&mut |width, height, pixels| {
            data = Some((width, height, pixels.to_vec()));
        });

        if let Some((width, height, pixels)) = data {
            if width == 0 || height == 0 {
                return;
            }
            unsafe {
                let texture = SDL_CreateTexture(
                    self.renderer,
                    SDL_PIXELFORMAT_RGBA32,
                    SDL_TEXTUREACCESS_STATIC,
                    width as c_int,
                    height as c_int,
                );
                if texture.is_null() {
                    return;
                }
                SDL_SetTextureBlendMode(texture, SDL_BLENDMODE_BLEND);
                SDL_UpdateTexture(
                    texture,
                    std::ptr::null(),
                    pixels.as_ptr() as *const _,
                    (width * 4) as c_int,
                );
                let a = (255.0 * self.state.opacity) as u8;
                SDL_SetTextureAlphaMod(texture, a);

                let sf = self.scale_factor;
                let dst = SDL_FRect {
                    x: self.state.offset.x * sf,
                    y: self.state.offset.y * sf,
                    w: width as f32,
                    h: height as f32,
                };
                SDL_RenderTexture(self.renderer, texture, std::ptr::null(), &dst);
                SDL_DestroyTexture(texture);
            }
        }
    }

    fn draw_string(&mut self, string: &str, color: Color) {
        let request = FontRequest::default();
        let font = self.font_manager.font_for_request(&request, self.scale_factor, 0);
        self.render_text_to_renderer(string, font, color, 0.0, 0.0, None, (0.0, 0.0));
    }

    fn draw_image_direct(&mut self, image: Image) {
        let texture = self.create_texture_from_image(&image);
        if texture.is_null() {
            return;
        }

        let size = image.size();
        let a = (255.0 * self.state.opacity) as u8;
        unsafe {
            SDL_SetTextureAlphaMod(texture, a);
            SDL_SetTextureBlendMode(texture, SDL_BLENDMODE_BLEND);
        }
        let sf = self.scale_factor;
        let dst = SDL_FRect {
            x: self.state.offset.x * sf,
            y: self.state.offset.y * sf,
            w: size.width as f32,
            h: size.height as f32,
        };
        unsafe {
            SDL_RenderTexture(self.renderer, texture, std::ptr::null(), &dst);
            SDL_DestroyTexture(texture);
        }
    }

    fn window(&self) -> &WindowInner {
        self.window_inner
    }

    fn as_any(&mut self) -> Option<&mut dyn core::any::Any> {
        None
    }
}
