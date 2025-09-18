// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::sync::Arc;

pub use fontdb;
pub use ttf_parser;

#[derive(derive_more::Deref)]
pub struct FontDatabase {
    // This is in a Arc because usvg takes the database in a Arc
    #[deref]
    db: Arc<fontdb::Database>,
    #[cfg(not(any(
        target_family = "windows",
        target_vendor = "apple",
        target_arch = "wasm32",
        target_os = "android",
        target_os = "nto",
    )))]
    pub fontconfig_fallback_families: Vec<String>,
    // Default font families to use instead of SansSerif when SLINT_DEFAULT_FONT env var is set.
    pub default_font_family_ids: Vec<fontdb::ID>,
    // Same as default_font_families but reduced to unique family names
    default_font_family_names: Vec<String>,
}

impl FontDatabase {
    pub fn query_with_family(
        &self,
        query: fontdb::Query<'_>,
        family: Option<&'_ str>,
    ) -> Option<fontdb::ID> {
        let mut query = query;
        if let Some(specified_family) = family {
            let single_family = [fontdb::Family::Name(specified_family)];
            query.families = &single_family;
            self.db.query(&query)
        } else if self.default_font_family_ids.is_empty() {
            query.families = &[fontdb::Family::SansSerif];
            self.db.query(&query)
        } else {
            let family_storage = self
                .default_font_family_names
                .iter()
                .map(|name| fontdb::Family::Name(name))
                .collect::<Vec<_>>();
            query.families = &family_storage;
            self.db.query(&query)
        }
    }

    pub fn make_mut(&mut self) -> &mut fontdb::Database {
        Arc::make_mut(&mut self.db)
    }
}

thread_local! {
    pub static FONT_DB: RefCell<FontDatabase>  = RefCell::new(init_fontdb())
}

#[cfg(not(any(
    target_family = "windows",
    target_vendor = "apple",
    target_arch = "wasm32",
    target_os = "android",
    target_os = "nto",
)))]
mod fontconfig;

fn init_fontdb() -> FontDatabase {
    let mut font_db = fontdb::Database::new();

    #[cfg(not(target_arch = "wasm32"))]
    let (default_font_family_ids, default_font_family_names) =
        std::env::var_os("SLINT_DEFAULT_FONT")
            .and_then(|maybe_font_path| {
                let path = std::path::Path::new(&maybe_font_path);
                match if path.extension().is_some() {
                    font_db.load_font_file(path)
                } else {
                    font_db.load_fonts_dir(path);
                    Ok(())
                } {
                    Ok(_) => {
                        let mut family_ids = Vec::new();
                        let mut family_names = Vec::new();

                        for face_info in font_db.faces() {
                            family_ids.push(face_info.id);

                            let family_name = &face_info.families[0].0;
                            if let Err(insert_pos) = family_names.binary_search(family_name) {
                                family_names.insert(insert_pos, family_name.clone());
                            }
                        }

                        Some((family_ids, family_names))
                    }
                    Err(err) => {
                        eprintln!(
                            "Could not load the font set via `SLINT_DEFAULT_FONT`: {}: {}",
                            path.display(),
                            err,
                        );
                        None
                    }
                }
            })
            .unwrap_or_default();

    #[cfg(target_arch = "wasm32")]
    let (default_font_family_ids, default_font_family_names) =
        (Default::default(), Default::default());

    #[cfg(not(any(
        target_family = "windows",
        target_vendor = "apple",
        target_arch = "wasm32",
        target_os = "android",
        target_os = "nto",
    )))]
    let mut fontconfig_fallback_families = Vec::new();

    #[cfg(any(target_arch = "wasm32", target_os = "nto"))]
    {
        let data = include_bytes!("sharedfontdb/DejaVuSans.ttf");
        font_db.load_font_data(data.to_vec());
        font_db.set_sans_serif_family("DejaVu Sans");
    }
    #[cfg(target_os = "nto")]
    {
        font_db.set_sans_serif_family("Noto Sans");
    }
    #[cfg(target_os = "android")]
    {
        font_db.load_fonts_dir("/system/fonts");
        font_db.set_sans_serif_family("Roboto");
    }
    #[cfg(all(target_vendor = "apple", not(target_os = "macos")))]
    {
        font_db.load_fonts_dir("/System/Library/Fonts");
        font_db.load_fonts_dir("/System/Library/Cache");
        font_db.set_sans_serif_family("Arial");
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    {
        font_db.load_system_fonts();
        cfg_if::cfg_if! {
            if #[cfg(not(any(
                target_family = "windows",
                target_vendor = "apple",
                target_arch = "wasm32",
                target_os = "android",
                target_os = "nto",
            )))] {
                match fontconfig::find_families("sans-serif") {
                    Ok(mut fallback_families) => {
                        if !fallback_families.is_empty() {
                            let default_sans_serif_family = fallback_families.remove(0);
                            font_db.set_sans_serif_family(default_sans_serif_family);
                        }
                        fontconfig_fallback_families = fallback_families;
                    }
                    Err(e) => {
                        eprintln!("Error opening libfontconfig.so.1: {e}");
                    }
                }
            }
        }
        if font_db
            .query(&fontdb::Query { families: &[fontdb::Family::SansSerif], ..Default::default() })
            .is_none()
        {
            panic!(
                "Unable to determine default font. Failed to locate font for family {}",
                font_db.family_name(&fontdb::Family::SansSerif)
            )
        }
    }

    FontDatabase {
        db: Arc::new(font_db),
        #[cfg(not(any(
            target_family = "windows",
            target_vendor = "apple",
            target_arch = "wasm32",
            target_os = "android",
            target_os = "nto",
        )))]
        fontconfig_fallback_families,
        default_font_family_ids,
        default_font_family_names,
    }
}

/// This function can be used to register a custom TrueType font with Slint,
/// for use with the `font-family` property. The provided slice must be a valid TrueType
/// font.
pub fn register_font_from_memory(data: &'static [u8]) -> Result<(), Box<dyn std::error::Error>> {
    FONT_DB.with_borrow_mut(|db| {
        db.make_mut().load_font_source(fontdb::Source::Binary(std::sync::Arc::new(data)))
    });
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn register_font_from_path(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let requested_path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    FONT_DB.with_borrow_mut(|db| {
        for face_info in db.faces() {
            match &face_info.source {
                fontdb::Source::Binary(_) => {}
                fontdb::Source::File(loaded_path) => {
                    if *loaded_path == requested_path {
                        return Ok(());
                    }
                }
                #[cfg(not(target_os = "nto"))]
                fontdb::Source::SharedFile(loaded_path, ..) => {
                    if *loaded_path == requested_path {
                        return Ok(());
                    }
                }
            }
        }
        db.make_mut().load_font_file(requested_path).map_err(|e| e.into())
    })
}

#[cfg(target_arch = "wasm32")]
pub fn register_font_from_path(_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    return Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Registering fonts from paths is not supported in WASM builds",
    )
    .into());
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
    pub fn new(face: ttf_parser::Face<'_>) -> Self {
        Self {
            ascent: face.ascender() as f32,
            descent: face.descender() as f32,
            x_height: face.x_height().unwrap_or_default() as f32,
            cap_height: face.capital_height().unwrap_or_default() as f32,
            units_per_em: face.units_per_em() as f32,
        }
    }
}
