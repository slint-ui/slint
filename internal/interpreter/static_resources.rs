// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The pre-rendered textures and bitmap fonts of a design, in the form the software renderer
//! takes them.

use crate::prelude::*;
use i_slint_compiler::embedded_resources::{self as compiled, EmbeddedResourcesIdx};
use i_slint_core::graphics::{
    Image, ImageCacheKey, ImageInner, Rgba8Pixel, SharedImageBuffer, SharedPixelBuffer,
};
use typed_index_collections::TiVec;

/// A pre-rendered resource, in a form that can be sent to another thread.
#[derive(Clone)]
pub enum SendableResource {
    Texture(compiled::Texture),
    BitmapFont(compiled::BitmapFont),
}

#[derive(Clone)]
enum Resource {
    Texture(compiled::Texture, OnceCell<Image>),
    BitmapFont(Rc<compiled::BitmapFont>),
}

/// The pre-rendered resources that the compiled code refers to by index.
/// The other indexes refer to files, which the interpreter loads itself.
#[derive(Clone, Default)]
pub struct PreRendered(TiVec<EmbeddedResourcesIdx, Option<Resource>>);

impl PreRendered {
    pub fn from_document(document: &i_slint_compiler::object_tree::Document) -> Self {
        Self::from_sendable(document.embedded_file_resources.borrow().iter().map(|resource| {
            match &resource.kind {
                compiled::EmbeddedResourcesKind::TextureData(texture) => {
                    Some(SendableResource::Texture(texture.clone()))
                }
                compiled::EmbeddedResourcesKind::BitmapFontData(font) => {
                    Some(SendableResource::BitmapFont(font.clone()))
                }
                _ => None,
            }
        }))
    }

    pub fn from_sendable(resources: impl IntoIterator<Item = Option<SendableResource>>) -> Self {
        Self(
            resources
                .into_iter()
                .map(|resource| {
                    Some(match resource? {
                        SendableResource::Texture(texture) => {
                            Resource::Texture(texture, OnceCell::new())
                        }
                        SendableResource::BitmapFont(font) => Resource::BitmapFont(Rc::new(font)),
                    })
                })
                .collect(),
        )
    }

    pub fn into_sendable(self) -> Vec<Option<SendableResource>> {
        self.0
            .into_iter()
            .map(|resource| {
                Some(match resource? {
                    Resource::Texture(texture, _) => SendableResource::Texture(texture),
                    Resource::BitmapFont(font) => {
                        SendableResource::BitmapFont(Rc::unwrap_or_clone(font))
                    }
                })
            })
            .collect()
    }

    pub fn image(&self, id: EmbeddedResourcesIdx) -> Option<Image> {
        let Some(Resource::Texture(texture, image)) = self.0.get(id)? else { return None };
        Some(image.get_or_init(|| texture_image(texture)).clone())
    }

    pub fn bitmap_font(&self, id: EmbeddedResourcesIdx) -> Option<Rc<compiled::BitmapFont>> {
        let Some(Resource::BitmapFont(font)) = self.0.get(id)? else { return None };
        Some(font.clone())
    }
}

/// The image that `texture` is, as large as the whole image: the pixels outside the
/// texture's rectangle are transparent.
fn texture_image(texture: &compiled::Texture) -> Image {
    let rgba = |p: &[u8]| Rgba8Pixel::new(p[0], p[1], p[2], p[3]);
    let (width, height) = (texture.total_size.width, texture.total_size.height);
    let rect = texture.rect;
    let covers_image =
        rect.x() == 0 && rect.y() == 0 && rect.width() == width && rect.height() == height;
    let buffer = match texture.format {
        compiled::PixelFormat::Rgb if covers_image => SharedImageBuffer::RGB8(
            SharedPixelBuffer::clone_from_slice(&texture.data, width, height),
        ),
        compiled::PixelFormat::Rgb => {
            SharedImageBuffer::RGBA8Premultiplied(expand(texture, 3, |p| {
                Rgba8Pixel::new(p[0], p[1], p[2], 255)
            }))
        }
        compiled::PixelFormat::Rgba => SharedImageBuffer::RGBA8(expand(texture, 4, rgba)),
        compiled::PixelFormat::RgbaPremultiplied => {
            SharedImageBuffer::RGBA8Premultiplied(expand(texture, 4, rgba))
        }
        compiled::PixelFormat::AlphaMap([r, g, b]) => SharedImageBuffer::AlphaMap {
            alpha: expand(texture, 1, |p| p[0]),
            color: i_slint_core::Color::from_rgb_u8(r, g, b),
        },
    };
    Image::from(ImageInner::EmbeddedImage { cache_key: ImageCacheKey::Invalid, buffer })
}

/// The whole image of `texture`, whose pixels are `bpp` bytes, converted by `pixel`.
fn expand<Pixel: Clone + Default>(
    texture: &compiled::Texture,
    bpp: usize,
    pixel: impl Fn(&[u8]) -> Pixel,
) -> SharedPixelBuffer<Pixel> {
    let mut buffer = SharedPixelBuffer::new(texture.total_size.width, texture.total_size.height);
    let stride = buffer.width() as usize;
    let rect = texture.rect;
    let pixels = buffer.make_mut_slice();
    for (y, row) in texture.data.chunks_exact(rect.width() as usize * bpp).enumerate() {
        let start = (rect.y() as usize + y) * stride + rect.x() as usize;
        for (target, source) in pixels[start..].iter_mut().zip(row.chunks_exact(bpp)) {
            *target = pixel(source);
        }
    }
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "internal")]
    #[test]
    fn a_sent_design_keeps_its_fonts() {
        let mut compiler = crate::Compiler::default();
        compiler.set_embed_resources(i_slint_compiler::EmbedResourcesKind::EmbedTextures);
        let source = r#"export component Live inherits Window {
            Text { text: "Version 1"; font-size: 32px; }
        }"#;
        let result =
            spin_on::spin_on(compiler.build_from_source(source.into(), Default::default()));
        assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
        let result = crate::CompilationResult::from(result.into_send());
        let definition = result.components().next().unwrap();
        let pre_rendered = &definition.inner.resources.pre_rendered;
        let font = (0..pre_rendered.0.len())
            .find_map(|id| pre_rendered.bitmap_font(EmbeddedResourcesIdx(id)))
            .unwrap();

        let weak = Rc::downgrade(&font);
        drop(font);
        assert!(weak.upgrade().is_some());
        drop((definition, result));
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn a_texture_becomes_the_whole_image() {
        let texture = compiled::Texture {
            total_size: compiled::Size { width: 2, height: 1 },
            original_size: compiled::Size { width: 2, height: 1 },
            rect: compiled::Rect::from_ltrb(1, 0, 2, 1).unwrap(),
            data: vec![10, 20, 30, 255],
            format: compiled::PixelFormat::Rgba,
        };
        let pixels = texture_image(&texture).to_rgba8().unwrap();
        assert_eq!(
            pixels.as_slice(),
            [Rgba8Pixel::new(0, 0, 0, 0), Rgba8Pixel::new(10, 20, 30, 255)]
        );
    }
}
