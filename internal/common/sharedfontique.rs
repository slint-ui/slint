// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub use fontique;
pub use ttf_parser;

use std::collections::HashMap;
use std::sync::Arc;

pub static COLLECTION: std::sync::LazyLock<Collection> = std::sync::LazyLock::new(|| {
    let mut collection = fontique::Collection::new(fontique::CollectionOptions {
        shared: true,
        ..Default::default()
    });

    let mut source_cache = fontique::SourceCache::new_shared();

    let mut default_fonts: HashMap<std::path::PathBuf, fontique::QueryFont> = Default::default();

    #[cfg(any(target_family = "wasm", target_os = "nto"))]
    {
        let data = include_bytes!("sharedfontique/DejaVuSans.ttf");
        let fonts = collection.register_fonts(fontique::Blob::new(Arc::new(data)), None);
        for script in fontique::Script::all_samples().iter().map(|(script, _)| *script) {
            collection.append_fallbacks(
                fontique::FallbackKey::new(script, None),
                fonts.iter().map(|(family_id, _)| *family_id),
            );
        }
        for generic_family in [
            fontique::GenericFamily::SansSerif,
            fontique::GenericFamily::SystemUi,
            fontique::GenericFamily::UiSansSerif,
        ] {
            collection.append_generic_families(
                generic_family,
                fonts.iter().map(|(family_id, _)| *family_id),
            );
        }
    }

    let mut add_font_from_path = |path: std::path::PathBuf| {
        if let Ok(bytes) = std::fs::read(&path) {
            // just use the first font of the first family in the file.
            if let Some(font) =
                collection.register_fonts(bytes.into(), None).first().and_then(|(id, infos)| {
                    let info = infos.first()?;
                    get_font_for_info(&mut collection, &mut source_cache, *id, &info)
                })
            {
                default_fonts.insert(path, font);
            }
        }
    };

    if let Some(path) = std::env::var_os("SLINT_DEFAULT_FONT") {
        let path = std::path::Path::new(&path);
        if path.extension().is_some() {
            add_font_from_path(path.to_owned());
        } else {
            if let Ok(dir) = std::fs::read_dir(path) {
                for file in dir {
                    if let Ok(file) = file {
                        add_font_from_path(file.path());
                    }
                }
            }
        }
    }

    Collection { inner: collection, source_cache, default_fonts: Arc::new(default_fonts) }
});

pub fn get_collection() -> Collection {
    COLLECTION.clone()
}

#[derive(Clone)]
pub struct Collection {
    pub inner: fontique::Collection,
    pub source_cache: fontique::SourceCache,
    pub default_fonts: Arc<HashMap<std::path::PathBuf, fontique::QueryFont>>,
}

impl Collection {
    pub fn query<'a>(&'a mut self) -> fontique::Query<'a> {
        self.inner.query(&mut self.source_cache)
    }

    pub fn get_font_for_info(
        &mut self,
        family_id: fontique::FamilyId,
        info: &fontique::FontInfo,
    ) -> Option<fontique::QueryFont> {
        get_font_for_info(&mut self.inner, &mut self.source_cache, family_id, info)
    }
}

fn get_font_for_info(
    collection: &mut fontique::Collection,
    source_cache: &mut fontique::SourceCache,
    family_id: fontique::FamilyId,
    info: &fontique::FontInfo,
) -> Option<fontique::QueryFont> {
    let mut query = collection.query(source_cache);
    query.set_families(std::iter::once(fontique::QueryFamily::from(family_id)));
    query.set_attributes(fontique::Attributes {
        weight: info.weight(),
        style: info.style(),
        width: info.width(),
    });
    let mut font = None;
    query.matches_with(|queried_font| {
        font = Some(queried_font.clone());
        fontique::QueryStatus::Stop
    });
    font
}

impl std::ops::Deref for Collection {
    type Target = fontique::Collection;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for Collection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Font metrics in design space. Scale with desired pixel size and divided by units_per_em
/// to obtain pixel metrics.
#[derive(Clone)]
pub struct DesignFontMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub x_height: f32,
    pub cap_height: f32,
    pub units_per_em: f32,
}

impl DesignFontMetrics {
    pub fn new(font: &fontique::QueryFont) -> Self {
        let face = ttf_parser::Face::parse(font.blob.data(), font.index).unwrap();
        Self::new_from_face(&face)
    }

    pub fn new_from_face(face: &ttf_parser::Face) -> Self {
        Self {
            ascent: face.ascender() as f32,
            descent: face.descender() as f32,
            x_height: face.x_height().unwrap_or_default() as f32,
            cap_height: face.capital_height().unwrap_or_default() as f32,
            units_per_em: face.units_per_em() as f32,
        }
    }
}
