// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::{Model, ModelExt, ModelRc, SharedString};

use super::{Api, ElementLibraryEntry, ElementLibraryGroup, PaletteComponentKind};

fn catalog() -> ModelRc<ElementLibraryGroup> {
    let groups = [
        (
            "Visual",
            vec![
                ElementLibraryEntry {
                    label: "Rectangle".into(),
                    kind: PaletteComponentKind::Rectangle,
                },
                ElementLibraryEntry { label: "Text".into(), kind: PaletteComponentKind::Text },
                ElementLibraryEntry { label: "Image".into(), kind: PaletteComponentKind::Image },
            ],
        ),
        (
            "Input & interaction",
            vec![ElementLibraryEntry {
                label: "TouchArea".into(),
                kind: PaletteComponentKind::TouchArea,
            }],
        ),
    ];
    ModelRc::new(slint::VecModel::from(
        groups
            .into_iter()
            .map(|(label, mut entries)| {
                entries.sort_by(|a, b| a.label.cmp(&b.label));
                ElementLibraryGroup {
                    label: label.into(),
                    entries: ModelRc::new(slint::VecModel::from(entries)),
                }
            })
            .collect::<Vec<_>>(),
    ))
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

fn has_matches(groups: ModelRc<ElementLibraryGroup>, query: SharedString) -> bool {
    groups.iter().any(|group| group.entries.iter().any(|entry| matches_query(&entry, &query)))
}

pub(super) fn setup(api: &Api<'_>) {
    api.set_element_library(catalog());
    api.on_element_library_normalize_query(normalize_query);
    api.on_element_library_filter_entries(filter);
    api.on_element_library_has_matches(has_matches);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_is_alphabetical_and_search_preserves_order() {
        let groups = catalog();
        assert_eq!(groups.row_count(), 2);
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
        let interaction = groups.row_data(1).unwrap();
        assert_eq!(interaction.label, "Input & interaction");
        assert_eq!(interaction.entries.row_data(0).unwrap().kind, PaletteComponentKind::TouchArea);
        for (query, expected) in [
            ("", vec!["TouchArea"]),
            ("  ", vec!["TouchArea"]),
            (" tOuCh ", vec!["TouchArea"]),
            ("AREA", vec!["TouchArea"]),
            ("image", vec![]),
            ("missing", vec![]),
        ] {
            assert_eq!(
                filter(interaction.entries.clone(), normalize_query(query.into()))
                    .iter()
                    .map(|entry| entry.label.to_string())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn library_has_matches_in_any_group() {
        let groups = ModelRc::new(slint::VecModel::from(vec![
            ElementLibraryGroup { label: "Empty".into(), entries: ModelRc::default() },
            catalog().row_data(0).unwrap(),
        ]));
        assert!(has_matches(groups.clone(), normalize_query("  tEx  ".into())));
        assert!(!has_matches(groups, normalize_query("missing".into())));
        assert!(!has_matches(ModelRc::default(), "".into()));
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
