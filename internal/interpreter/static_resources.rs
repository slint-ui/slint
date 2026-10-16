// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The pre-rendered textures and bitmap fonts of a compilation unit, in the form the
//! software renderer takes them.

use alloc::rc::Rc;
use i_slint_compiler::embedded_resources as unit;
use i_slint_core::graphics::{
    Image, ImageCacheKey, ImageInner, Rgba8Pixel, SharedBitmapFont, SharedBitmapGlyph,
    SharedBitmapGlyphs, SharedImageBuffer, SharedPixelBuffer,
};

/// The image that `texture` is, as large as the whole image: the pixels outside the
/// texture's rectangle are transparent.
pub fn texture_image(texture: &unit::Texture) -> Image {
    let (width, height) = (texture.total_size.width, texture.total_size.height);
    let rect = texture.rect;
    let covers_image =
        rect.x() == 0 && rect.y() == 0 && rect.width() == width && rect.height() == height;
    let buffer = match texture.format {
        unit::PixelFormat::Rgb if covers_image => SharedImageBuffer::RGB8(
            SharedPixelBuffer::clone_from_slice(&texture.data, width, height),
        ),
        unit::PixelFormat::Rgb => SharedImageBuffer::RGBA8Premultiplied(expand(texture, 3, |p| {
            Rgba8Pixel::new(p[0], p[1], p[2], 255)
        })),
        unit::PixelFormat::Rgba => SharedImageBuffer::RGBA8(expand(texture, 4, |p| {
            Rgba8Pixel::new(p[0], p[1], p[2], p[3])
        })),
        unit::PixelFormat::RgbaPremultiplied => {
            SharedImageBuffer::RGBA8Premultiplied(expand(texture, 4, |p| {
                Rgba8Pixel::new(p[0], p[1], p[2], p[3])
            }))
        }
        #[cfg(feature = "internal-image-pixel-format-alpha-map")]
        unit::PixelFormat::AlphaMap([r, g, b]) => SharedImageBuffer::AlphaMap {
            alpha: expand(texture, 1, |p| p[0]),
            color: i_slint_core::Color::from_rgb_u8(r, g, b),
        },
        #[cfg(not(feature = "internal-image-pixel-format-alpha-map"))]
        unit::PixelFormat::AlphaMap([r, g, b]) => {
            SharedImageBuffer::RGBA8(expand(texture, 1, |p| Rgba8Pixel::new(r, g, b, p[0])))
        }
    };
    Image::from(ImageInner::EmbeddedImage { cache_key: ImageCacheKey::Invalid, buffer })
}

/// The whole image of `texture`, whose pixels are `bpp` bytes, converted by `pixel`.
fn expand<Pixel: Clone + Default>(
    texture: &unit::Texture,
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

/// The font as the renderer takes it.
pub fn shared_font(font: &unit::BitmapFont) -> Rc<SharedBitmapFont> {
    Rc::new(SharedBitmapFont {
        family_name: font.family_name.as_str().into(),
        character_map: font.character_map.clone(),
        units_per_em: font.units_per_em,
        ascent: font.ascent,
        descent: font.descent,
        x_height: font.x_height,
        cap_height: font.cap_height,
        glyphs: font
            .glyphs
            .iter()
            .map(|glyphs| SharedBitmapGlyphs {
                pixel_size: glyphs.pixel_size,
                glyph_data: glyphs
                    .glyph_data
                    .iter()
                    .map(|glyph| SharedBitmapGlyph {
                        x: glyph.x,
                        y: glyph.y,
                        width: glyph.width,
                        height: glyph.height,
                        x_advance: glyph.x_advance,
                        data: glyph.data.as_slice().into(),
                    })
                    .collect(),
            })
            .collect(),
        weight: font.weight,
        italic: font.italic,
        sdf: font.sdf,
    })
}

#[cfg(all(test, feature = "internal", feature = "std"))]
fn compile_embedding_textures(source: &str) -> i_slint_compiler::llr::CompilationUnit {
    let mut compiler = crate::Compiler::default();
    compiler.set_embed_resources(crate::EmbedResourcesKind::EmbedTextures);
    let result = spin_on::spin_on(compiler.build_from_source(source.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<alloc::vec::Vec<_>>());
    result.into_compilation_unit(i_slint_core::InternalToken).unwrap()
}

#[cfg(all(test, feature = "internal", feature = "std"))]
const LIVE: &str = r#"export component Live inherits Window {
    Text { text: "Version 1"; font-size: 32px; }
}"#;

#[cfg(all(test, feature = "internal", feature = "std"))]
fn fonts_of(unit: &i_slint_compiler::llr::CompilationUnit) -> alloc::vec::Vec<Rc<SharedBitmapFont>> {
    unit.embedded_resources
        .iter()
        .filter_map(|resource| match resource {
            unit::EmbeddedResource::BitmapFont(font) => Some(shared_font(font)),
            _ => None,
        })
        .collect()
}

#[cfg(all(test, feature = "internal", feature = "std"))]
#[test]
fn unused_resources_are_freed() {
    let unit = compile_embedding_textures(LIVE);
    let font = Rc::downgrade(&fonts_of(&unit)[0]);
    assert_eq!(font.strong_count(), 0);
}

#[cfg(all(test, feature = "internal", feature = "std"))]
#[test]
fn a_texture_becomes_the_whole_image() {
    let texture = unit::Texture {
        total_size: unit::Size { width: 2, height: 1 },
        original_size: unit::Size { width: 2, height: 1 },
        rect: unit::Rect::from_ltrb(1, 0, 2, 1).unwrap(),
        data: vec![10, 20, 30, 255],
        format: unit::PixelFormat::Rgba,
    };
    let pixels = texture_image(&texture).to_rgba8().unwrap();
    assert_eq!(pixels.as_slice(), [Rgba8Pixel::new(0, 0, 0, 0), Rgba8Pixel::new(10, 20, 30, 255)]);
}
