// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::cell::RefCell;

pub use fontdb;

#[derive(derive_more::Deref, derive_more::DerefMut)]
pub struct FontDatabase {
    #[deref]
    #[deref_mut]
    db: fontdb::Database,
    #[cfg(not(any(
        target_family = "windows",
        target_os = "macos",
        target_os = "ios",
        target_arch = "wasm32"
    )))]
    pub fontconfig_fallback_families: Vec<crate::SharedString>,
}

thread_local! {
    pub static FONT_DB: RefCell<FontDatabase>  = RefCell::new(init_fontdb())
}

#[cfg(not(any(
    target_family = "windows",
    target_os = "macos",
    target_os = "ios",
    target_arch = "wasm32"
)))]
mod fontconfig;

fn init_fontdb() -> FontDatabase {
    let mut font_db = fontdb::Database::new();

    #[cfg(not(any(
        target_family = "windows",
        target_os = "macos",
        target_os = "ios",
        target_arch = "wasm32"
    )))]
    let mut fontconfig_fallback_families;

    #[cfg(target_arch = "wasm32")]
    {
        let data = include_bytes!("sharedfontdb/DejaVuSans.ttf");
        font_db.load_font_data(data.to_vec());
        font_db.set_sans_serif_family("DejaVu Sans");
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        font_db.load_system_fonts();
        cfg_if::cfg_if! {
            if #[cfg(not(any(
                target_family = "windows",
                target_os = "macos",
                target_os = "ios",
                target_arch = "wasm32"
            )))] {
                let default_sans_serif_family = {
                    fontconfig_fallback_families = fontconfig::find_families("sans-serif")
                        .into_iter()
                        .map(|s| s.into())
                        .collect::<Vec<crate::SharedString>>();
                    fontconfig_fallback_families.remove(0)
                };
            } else {
                let default_sans_serif_family = "Arial";
            }
        }
        font_db.set_sans_serif_family(default_sans_serif_family);
    }

    FontDatabase {
        db: font_db,
        #[cfg(not(any(
            target_family = "windows",
            target_os = "macos",
            target_os = "ios",
            target_arch = "wasm32"
        )))]
        fontconfig_fallback_families,
    }
}

/// This function can be used to register a custom TrueType font with Slint,
/// for use with the `font-family` property. The provided slice must be a valid TrueType
/// font.
pub fn register_font_from_memory(data: &'static [u8]) -> Result<(), Box<dyn std::error::Error>> {
    FONT_DB.with(|db| {
        db.borrow_mut().load_font_source(fontdb::Source::Binary(std::sync::Arc::new(data)))
    });
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn register_font_from_path(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let requested_path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    FONT_DB.with(|db| {
        for face_info in db.borrow().faces() {
            match &face_info.source {
                fontdb::Source::Binary(_) => {}
                fontdb::Source::File(loaded_path) | fontdb::Source::SharedFile(loaded_path, ..) => {
                    if *loaded_path == requested_path {
                        return Ok(());
                    }
                }
            }
        }

        db.borrow_mut().load_font_file(requested_path).map_err(|e| e.into())
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
