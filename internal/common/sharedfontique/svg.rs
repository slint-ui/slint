// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Lets usvg render SVG `<text>` using fontique fonts.
//!
//! usvg shapes text against its own `fontdb`, which Slint leaves empty, so SVG glyphs go missing.
//! [`options`] returns usvg options whose font resolver looks each font up through a
//! caller-provided query and registers only the used faces into usvg's database.
//! The caller picks the collection, so the runtime and the compiler share one bridge.

use super::fontique;
use resvg::usvg;
use usvg::fontdb;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Runs a fontique query against the given collection and returns the first match.
/// When `require_char` is set, only fonts covering that character match (for fallback).
/// Callers wrap this with their own collection lookup.
pub fn query_font(
    collection: &mut fontique::Collection,
    source_cache: &mut fontique::SourceCache,
    families: &[fontique::QueryFamily],
    attributes: fontique::Attributes,
    require_char: Option<char>,
) -> Option<fontique::QueryFont> {
    let mut found = None;
    let mut query = collection.query(source_cache);
    query.set_families(families.iter().copied());
    query.set_attributes(attributes);
    query.matches_with(|font| {
        let covers = require_char.is_none_or(|c| font.charmap().and_then(|cm| cm.map(c)).is_some());
        if covers {
            found = Some(font.clone());
            fontique::QueryStatus::Stop
        } else {
            fontique::QueryStatus::Continue
        }
    });
    found
}

fn to_fontique_family(family: &usvg::FontFamily) -> fontique::QueryFamily<'_> {
    use fontique::{GenericFamily, QueryFamily};
    match family {
        usvg::FontFamily::Serif => QueryFamily::Generic(GenericFamily::Serif),
        usvg::FontFamily::SansSerif => QueryFamily::Generic(GenericFamily::SansSerif),
        usvg::FontFamily::Cursive => QueryFamily::Generic(GenericFamily::Cursive),
        usvg::FontFamily::Fantasy => QueryFamily::Generic(GenericFamily::Fantasy),
        usvg::FontFamily::Monospace => QueryFamily::Generic(GenericFamily::Monospace),
        usvg::FontFamily::Named(name) => QueryFamily::Named(name),
    }
}

fn to_fontique_attributes(font: &usvg::Font) -> fontique::Attributes {
    let style = match font.style() {
        usvg::FontStyle::Normal => fontique::FontStyle::Normal,
        usvg::FontStyle::Italic => fontique::FontStyle::Italic,
        usvg::FontStyle::Oblique => fontique::FontStyle::Oblique(None),
    };
    let width = match font.stretch() {
        usvg::FontStretch::UltraCondensed => fontique::FontWidth::ULTRA_CONDENSED,
        usvg::FontStretch::ExtraCondensed => fontique::FontWidth::EXTRA_CONDENSED,
        usvg::FontStretch::Condensed => fontique::FontWidth::CONDENSED,
        usvg::FontStretch::SemiCondensed => fontique::FontWidth::SEMI_CONDENSED,
        usvg::FontStretch::Normal => fontique::FontWidth::NORMAL,
        usvg::FontStretch::SemiExpanded => fontique::FontWidth::SEMI_EXPANDED,
        usvg::FontStretch::Expanded => fontique::FontWidth::EXPANDED,
        usvg::FontStretch::ExtraExpanded => fontique::FontWidth::EXTRA_EXPANDED,
        usvg::FontStretch::UltraExpanded => fontique::FontWidth::ULTRA_EXPANDED,
    };
    fontique::Attributes { weight: fontique::FontWeight::new(font.weight() as f32), style, width }
}

/// Faces registered per parse, keyed by the fontique blob id,
/// so the same font is added to the database only once even when many spans share it.
type Registered = Mutex<HashMap<u64, Vec<fontdb::ID>>>;

/// Adds the face data behind `font` to `db` (once) and returns the id of that face.
fn register(
    db: &mut Arc<fontdb::Database>,
    font: &fontique::QueryFont,
    registered: &Registered,
) -> Option<fontdb::ID> {
    let mut registered = registered.lock().ok()?;
    let ids = registered.entry(font.blob.id()).or_insert_with(|| {
        let source = fontdb::Source::Binary(Arc::new(font.blob.clone()));
        fontdb::Database::load_font_source(Arc::make_mut(db), source).to_vec()
    });
    // fontdb may skip faces it cannot load, so match by face index rather than position.
    ids.iter()
        .copied()
        .find(|id| db.face(*id).is_some_and(|f| f.index == font.index))
        .or_else(|| ids.first().copied())
}

/// Builds usvg options whose font resolver resolves `<text>` fonts through `find_font`.
///
/// `find_font(families, attributes, require_char)` returns the chosen fontique font,
/// and the caller decides which collection to query (see [`query_font`]).
pub fn options<F>(find_font: F) -> usvg::Options<'static>
where
    F: Fn(
            &[fontique::QueryFamily],
            fontique::Attributes,
            Option<char>,
        ) -> Option<fontique::QueryFont>
        + Send
        + Sync
        + 'static,
{
    let find_font = Arc::new(find_font);
    let registered: Arc<Registered> = Arc::new(Mutex::new(HashMap::new()));

    let select_font = {
        let find_font = find_font.clone();
        let registered = registered.clone();
        move |font: &usvg::Font, db: &mut Arc<fontdb::Database>| -> Option<fontdb::ID> {
            // usvg drops a span whose base font stays unresolved (`select_fallback`
            // below only covers missing glyphs), so chain the generic families
            // rather than returning None for an unknown named family.
            let families: Vec<fontique::QueryFamily> = font
                .families()
                .iter()
                .map(to_fontique_family)
                .chain(super::FALLBACK_FAMILIES.iter().copied().map(fontique::QueryFamily::Generic))
                .collect();
            let found = find_font(&families, to_fontique_attributes(font), None)?;
            register(db, &found, &registered)
        }
    };

    let select_fallback = {
        let find_font = find_font.clone();
        let registered = registered.clone();
        move |c: char,
              exclude: &[fontdb::ID],
              db: &mut Arc<fontdb::Database>|
              -> Option<fontdb::ID> {
            // Slint's fallback chain, plus emoji.
            let families: Vec<fontique::QueryFamily> = super::FALLBACK_FAMILIES
                .iter()
                .copied()
                .chain(std::iter::once(fontique::GenericFamily::Emoji))
                .map(fontique::QueryFamily::Generic)
                .collect();
            let found = find_font(&families, fontique::Attributes::default(), Some(c))?;
            let id = register(db, &found, &registered)?;
            // The character may already be served by an excluded font picked above.
            (!exclude.contains(&id)).then_some(id)
        }
    };

    usvg::Options {
        font_resolver: usvg::FontResolver {
            select_font: Box::new(select_font),
            select_fallback: Box::new(select_fallback),
        },
        ..Default::default()
    }
}
