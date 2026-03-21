// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Font management for the SDL backend using SDL_ttf.
//!
//! Handles font loading, caching, and text measurement via the SDL_ttf 3.x API.

use crate::sdl3_bindings::*;
use i_slint_core::graphics::FontRequest;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_int;

/// Default font size in logical pixels when none is specified.
const DEFAULT_FONT_SIZE: f32 = 16.0;

/// A key for looking up cached fonts.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct FontCacheKey {
    family: String,
    pixel_size_tenths: i32, // pixel_size * 10, to avoid float hashing
    weight: i32,
    italic: bool,
    outline: i32,
}

impl FontCacheKey {
    fn from_request(request: &FontRequest, scale_factor: f32, outline: i32) -> Self {
        let pixel_size = request.pixel_size.map_or(DEFAULT_FONT_SIZE, |s| s.get()) * scale_factor;
        Self {
            family: request.family.as_ref().map_or_else(String::new, |f| f.to_string()),
            pixel_size_tenths: (pixel_size * 10.0) as i32,
            weight: request.weight.unwrap_or(400),
            italic: request.italic,
            outline,
        }
    }
}

/// Manages font loading and caching for the SDL backend.
pub(crate) struct FontManager {
    /// Cached fonts keyed by (family, size, weight, italic, outline).
    cache: RefCell<HashMap<FontCacheKey, *mut TTF_Font>>,
    /// Fonts registered from memory (kept alive for the font's lifetime).
    registered_fonts: RefCell<Vec<(String, Vec<u8>)>>,
    /// Fonts registered from file paths.
    registered_font_paths: RefCell<Vec<(String, String)>>,
    /// Fallback font path (system default).
    default_font_path: Option<String>,
}

impl FontManager {
    pub fn new() -> Self {
        let default_font_path = find_default_font();

        Self {
            cache: RefCell::new(HashMap::new()),
            registered_fonts: RefCell::new(Vec::new()),
            registered_font_paths: RefCell::new(Vec::new()),
            default_font_path,
        }
    }

    /// Get or open a TTF_Font for the given request and outline. The returned
    /// pointer is valid as long as the FontManager is alive. Returns null if no
    /// font could be loaded.
    ///
    /// When `outline > 0` and a non-outlined version of the same font is already
    /// cached, `TTF_CopyFont` is used to create the outlined variant. This shares
    /// the underlying FreeType face data and avoids loading from disk again, while
    /// keeping a separate glyph cache so that toggling the outline doesn't flush
    /// the base font's cache.
    pub fn font_for_request(
        &self,
        request: &FontRequest,
        scale_factor: f32,
        outline: i32,
    ) -> *mut TTF_Font {
        let key = FontCacheKey::from_request(request, scale_factor, outline);

        if let Some(&font) = self.cache.borrow().get(&key) {
            return font;
        }

        // For outlined variants, try to copy from the cached base font
        if outline > 0 {
            let base_key = FontCacheKey::from_request(request, scale_factor, 0);
            let base = self.cache.borrow().get(&base_key).copied();
            if let Some(base) = base {
                let font = unsafe { TTF_CopyFont(base) };
                if !font.is_null() {
                    unsafe { TTF_SetFontOutline(font, outline) };
                    self.cache.borrow_mut().insert(key, font);
                    return font;
                }
            }
        }

        let pixel_size = request.pixel_size.map_or(DEFAULT_FONT_SIZE, |s| s.get()) * scale_factor;
        let family = request.family.as_ref().map_or("", |f| f.as_str());

        let font =
            self.load_font(family, pixel_size, request.weight.unwrap_or(400), request.italic);
        if !font.is_null() {
            if outline > 0 {
                unsafe { TTF_SetFontOutline(font, outline) };
            }
            self.cache.borrow_mut().insert(key, font);
        }
        font
    }

    fn load_font(&self, family: &str, pixel_size: f32, weight: i32, italic: bool) -> *mut TTF_Font {
        // Try registered fonts from memory first
        for (name, data) in self.registered_fonts.borrow().iter() {
            if family.is_empty() || name.eq_ignore_ascii_case(family) {
                let font = self.open_font_from_memory(data, pixel_size);
                if !font.is_null() {
                    apply_font_style(font, weight, italic);
                    return font;
                }
            }
        }

        // Try registered font paths
        for (name, path) in self.registered_font_paths.borrow().iter() {
            if family.is_empty() || name.eq_ignore_ascii_case(family) {
                let font = self.open_font_from_path(path, pixel_size);
                if !font.is_null() {
                    apply_font_style(font, weight, italic);
                    return font;
                }
            }
        }

        // Try system font paths
        if let Some(path) = find_system_font(family) {
            let font = self.open_font_from_path(&path, pixel_size);
            if !font.is_null() {
                apply_font_style(font, weight, italic);
                return font;
            }
        }

        // Fallback to default font
        if let Some(ref path) = self.default_font_path {
            let font = self.open_font_from_path(path, pixel_size);
            if !font.is_null() {
                apply_font_style(font, weight, italic);
                return font;
            }
        }

        std::ptr::null_mut()
    }

    fn open_font_from_path(&self, path: &str, pixel_size: f32) -> *mut TTF_Font {
        let c_path = match CString::new(path) {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };
        unsafe { TTF_OpenFont(c_path.as_ptr(), pixel_size) }
    }

    fn open_font_from_memory(&self, data: &[u8], pixel_size: f32) -> *mut TTF_Font {
        unsafe {
            let io = SDL_IOFromConstMem(data.as_ptr() as *const _, data.len());
            if io.is_null() {
                return std::ptr::null_mut();
            }
            TTF_OpenFontIO(io, true, pixel_size)
        }
    }

    pub fn register_font_from_memory(&self, family_name: String, data: Vec<u8>) {
        self.registered_fonts.borrow_mut().push((family_name, data));
        // Clear cache so newly registered fonts are picked up
        self.cache.borrow_mut().clear();
    }

    pub fn register_font_from_path(&self, family_name: String, path: String) {
        self.registered_font_paths.borrow_mut().push((family_name, path));
        self.cache.borrow_mut().clear();
    }

    /// Measure text size in physical pixels.
    pub fn text_size(&self, font: *mut TTF_Font, text: &str, max_width: Option<f32>) -> (f32, f32) {
        if font.is_null() || text.is_empty() {
            let h = if font.is_null() {
                DEFAULT_FONT_SIZE
            } else {
                unsafe { TTF_GetFontHeight(font) as f32 }
            };
            return (0.0, h);
        }

        let _c_text = match CString::new(text) {
            Ok(s) => s,
            Err(_) => {
                // Text contains null bytes; measure up to first null
                let truncated = text.split('\0').next().unwrap_or("");
                match CString::new(truncated) {
                    Ok(s) => s,
                    Err(_) => return (0.0, 0.0),
                }
            }
        };

        if let Some(max_w) = max_width {
            // Measure with wrapping: use line-by-line approach
            return self.measure_wrapped_text(font, text, max_w);
        }

        // Measure each line separately and return the max width and total height
        let mut max_w: f32 = 0.0;
        let mut total_h: f32 = 0.0;
        let line_skip = unsafe { TTF_GetFontLineSkip(font) } as f32;

        for (i, line) in text.split('\n').enumerate() {
            if i > 0 {
                total_h += line_skip;
            }
            if line.is_empty() {
                if i == 0 {
                    total_h += unsafe { TTF_GetFontHeight(font) } as f32;
                }
                continue;
            }
            let c_line = match CString::new(line) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let mut w: c_int = 0;
            let mut h: c_int = 0;
            unsafe {
                TTF_GetStringSize(font, c_line.as_ptr(), line.len(), &mut w, &mut h);
            }
            max_w = max_w.max(w as f32);
            if i == 0 {
                total_h += h as f32;
            }
        }

        (max_w, total_h)
    }

    /// Measure wrapped text by simulating word wrapping.
    fn measure_wrapped_text(&self, font: *mut TTF_Font, text: &str, max_width: f32) -> (f32, f32) {
        let line_skip = unsafe { TTF_GetFontLineSkip(font) } as f32;
        let font_height = unsafe { TTF_GetFontHeight(font) } as f32;
        let mut total_height = 0.0f32;
        let mut max_line_width = 0.0f32;

        for paragraph in text.split('\n') {
            if paragraph.is_empty() {
                total_height += line_skip;
                continue;
            }

            let _c_text = match CString::new(paragraph) {
                Ok(s) => s,
                Err(_) => {
                    total_height += line_skip;
                    continue;
                }
            };

            let mut remaining = paragraph;
            let mut first_line = true;
            while !remaining.is_empty() {
                let c_remaining = match CString::new(remaining) {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let mut extent: c_int = 0;
                let mut count: usize = 0;
                unsafe {
                    TTF_MeasureString(
                        font,
                        c_remaining.as_ptr(),
                        remaining.len(),
                        max_width as c_int,
                        &mut extent,
                        &mut count,
                    );
                }
                if count == 0 {
                    // At least one character per line to avoid infinite loop
                    count = remaining.char_indices().nth(1).map_or(remaining.len(), |(i, _)| i);
                }
                max_line_width = max_line_width.max(extent as f32);
                if first_line {
                    total_height += font_height;
                    first_line = false;
                } else {
                    total_height += line_skip;
                }
                remaining = &remaining[count..];
                // Skip leading whitespace on next line
                remaining = remaining.trim_start();
            }
            if first_line {
                // Empty paragraph handled above, but just in case
                total_height += font_height;
            }
        }

        if total_height == 0.0 {
            total_height = font_height;
        }

        (max_line_width, total_height)
    }

    /// Get font metrics in physical pixels.
    pub fn font_metrics(&self, font: *mut TTF_Font) -> (f32, f32, f32, f32) {
        if font.is_null() {
            return (DEFAULT_FONT_SIZE * 0.8, -DEFAULT_FONT_SIZE * 0.2, 0.0, 0.0);
        }
        unsafe {
            let ascent = TTF_GetFontAscent(font) as f32;
            let descent = TTF_GetFontDescent(font) as f32;
            // SDL_ttf doesn't provide x_height and cap_height directly.
            // Approximate: x_height ≈ 0.53 * ascent, cap_height ≈ 0.71 * (ascent - descent)
            let x_height = ascent * 0.53;
            let cap_height = (ascent - descent) * 0.71;
            (ascent, descent, x_height, cap_height)
        }
    }

    /// Find the byte offset in `text` closest to physical pixel position `x` on a single line.
    pub fn byte_offset_for_x(&self, font: *mut TTF_Font, text: &str, x: f32) -> usize {
        if font.is_null() || text.is_empty() || x <= 0.0 {
            return 0;
        }

        // Binary search through character positions
        let mut best_offset = 0;
        let mut best_distance = x.abs();

        for (byte_idx, _) in text.char_indices() {
            let prefix = &text[..byte_idx];
            let c_prefix = match CString::new(prefix) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let mut w: c_int = 0;
            let mut h: c_int = 0;
            unsafe {
                TTF_GetStringSize(font, c_prefix.as_ptr(), prefix.len(), &mut w, &mut h);
            }
            let distance = (w as f32 - x).abs();
            if distance < best_distance {
                best_distance = distance;
                best_offset = byte_idx;
            }
        }

        // Also check the full string
        let c_text = match CString::new(text) {
            Ok(s) => s,
            Err(_) => return best_offset,
        };
        let mut w: c_int = 0;
        let mut h: c_int = 0;
        unsafe {
            TTF_GetStringSize(font, c_text.as_ptr(), text.len(), &mut w, &mut h);
        }
        let distance = (w as f32 - x).abs();
        if distance < best_distance {
            best_offset = text.len();
        }

        best_offset
    }

    /// Get the x-position in physical pixels for a given byte offset in single-line text.
    pub fn x_for_byte_offset(&self, font: *mut TTF_Font, text: &str, byte_offset: usize) -> f32 {
        if font.is_null() || text.is_empty() || byte_offset == 0 {
            return 0.0;
        }

        let prefix = &text[..byte_offset.min(text.len())];
        let c_prefix = match CString::new(prefix) {
            Ok(s) => s,
            Err(_) => return 0.0,
        };
        let mut w: c_int = 0;
        let mut h: c_int = 0;
        unsafe {
            TTF_GetStringSize(font, c_prefix.as_ptr(), prefix.len(), &mut w, &mut h);
        }
        w as f32
    }
}

impl Drop for FontManager {
    fn drop(&mut self) {
        // Close all cached fonts
        for (_, font) in self.cache.borrow().iter() {
            if !font.is_null() {
                unsafe { TTF_CloseFont(*font) };
            }
        }
    }
}

fn apply_font_style(font: *mut TTF_Font, weight: i32, italic: bool) {
    let mut style = TTF_STYLE_NORMAL;
    if weight >= 700 {
        style |= TTF_STYLE_BOLD;
    }
    if italic {
        style |= TTF_STYLE_ITALIC;
    }
    if style != TTF_STYLE_NORMAL {
        unsafe { TTF_SetFontStyle(font, style) };
    }
}

/// Try to find a system default font. Returns a path to a TTF file.
fn find_default_font() -> Option<String> {
    fc_match("sans")
}

/// Try to find a system font matching the given family name.
fn find_system_font(family: &str) -> Option<String> {
    if family.is_empty() {
        return None;
    }
    fc_match(family)
}

/// Use fontconfig's `fc-match` to resolve a font pattern to a file path.
fn fc_match(pattern: &str) -> Option<String> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pattern;
        return None;
    }

    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("fc-match")
            .args(["--format=%{file}", pattern])
            .output()
            .ok()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() && std::path::Path::new(&path).exists() {
                return Some(path);
            }
        }
        None
    }
}
