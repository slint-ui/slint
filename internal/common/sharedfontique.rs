// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub use fontique;
pub use skrifa;

#[cfg(feature = "svg-text")]
pub mod svg;

#[cfg(any(target_family = "wasm", target_os = "nto"))]
use fontique::ScriptExt;

use std::collections::HashSet;
use std::sync::Arc;

/// Create a new fontique Collection.
/// When `shared` is true, the collection uses `Arc`-based internal sharing,
/// so that clones share the underlying data and mutations are visible across clones.
pub fn create_collection(shared: bool) -> Collection {
    let mut collection =
        fontique::Collection::new(fontique::CollectionOptions { shared, system_fonts: true });
    let mut source_cache =
        if shared { fontique::SourceCache::new_shared() } else { fontique::SourceCache::default() };

    // Preserves insertion order — the primary (SLINT_DEFAULT_FONT) lands first, fallbacks
    // (SLINT_FONT_PATH) follow. The runtime bitmap-font fallback and the compile-time
    // bitmap-font emission both rely on this ordering rather than any later sort.
    let mut default_fonts: Vec<(std::path::PathBuf, fontique::QueryFont)> = Vec::new();
    let mut chain_families: Vec<fontique::FamilyId> = Vec::new();

    #[cfg(any(target_family = "wasm", target_os = "nto"))]
    {
        let data = include_bytes!("sharedfontique/Inter-VariableFont.ttf");
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

    let mut registered_paths: HashSet<std::path::PathBuf> = HashSet::new();
    let mut register_path =
        |path: std::path::PathBuf,
         collection: &mut fontique::Collection,
         source_cache: &mut fontique::SourceCache,
         default_fonts: &mut Vec<_>,
         chain_families: &mut Vec<fontique::FamilyId>| {
            if !registered_paths.insert(path.clone()) {
                return;
            }
            let Ok(bytes) = std::fs::read(&path) else { return };
            let fonts = collection.register_fonts(bytes.into(), None);
            if fonts.is_empty() {
                return;
            }
            for (family_id, _) in &fonts {
                if !chain_families.contains(family_id) {
                    chain_families.push(*family_id);
                }
            }
            if let Some(font) = fonts.first().and_then(|(id, infos)| {
                let info = infos.first()?;
                get_font_for_info(collection, source_cache, *id, info)
            }) {
                default_fonts.push((path, font));
            }
        };

    // SLINT_DEFAULT_FONT: a single .ttf to act as the primary font.
    if let Some(path) = std::env::var_os("SLINT_DEFAULT_FONT") {
        register_path(
            path.into(),
            &mut collection,
            &mut source_cache,
            &mut default_fonts,
            &mut chain_families,
        );
    }

    // SLINT_FONT_PATH: OS-PATH-style list of additional fonts. Entries may be `.ttf`
    // files or directories (scanned non-recursively); everything found is appended to
    // the fallback chain after the primary.
    if let Some(path_list) = std::env::var_os("SLINT_FONT_PATH") {
        for entry in std::env::split_paths(&path_list) {
            if entry.is_file() {
                register_path(
                    entry,
                    &mut collection,
                    &mut source_cache,
                    &mut default_fonts,
                    &mut chain_families,
                );
            } else if let Ok(dir) = std::fs::read_dir(&entry) {
                for file in dir.flatten() {
                    register_path(
                        file.path(),
                        &mut collection,
                        &mut source_cache,
                        &mut default_fonts,
                        &mut chain_families,
                    );
                }
            }
        }
    }

    if !chain_families.is_empty() {
        for generic_family in [
            fontique::GenericFamily::SansSerif,
            fontique::GenericFamily::SystemUi,
            fontique::GenericFamily::UiSansSerif,
        ] {
            collection.set_generic_families(generic_family, chain_families.iter().copied());
        }
    }

    Collection { inner: collection, source_cache, default_fonts: Arc::new(default_fonts) }
}

#[derive(Clone)]
pub struct Collection {
    pub inner: fontique::Collection,
    pub source_cache: fontique::SourceCache,
    pub default_fonts: Arc<Vec<(std::path::PathBuf, fontique::QueryFont)>>,
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

pub const FALLBACK_FAMILIES: [fontique::GenericFamily; 2] = [
    // FemtoVG renderer needs SansSerif first, as it has difficulties rendering from SystemUi on macOS
    fontique::GenericFamily::SansSerif,
    fontique::GenericFamily::SystemUi,
];

/// Point the sans-serif / system-ui generic families at `family_name`, so text with no explicit
/// family resolves to it. Returns false if the family isn't registered.
pub fn set_default_font_family(collection: &mut fontique::Collection, family_name: &str) -> bool {
    let Some(id) = collection.family_id(family_name) else {
        return false;
    };
    for generic in [
        fontique::GenericFamily::SansSerif,
        fontique::GenericFamily::SystemUi,
        fontique::GenericFamily::UiSansSerif,
    ] {
        collection.set_generic_families(generic, core::iter::once(id));
    }
    true
}

/// Wrapper around fontique::Blob to permit use of the blob as a key in the cache in the different renderers,
/// to map the blob to the native type face representation (skia_safe::Typeface, femtovg::FontId, QRawFont, etc.).
/// The use as key also ensures the blob remains strongly referenced, so that it doesn't vanish from the
/// shared SourceCache (parley prunes it).
#[derive(Clone)]
pub struct HashedBlob(fontique::Blob<u8>);
impl core::hash::Hash for HashedBlob {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.id().hash(state);
    }
}

impl PartialEq for HashedBlob {
    fn eq(&self, other: &Self) -> bool {
        self.0.id() == other.0.id()
    }
}

impl Eq for HashedBlob {}

impl From<fontique::Blob<u8>> for HashedBlob {
    fn from(value: fontique::Blob<u8>) -> Self {
        Self(value)
    }
}

impl AsRef<fontique::Blob<u8>> for HashedBlob {
    fn as_ref(&self) -> &fontique::Blob<u8> {
        &self.0
    }
}
