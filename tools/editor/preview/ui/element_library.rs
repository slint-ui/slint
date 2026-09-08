// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::{Model, ModelExt, ModelRc, SharedString};

use super::{Api, ElementLibraryEntry, ElementLibraryGroup, PaletteComponentKind};

fn catalog() -> ModelRc<ElementLibraryGroup> {
    let mut entries = [
        ElementLibraryEntry { label: "Rectangle".into(), kind: PaletteComponentKind::Rectangle },
        ElementLibraryEntry { label: "Text".into(), kind: PaletteComponentKind::Text },
        ElementLibraryEntry { label: "Image".into(), kind: PaletteComponentKind::Image },
    ];
    entries.sort_by(|a, b| a.label.cmp(&b.label));
    ModelRc::new(slint::VecModel::from(vec![ElementLibraryGroup {
        label: "Visual".into(),
        entries: ModelRc::new(slint::VecModel::from(Vec::from(entries))),
    }]))
}

fn normalize_query(query: SharedString) -> SharedString {
    query.trim().to_lowercase().into()
}

fn matches_query(entry: &ElementLibraryEntry, query: &str) -> bool {
    entry.label.to_lowercase().contains(query)
}

fn filter(
    entries: ModelRc<ElementLibraryEntry>,
    query: SharedString,
) -> ModelRc<ElementLibraryEntry> {
    ModelRc::new(entries.filter(move |entry| matches_query(entry, &query)))
}

pub(super) fn setup(api: &Api<'_>) {
    api.set_element_library(catalog());
    api.on_normalize_element_search(normalize_query);
    api.on_filter_library_elements(filter);
    api.on_element_library_has_matches(|groups, query| {
        groups.iter().any(|group| group.entries.iter().any(|entry| matches_query(&entry, &query)))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_library_is_alphabetical_and_search_preserves_order() {
        let groups = catalog();
        assert_eq!(groups.row_count(), 1);
        let group = groups.row_data(0).unwrap();
        assert_eq!(group.label, "Visual");
        for (query, expected) in [
            ("", vec!["Image", "Rectangle", "Text"]),
            ("  ", vec!["Image", "Rectangle", "Text"]),
            ("E", vec!["Image", "Rectangle", "Text"]),
            ("  aG  ", vec!["Image"]),
            ("t", vec!["Rectangle", "Text"]),
            ("TouchArea", vec![]),
        ] {
            let result = filter(group.entries.clone(), normalize_query(query.into()));
            assert_eq!(
                result.iter().map(|entry| entry.label.to_string()).collect::<Vec<_>>(),
                expected,
                "query: {query:?}"
            );
        }
    }

    #[test]
    fn filtered_entries_follow_catalog_changes() {
        let entries = std::rc::Rc::new(slint::VecModel::from(vec![ElementLibraryEntry {
            label: "Rectangle".into(),
            kind: PaletteComponentKind::Rectangle,
        }]));
        let filtered = filter(entries.clone().into(), "text".into());
        assert_eq!(filtered.row_count(), 0);
        entries
            .push(ElementLibraryEntry { label: "Text".into(), kind: PaletteComponentKind::Text });
        assert_eq!(filtered.row_data(0).unwrap().label, "Text");
        entries.remove(1);
        assert_eq!(filtered.row_count(), 0);
    }
}
