// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The pre-rendered textures and bitmap fonts of a design, in the form the software renderer
//! takes them.

use crate::prelude::*;
use i_slint_compiler::embedded_resources::{self as compiled, EmbeddedResourcesIdx};
use i_slint_core::graphics::{Image, ImageInner};
use typed_index_collections::TiVec;

/// A pre-rendered resource, in a form that can be sent to another thread.
#[derive(Clone)]
pub enum SendableResource {
    Texture(compiled::Texture),
    BitmapFont(compiled::BitmapFont),
}

#[derive(Clone)]
enum Resource {
    Texture(Rc<compiled::Texture>),
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
                        SendableResource::Texture(texture) => Resource::Texture(Rc::new(texture)),
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
                    Resource::Texture(texture) => {
                        SendableResource::Texture(Rc::unwrap_or_clone(texture))
                    }
                    Resource::BitmapFont(font) => {
                        SendableResource::BitmapFont(Rc::unwrap_or_clone(font))
                    }
                })
            })
            .collect()
    }

    pub fn image(&self, id: EmbeddedResourcesIdx) -> Option<Image> {
        let Some(Resource::Texture(texture)) = self.0.get(id)? else { return None };
        Some(Image::from(ImageInner::OwnedTextures(texture.clone())))
    }

    pub fn bitmap_font(&self, id: EmbeddedResourcesIdx) -> Option<Rc<compiled::BitmapFont>> {
        let Some(Resource::BitmapFont(font)) = self.0.get(id)? else { return None };
        Some(font.clone())
    }
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
        use i_slint_common::pre_rendered::{StaticTexture, TexturePixelFormat};
        use i_slint_core::graphics::{IntRect, IntSize, Rgba8Pixel};
        let texture = compiled::Texture {
            size: IntSize::new(2, 1),
            original_size: IntSize::new(2, 1),
            data: vec![10, 20, 30, 255],
            textures: vec![StaticTexture {
                rect: IntRect::new((1, 0).into(), (1, 1).into()),
                format: TexturePixelFormat::Rgba,
                color: 0,
                index: 0,
            }],
        };
        let pixels = PreRendered::from_sendable([Some(SendableResource::Texture(texture))])
            .image(EmbeddedResourcesIdx(0))
            .unwrap()
            .to_rgba8()
            .unwrap();
        assert_eq!(
            pixels.as_slice(),
            [Rgba8Pixel::new(0, 0, 0, 0), Rgba8Pixel::new(10, 20, 30, 255)]
        );
    }
}
