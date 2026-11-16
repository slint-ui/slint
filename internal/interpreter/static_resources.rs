// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The pre-rendered textures and bitmap fonts of a design, in the form the software renderer
//! takes them.

use crate::prelude::*;
use i_slint_common::bitmapfont::{BitmapFont, OwnedStorage};
use i_slint_common::pre_rendered::{PreRenderedResource, StaticTextures};
use i_slint_compiler::embedded_resources::EmbeddedResourcesIdx;
use i_slint_core::graphics::{Image, ImageInner};
use typed_index_collections::TiVec;

#[derive(Clone)]
enum Resource {
    Texture(Rc<StaticTextures<OwnedStorage>>),
    BitmapFont(Rc<BitmapFont<OwnedStorage>>),
}

impl From<PreRenderedResource> for Resource {
    fn from(resource: PreRenderedResource) -> Self {
        match resource {
            PreRenderedResource::Texture(texture) => Self::Texture(Rc::new(texture)),
            PreRenderedResource::BitmapFont(font) => Self::BitmapFont(Rc::new(font)),
        }
    }
}

/// This is an internal type without API stability guarantees.
/// A pre-rendered resource of a compilation unit, in the form the renderer takes it,
/// for [`crate::CompilationResult::from_compilation_unit()`].
#[doc(hidden)]
#[derive(Clone)]
pub struct LoadedResource(Option<Resource>);

impl LoadedResource {
    /// Converts `resource`, such as one that `i_slint_compiler::llr::serialize` read.
    /// `None` stands for a resource that the compiler didn't pre-render.
    pub fn new(resource: Option<PreRenderedResource>, _: i_slint_core::InternalToken) -> Self {
        Self(resource.map(Resource::from))
    }
}

/// The pre-rendered resources that the compiled code refers to by index.
/// The other indexes refer to files, which the interpreter loads itself.
#[derive(Clone, Default)]
pub struct PreRendered(TiVec<EmbeddedResourcesIdx, Option<Resource>>);

impl PreRendered {
    #[cfg(feature = "std")]
    pub fn from_document(document: &i_slint_compiler::object_tree::Document) -> Self {
        Self::from_sendable(i_slint_compiler::embedded_resources::pre_rendered_resources(document))
    }

    pub fn from_sendable(resources: impl IntoIterator<Item = Option<PreRenderedResource>>) -> Self {
        Self(resources.into_iter().map(|resource| resource.map(Resource::from)).collect())
    }

    pub fn from_loaded(resources: impl IntoIterator<Item = LoadedResource>) -> Self {
        Self(resources.into_iter().map(|resource| resource.0).collect())
    }

    pub fn into_sendable(self) -> Vec<Option<PreRenderedResource>> {
        self.0
            .into_iter()
            .map(|resource| match resource? {
                Resource::Texture(texture) => {
                    Some(PreRenderedResource::Texture(Rc::unwrap_or_clone(texture)))
                }
                Resource::BitmapFont(font) => {
                    Some(PreRenderedResource::BitmapFont(Rc::unwrap_or_clone(font)))
                }
            })
            .collect()
    }

    pub fn image(&self, id: EmbeddedResourcesIdx) -> Option<Image> {
        let Some(Resource::Texture(texture)) = self.0.get(id)? else { return None };
        Some(Image::from(ImageInner::OwnedTextures(texture.clone())))
    }

    pub fn bitmap_font(&self, id: EmbeddedResourcesIdx) -> Option<Rc<BitmapFont<OwnedStorage>>> {
        let Some(Resource::BitmapFont(font)) = self.0.get(id)? else { return None };
        Some(font.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(feature = "internal", feature = "internal-embed-textures", feature = "std"))]
    #[test]
    fn a_sent_or_loaded_design_keeps_its_fonts() {
        use i_slint_core::InternalToken;
        let mut compiler = crate::Compiler::default();
        compiler.set_embed_resources(i_slint_compiler::EmbedResourcesKind::EmbedTextures);
        let source = r#"export component Live inherits Window {
            Text { text: "Version 1"; font-size: 32px; }
        }"#;
        let result =
            spin_on::spin_on(compiler.build_from_source(source.into(), Default::default()));
        assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
        let loaded = {
            let sent = result.clone().into_send();
            let resources = sent
                .pre_rendered
                .into_iter()
                .map(|resource| LoadedResource::new(resource, InternalToken));
            let unit = sent.compilation_unit.unwrap();
            crate::CompilationResult::from_compilation_unit(unit, resources, InternalToken)
        };
        let sent = crate::CompilationResult::from(result.into_send());

        for result in [sent, loaded] {
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
    }

    #[test]
    fn a_texture_becomes_the_whole_image() {
        use i_slint_common::pre_rendered::{StaticTexture, TexturePixelFormat};
        use i_slint_core::graphics::{IntRect, IntSize, Rgba8Pixel};
        let texture = StaticTextures::<OwnedStorage> {
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
        let pixels = PreRendered::from_sendable([Some(PreRenderedResource::Texture(texture))])
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
