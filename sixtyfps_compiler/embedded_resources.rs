/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

#[cfg(not(target_arch = "wasm32"))]
pub use tiny_skia::IntRect as Rect;

#[derive(Debug, Clone, Copy, Default)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug)]
pub enum PixelFormat {
    // 24 bit RGB
    Rgb,
    // 32 bit RGBA
    Rgba,
    // 8bit alpha map with a given color
    AlphaMap([u8; 3]),
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct Texture {
    pub total_size: Size,
    pub rect: Rect,
    pub data: Vec<u8>,
    pub format: PixelFormat,
}

#[cfg(not(target_arch = "wasm32"))]
impl Texture {
    pub fn new_empty() -> Self {
        Self {
            total_size: Size::default(),
            rect: Rect::from_xywh(0, 0, 1, 1).unwrap(),
            data: vec![0, 0, 0, 0],
            format: PixelFormat::Rgba,
        }
    }
}

#[derive(Debug, Clone)]
pub enum EmbeddedResourcesKind {
    /// Just put the file content as a resource
    RawData,
    /// The data has been processed in a texture
    TextureData(#[cfg(not(target_arch = "wasm32"))] Texture),
}

#[derive(Debug, Clone)]
pub struct EmbeddedResources {
    /// unique integer id, that can be used by the generator for symbol generation.
    pub id: usize,

    pub kind: EmbeddedResourcesKind,
}
