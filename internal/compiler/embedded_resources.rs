// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use alloc::string::String;
use alloc::vec::Vec;

#[cfg(feature = "renderer-software")]
use i_slint_common::bitmapfont::OwnedStorage;
#[cfg(feature = "renderer-software")]
pub type Texture = i_slint_common::pre_rendered::StaticTextures<OwnedStorage>;
#[cfg(feature = "renderer-software")]
pub type BitmapGlyph = i_slint_common::bitmapfont::BitmapGlyph<OwnedStorage>;
#[cfg(feature = "renderer-software")]
pub type BitmapGlyphs = i_slint_common::bitmapfont::BitmapGlyphs<OwnedStorage>;
#[cfg(feature = "renderer-software")]
pub type BitmapFont = i_slint_common::bitmapfont::BitmapFont<OwnedStorage>;

/// The resources of `document` that the compiler pre-rendered, by [`EmbeddedResourcesIdx`].
#[cfg(feature = "std")]
pub fn pre_rendered_resources(
    document: &crate::object_tree::Document,
) -> Vec<Option<i_slint_common::pre_rendered::PreRenderedResource>> {
    let resources = document.embedded_file_resources.borrow();
    resources
        .iter()
        .map(|resource| match &resource.kind {
            #[cfg(feature = "renderer-software")]
            EmbeddedResourcesKind::TextureData(texture) => {
                Some(i_slint_common::pre_rendered::PreRenderedResource::Texture(texture.clone()))
            }
            #[cfg(feature = "renderer-software")]
            EmbeddedResourcesKind::BitmapFontData(font) => {
                Some(i_slint_common::pre_rendered::PreRenderedResource::BitmapFont(font.clone()))
            }
            _ => None,
        })
        .collect()
}

#[derive(Debug, Clone)]
pub enum EmbeddedResourcesKind {
    /// Only List the resource, do not actually embed it
    ListOnly,
    /// Just put the file content as a resource
    FileData,
    /// Encoded payload from a data URI (bytes, extension)
    DataUriPayload(Vec<u8>, String),
    /// The data has been processed in a texture
    #[cfg(feature = "renderer-software")]
    TextureData(Texture),
    /// A set of pre-rendered glyphs of a TrueType font
    #[cfg(feature = "renderer-software")]
    BitmapFontData(BitmapFont),
    /// The image of a Slint SC `@image-url()`, decoded at compile time. The
    /// Slint SC generator embeds its pixels in the generated code.
    #[cfg(feature = "slint-sc")]
    StaticPixels(image::RgbaImage),
}

#[derive(Debug, Clone)]
pub struct EmbeddedResources {
    /// `None` for in-memory payloads such as data URIs.
    pub path: Option<crate::source_path::SourcePath>,

    pub kind: EmbeddedResourcesKind,
}

/// Index of an [`EmbeddedResources`] entry in [`crate::object_tree::Document::embedded_file_resources`].
#[derive(
    Debug,
    Clone,
    Copy,
    Hash,
    PartialEq,
    Eq,
    derive_more::Into,
    derive_more::From,
    derive_more::Display,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EmbeddedResourcesIdx(pub usize);
