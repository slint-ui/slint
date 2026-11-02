// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::embedded_resources::BitmapFont;
use i_slint_common::sharedfontique::{fontique, skrifa};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

#[derive(Clone)]
enum FontData {
    Borrowed(fontique::Blob<u8>),
    Owned(std::sync::Arc<[u8]>),
}

#[derive(Clone)]
pub(crate) struct FontSource {
    data: FontData,
    index: u32,
}

impl FontSource {
    pub(crate) fn new(font: &fontique::QueryFont) -> Self {
        Self { data: FontData::Borrowed(font.blob.clone()), index: font.index }
    }
    fn bytes(&self) -> &[u8] {
        match &self.data {
            FontData::Borrowed(blob) => blob.data(),
            FontData::Owned(bytes) => bytes,
        }
    }
    fn size(&self) -> usize {
        self.bytes().len()
    }
    fn owned(mut self) -> Self {
        // Fontique file blobs can be memory-mapped; retained keys own an immutable snapshot.
        if let FontData::Borrowed(blob) = &self.data {
            self.data = FontData::Owned(blob.data().into());
        }
        self
    }
}
impl PartialEq for FontSource {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && (match (&self.data, &other.data) {
                (FontData::Borrowed(a), FontData::Borrowed(b)) if a.id() == b.id() => true,
                (FontData::Owned(a), FontData::Owned(b)) if std::sync::Arc::ptr_eq(a, b) => true,
                _ => self.bytes() == other.bytes(),
            })
    }
}
impl Eq for FontSource {}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct FontKey {
    pub weight: Option<u16>,
    pub sdf: bool,
    pub pixel_sizes: Vec<i16>,
    pub characters: Vec<char>,
    pub normalized_coords: Vec<i16>,
    pub variations: Vec<(skrifa::Tag, u32)>,
    pub family: String,
    pub character_sources: Vec<usize>,
    pub sources: Vec<FontSource>,
}

impl FontKey {
    fn size(&self) -> usize {
        core::mem::size_of::<Self>()
            + self.family.len()
            + self
                .sources
                .iter()
                .map(|source| source.size() + core::mem::size_of::<FontSource>())
                .sum::<usize>()
            + self.character_sources.len() * core::mem::size_of::<usize>()
            + self.characters.len() * core::mem::size_of::<char>()
            + self.pixel_sizes.len() * 2
            + self.normalized_coords.len() * 2
            + self.variations.len() * core::mem::size_of::<(skrifa::Tag, u32)>()
    }
}

#[derive(Clone, PartialEq, Eq)]
enum Key {
    Font(FontKey),
}
#[derive(Clone)]
enum Value {
    Font(BitmapFont),
}

struct Entry {
    key: Key,
    value: Value,
    size: usize,
}
struct Cache {
    entries: VecDeque<Entry>,
    size: usize,
    capacity: usize,
    #[cfg(test)]
    hits: usize,
}

impl Cache {
    fn get(&mut self, key: &Key) -> Option<Value> {
        let index = self.entries.iter().position(|entry| &entry.key == key)?;
        #[cfg(test)]
        {
            self.hits += 1;
        }
        let entry = self.entries.remove(index).unwrap();
        let result = entry.value.clone();
        self.entries.push_back(entry);
        Some(result)
    }

    fn insert(&mut self, key: Key, value: Value, size: usize) {
        if size > self.capacity {
            return;
        }
        if let Some(index) = self.entries.iter().position(|entry| entry.key == key) {
            self.size -= self.entries.remove(index).unwrap().size;
        }
        while self.size.saturating_add(size) > self.capacity {
            let Some(entry) = self.entries.pop_front() else {
                break;
            };
            self.size -= entry.size;
        }
        self.entries.push_back(Entry { key, value, size });
        self.size += size;
    }
}

/// A bounded cache of generated resources for repeated compilations.
/// Entries compare font contents and rendering inputs rather than file names or modification times.
/// Clones share the cache; eviction retains the most recently used entries.
#[derive(Clone)]
pub struct ResourceCache(Rc<RefCell<Cache>>);

impl Default for ResourceCache {
    fn default() -> Self {
        Self::new(64 * 1024 * 1024)
    }
}

impl ResourceCache {
    /// Limits retained source data and generated pixels to approximately `capacity` bytes.
    /// An individual resource exceeding the capacity is generated without retaining it.
    pub fn new(capacity: usize) -> Self {
        Self(Rc::new(RefCell::new(Cache {
            entries: VecDeque::new(),
            size: 0,
            capacity,
            #[cfg(test)]
            hits: 0,
        })))
    }

    pub fn clear(&self) {
        let mut cache = self.0.borrow_mut();
        cache.entries.clear();
        cache.size = 0;
    }

    pub(crate) fn font(&self, key: &FontKey) -> Option<BitmapFont> {
        match self.0.borrow_mut().get(&Key::Font(key.clone()))? {
            Value::Font(font) => Some(font),
        }
    }

    pub(crate) fn insert_font(&self, key: FontKey, font: &BitmapFont) {
        let size = key.size()
            + core::mem::size_of_val(font)
            + font.family_name.len()
            + font.character_map.len() * 4
            + font
                .glyphs
                .iter()
                .map(|set| {
                    core::mem::size_of_val(set)
                        + set
                            .glyph_data
                            .iter()
                            .map(|glyph| core::mem::size_of_val(glyph) + glyph.data.len())
                            .sum::<usize>()
                })
                .sum::<usize>();
        let mut cache = self.0.borrow_mut();
        if size <= cache.capacity {
            let mut key = key;
            key.sources = key.sources.into_iter().map(FontSource::owned).collect();
            cache.insert(Key::Font(key), Value::Font(font.clone()), size);
        }
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use crate::{
        CompilerConfiguration, EmbedResourcesKind, SystemFonts, generator::OutputFormat,
        llr::serialize, source_path::SourcePath,
    };

    fn compile(
        source: &str,
        system: &SystemFonts,
        cache: Option<ResourceCache>,
        scale: f32,
    ) -> Vec<Vec<u8>> {
        let mut config = CompilerConfiguration::new(OutputFormat::Interpreter);
        config.embed_resources = EmbedResourcesKind::EmbedTextures;
        config.system_fonts = Some(system.clone());
        config.resource_cache = cache;
        config.const_scale_factor = Some(scale);
        let (diagnostics, compiled) = spin_on::spin_on(serialize::compile(
            &SourcePath::File(std::env::temp_dir().join("resource-cache.slint")),
            source.into(),
            config,
        ));
        assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());
        compiled.unwrap().1.iter().map(|r| serialize::resource_to_bytes(r).unwrap()).collect()
    }

    fn source(font: &str, size: u16, weight: u16, text: &str) -> String {
        format!(
            r#"import "{font}"; export component Test inherits Window {{
            width: 320px; height: 240px; background: red; default-font-family: "Inter";
            Text {{ text: "{text}"; font-size: {size}px; font-weight: {weight}; }}
        }}"#
        )
    }

    fn fixture() -> String {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../common/sharedfontique/Inter-VariableFont.ttf")
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn font_cache_preserves_assets_and_distinguishes_rendering_inputs() {
        let system = SystemFonts::default();
        let cache = ResourceCache::default();
        for (size, weight, text, scale) in [
            (14, 400, "cache", 1.),
            (14, 400, "cache", 1.),
            (17, 400, "cache", 1.),
            (14, 700, "cache", 1.),
            (14, 400, "Ω", 1.),
            (14, 400, "cache", 1.5),
        ] {
            let source = source(&fixture(), size, weight, text);
            let expected = compile(&source, &system, None, scale);
            assert_eq!(compile(&source, &system, Some(cache.clone()), scale), expected);
            assert!(!cache.0.borrow().entries.is_empty());
        }
        let initial = cache.0.borrow().entries.len();
        let hits = cache.0.borrow().hits;
        let before = source(&fixture(), 14, 400, "cache");
        assert_eq!(
            compile(
                &before.replace("background: red", "background: blue"),
                &system,
                Some(cache.clone()),
                1.
            ),
            compile(&before, &system, None, 1.)
        );
        assert_eq!(cache.0.borrow().entries.len(), initial);
        assert!(cache.0.borrow().hits > hits);
    }

    #[test]
    fn changed_font_bytes_at_the_same_path_invalidate_generated_fonts() {
        let system = SystemFonts::default();
        let cache = ResourceCache::default();
        let path =
            std::env::temp_dir().join(format!("slint-cache-font-{}.ttf", std::process::id()));
        let mut bytes = std::fs::read(fixture()).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let source = source(&path.to_string_lossy(), 14, 400, "cache");
        let before = compile(&source, &system, Some(cache.clone()), 1.);
        let head = bytes[12..].chunks_exact(16).find(|record| &record[..4] == b"head").unwrap();
        let offset = u32::from_be_bytes(head[8..12].try_into().unwrap()) as usize;
        let units = u16::from_be_bytes(bytes[offset + 18..offset + 20].try_into().unwrap());
        bytes[offset + 18..offset + 20].copy_from_slice(&(units + 16).to_be_bytes());
        std::fs::write(&path, bytes).unwrap();
        let expected = compile(&source, &system, None, 1.);
        assert_ne!(before, expected);
        assert_eq!(compile(&source, &system, Some(cache), 1.), expected);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn cache_eviction_and_oversized_resources_preserve_output() {
        let system = SystemFonts::default();
        let source = source(&fixture(), 14, 400, "cache");
        let expected = compile(&source, &system, None, 1.);
        let zero = ResourceCache::new(0);
        assert_eq!(compile(&source, &system, Some(zero.clone()), 1.), expected);
        assert!(zero.0.borrow().entries.is_empty());
        let cache = ResourceCache::default();
        compile(&source, &system, Some(cache.clone()), 1.);
        let size = cache.0.borrow().size;
        cache.0.borrow_mut().capacity = size;
        compile(&source.replace("14px", "18px"), &system, Some(cache.clone()), 1.);
        assert!(cache.0.borrow().size <= size);
        assert_eq!(compile(&source, &system, Some(cache.clone()), 1.), expected);
        cache.clear();
        assert!(cache.0.borrow().entries.is_empty());
        assert_eq!(cache.0.borrow().size, 0);
    }
}
