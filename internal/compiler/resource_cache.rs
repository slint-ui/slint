// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::embedded_resources::BitmapFont;
use i_slint_common::sharedfontique::{fontique, skrifa};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
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
    Image(ImageKey),
}
#[derive(Clone)]
enum Value {
    Font(BitmapFont),
    Image(CachedImage),
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
            _ => None,
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

    pub(super) fn compile_with_diagnostics(
        source: &str,
        system: &SystemFonts,
        cache: Option<ResourceCache>,
        scale: f32,
    ) -> (Vec<Vec<u8>>, Vec<String>) {
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
        let resources =
            compiled.unwrap().1.iter().map(|r| serialize::resource_to_bytes(r).unwrap()).collect();
        (resources, diagnostics.to_string_vec())
    }

    pub(super) fn compile(
        source: &str,
        system: &SystemFonts,
        cache: Option<ResourceCache>,
        scale: f32,
    ) -> Vec<Vec<u8>> {
        compile_with_diagnostics(source, system, cache, scale).0
    }

    fn source(font: &str, size: u16, weight: u16, text: &str) -> String {
        format!(
            r#"import "{font}"; export component Test inherits Window {{
            width: 320px; height: 240px; background: red; default-font-family: "Inter";
            Text {{ text: "{text}"; font-size: {size}px; font-weight: {weight}; }}
        }}"#
        )
    }

    pub(super) fn fixture() -> String {
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
        compile(&source.replace("14px", "10px"), &system, Some(cache.clone()), 1.);
        assert!(cache.0.borrow().size <= size);
        assert!(cache.0.borrow().entries.iter().all(|entry| match &entry.key {
            Key::Font(key) => !key.pixel_sizes.contains(&14),
            _ => true,
        }));
        assert_eq!(compile(&source, &system, Some(cache.clone()), 1.), expected);
        cache.clear();
        assert!(cache.0.borrow().entries.is_empty());
        assert_eq!(cache.0.borrow().size, 0);
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ImageKey {
    hash: u64,
    extension: Option<String>,
    scale: u32,
    font_context: bool,
    data: Vec<u8>,
}
impl ImageKey {
    pub(crate) fn new(
        data: &[u8],
        extension: Option<&str>,
        scale: f32,
        font_context: bool,
    ) -> Self {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        data.hash(&mut hash);
        Self {
            hash: hash.finish(),
            extension: extension.map(str::to_owned),
            scale: scale.to_bits(),
            font_context,
            data: data.to_vec(),
        }
    }
    fn size(&self) -> usize {
        core::mem::size_of::<Self>()
            + self.data.len()
            + self.extension.as_ref().map_or(0, String::len)
    }
}

#[derive(Clone, PartialEq)]
enum Family {
    Named(String),
    Generic(fontique::GenericFamily),
    Id(fontique::FamilyId),
}
impl Family {
    fn own(value: &fontique::QueryFamily<'_>) -> Self {
        match value {
            fontique::QueryFamily::Named(name) => Self::Named((*name).into()),
            fontique::QueryFamily::Generic(g) => Self::Generic(*g),
            fontique::QueryFamily::Id(id) => Self::Id(*id),
        }
    }
    fn borrowed(&self) -> fontique::QueryFamily<'_> {
        match self {
            Self::Named(name) => fontique::QueryFamily::Named(name),
            Self::Generic(g) => fontique::QueryFamily::Generic(*g),
            Self::Id(id) => fontique::QueryFamily::Id(*id),
        }
    }
}

#[derive(Clone)]
pub(crate) struct FontQuery {
    families: Vec<Family>,
    attributes: fontique::Attributes,
    character: Option<char>,
    source: Option<FontSource>,
}
impl FontQuery {
    pub(crate) fn matches(
        &self,
        collection: &mut i_slint_common::sharedfontique::Collection,
    ) -> bool {
        // usvg's resolver supplies names and generics, never collection-local IDs.
        if self.families.iter().any(|f| matches!(f, Family::Id(_))) {
            return false;
        }
        let families: Vec<_> = self.families.iter().map(Family::borrowed).collect();
        let found = i_slint_common::sharedfontique::svg::query_font(
            &mut collection.inner,
            &mut collection.source_cache,
            &families,
            self.attributes,
            self.character,
        );
        match (&self.source, found.as_ref()) {
            (None, None) => true,
            (Some(expected), Some(found)) => expected == &FontSource::new(found),
            _ => false,
        }
    }
    fn size(&self) -> usize {
        core::mem::size_of::<Self>()
            + self.source.as_ref().map_or(0, FontSource::size)
            + self
                .families
                .iter()
                .map(|f| {
                    core::mem::size_of_val(f)
                        + match f {
                            Family::Named(s) => s.len(),
                            _ => 0,
                        }
                })
                .sum::<usize>()
    }
}

pub(crate) struct FontQueries {
    entries: Vec<FontQuery>,
    sources: std::collections::HashMap<(u64, u32), FontSource>,
    size: usize,
    capacity: usize,
    cacheable: bool,
}
impl FontQueries {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            sources: Default::default(),
            size: 0,
            capacity,
            cacheable: true,
        }
    }
    pub(crate) fn record(
        &mut self,
        families: &[fontique::QueryFamily<'_>],
        attributes: fontique::Attributes,
        character: Option<char>,
        found: Option<&fontique::QueryFont>,
    ) {
        if !self.cacheable {
            return;
        }
        let families: Vec<_> = families.iter().map(Family::own).collect();
        if self.entries.iter().any(|q| {
            q.families == families && q.attributes == attributes && q.character == character
        }) {
            return;
        }
        let mut size = core::mem::size_of::<FontQuery>()
            + families
                .iter()
                .map(|f| {
                    core::mem::size_of_val(f)
                        + match f {
                            Family::Named(s) => s.len(),
                            _ => 0,
                        }
                })
                .sum::<usize>();
        if let Some(font) = found {
            if !self.sources.contains_key(&(font.blob.id(), font.index)) {
                size = size.saturating_add(font.blob.data().len());
            }
        }
        if self.size.saturating_add(size) > self.capacity {
            self.cacheable = false;
            self.entries.clear();
            self.sources.clear();
            return;
        }
        self.size += size;
        let source = found.map(|f| {
            self.sources
                .entry((f.blob.id(), f.index))
                .or_insert_with(|| FontSource::new(f).owned())
                .clone()
        });
        self.entries.push(FontQuery { families, attributes, character, source });
    }
    pub(crate) fn external_image(&mut self) {
        self.cacheable = false;
        self.entries.clear();
        self.sources.clear();
    }
    pub(crate) fn finish(&mut self) -> Option<Vec<FontQuery>> {
        self.cacheable.then(|| core::mem::take(&mut self.entries))
    }
}

#[derive(Clone)]
pub(crate) struct CachedImage {
    pub texture: crate::embedded_resources::Texture,
    pub animated: bool,
    pub fonts: Vec<FontQuery>,
}
impl ResourceCache {
    pub(crate) fn capacity(&self) -> usize {
        self.0.borrow().capacity
    }
    pub(crate) fn image(&self, key: &ImageKey) -> Option<CachedImage> {
        match self.0.borrow_mut().get(&Key::Image(key.clone()))? {
            Value::Image(image) => Some(image),
            _ => None,
        }
    }
    pub(crate) fn insert_image(&self, key: ImageKey, image: &CachedImage) {
        let size = key.size()
            + core::mem::size_of_val(image)
            + image.texture.data.len()
            + image.texture.textures.len()
                * core::mem::size_of::<i_slint_common::pre_rendered::StaticTexture>()
            + image.fonts.iter().map(FontQuery::size).sum::<usize>();
        self.0.borrow_mut().insert(Key::Image(key), Value::Image(image.clone()), size);
    }
}

#[cfg(all(test, feature = "serde"))]
mod image_tests {
    use super::tests::{compile, compile_with_diagnostics, fixture};
    use super::*;
    use crate::SystemFonts;

    fn scene(path: &str) -> String {
        format!(
            r#"export component Main inherits Window {{ width: 32px; height: 32px; background:red;
            Image {{ source: @image-url("{path}"); }} }}"#
        )
    }
    fn path(name: &str, extension: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("slint-cache-{}-{name}.{extension}", std::process::id()))
    }
    fn png(path: &std::path::Path, color: [u8; 4]) {
        image::RgbaImage::from_pixel(16, 16, image::Rgba(color)).save(path).unwrap();
    }
    fn data_uri(data: &[u8], mime: &str) -> String {
        format!("data:{mime},{}", data.iter().map(|b| format!("%{b:02x}")).collect::<String>())
    }

    #[test]
    fn image_cache_reuses_pixels_and_invalidates_same_path_replacements_and_scale() {
        let system = SystemFonts::default();
        let cache = ResourceCache::default();
        let path = path("pixels", "png");
        png(&path, [255, 0, 0, 255]);
        let source = scene(&path.to_string_lossy());
        let expected = compile(&source, &system, None, 1.);
        assert_eq!(compile(&source, &system, Some(cache.clone()), 1.), expected);
        assert!(cache.0.borrow().entries.iter().any(|e| matches!(e.key, Key::Image(_))));
        let hits = cache.0.borrow().hits;
        assert_eq!(
            compile(
                &source.replace("background:red", "background:blue"),
                &system,
                Some(cache.clone()),
                1.
            ),
            expected
        );
        assert!(cache.0.borrow().hits > hits);
        png(&path, [0, 0, 255, 255]);
        let updated = compile(&source, &system, None, 1.);
        assert_ne!(updated, expected);
        assert_eq!(compile(&source, &system, Some(cache.clone()), 1.), updated);
        let scaled = compile(&source, &system, None, 0.5);
        assert_ne!(scaled[0], updated[0]);
        assert_eq!(compile(&source, &system, Some(cache.clone()), 0.5), scaled);
        let empty = ResourceCache::new(0);
        assert_eq!(compile(&source, &system, Some(empty.clone()), 1.), updated);
        assert!(empty.0.borrow().entries.is_empty());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn image_cache_preserves_data_uri_svg_pixels_and_scaling() {
        let system = SystemFonts::default();
        let cache = ResourceCache::default();
        let svg=br##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><circle cx="8" cy="8" r="7" fill="#f03010"/></svg>"##;
        let source = scene(&data_uri(svg, "image/svg+xml"));
        for scale in [1., 0.5, 2., 1.] {
            assert_eq!(
                compile(&source, &system, Some(cache.clone()), scale),
                compile(&source, &system, None, scale)
            );
        }
    }

    #[test]
    fn image_cache_revalidates_fonts_used_inside_svg_text() {
        let system = SystemFonts::default();
        let cache = ResourceCache::default();
        let font = path("svg-font", "ttf");
        let svg = path("font-text", "svg");
        let mut bytes = std::fs::read(fixture()).unwrap();
        std::fs::write(&font, &bytes).unwrap();
        std::fs::write(&svg,r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="80"><text x="5" y="50" font-family="Inter" font-size="32">Cache</text></svg>"#).unwrap();
        let source = format!("import \"{}\"; {}", font.display(), scene(&svg.to_string_lossy()));
        let before = compile(&source, &system, Some(cache.clone()), 1.);
        assert_eq!(before, compile(&source, &system, None, 1.));
        assert!(
            cache
                .0
                .borrow()
                .entries
                .iter()
                .any(|e| matches!(&e.value,Value::Image(image) if !image.fonts.is_empty()))
        );
        assert_eq!(compile(&source, &system, Some(cache.clone()), 1.), before);
        let head = bytes[12..].chunks_exact(16).find(|record| &record[..4] == b"head").unwrap();
        let offset = u32::from_be_bytes(head[8..12].try_into().unwrap()) as usize;
        let units = u16::from_be_bytes(bytes[offset + 18..offset + 20].try_into().unwrap());
        bytes[offset + 18..offset + 20].copy_from_slice(&(units + 256).to_be_bytes());
        std::fs::write(&font, bytes).unwrap();
        let after = compile(&source, &system, None, 1.);
        assert_ne!(before[0], after[0]);
        assert_eq!(compile(&source, &system, Some(cache), 1.), after);
        std::fs::remove_file(font).unwrap();
        std::fs::remove_file(svg).unwrap();
    }

    #[test]
    fn svg_external_images_are_not_retained() {
        let system = SystemFonts::default();
        let cache = ResourceCache::default();
        let raster = path("external-raster", "png");
        let svg = path("external", "svg");
        png(&raster, [255, 0, 0, 255]);
        std::fs::write(&svg,format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><image href="{}" width="16" height="16"/></svg>"#,raster.display())).unwrap();
        let source = scene(&svg.to_string_lossy());
        let before = compile(&source, &system, Some(cache.clone()), 1.);
        assert!(!cache.0.borrow().entries.iter().any(|e| matches!(e.key, Key::Image(_))));
        png(&raster, [0, 0, 255, 255]);
        let after = compile(&source, &system, Some(cache.clone()), 1.);
        assert_ne!(before[0], after[0]);
        assert_eq!(after, compile(&source, &system, None, 1.));
        std::fs::remove_file(raster).unwrap();
        std::fs::remove_file(svg).unwrap();
    }

    #[test]
    #[cfg(feature = "image-default-formats")]
    fn cached_animation_repeats_the_first_frame_warning() {
        let system = SystemFonts::default();
        let cache = ResourceCache::default();
        let bytes = include_bytes!("../core/graphics/image/testdata/frames.gif");
        let source = scene(&data_uri(bytes, "image/gif"));
        let expected = compile_with_diagnostics(&source, &system, None, 1.);
        assert_eq!(
            expected.1.iter().filter(|line| line.contains("Only the first frame")).count(),
            1
        );
        for _ in 0..2 {
            assert_eq!(
                compile_with_diagnostics(&source, &system, Some(cache.clone()), 1.),
                expected
            );
        }
    }
}
