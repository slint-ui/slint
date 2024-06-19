// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore descr rfind unindented

use crate::common::{ComponentInformation, DocumentCache, Position, PropertyChange};
#[cfg(feature = "preview-engine")]
use i_slint_compiler::langtype::{DefaultSizeBinding, ElementType};

#[cfg(feature = "preview-engine")]
fn builtin_component_info(name: &str, fills_parent: bool) -> ComponentInformation {
    let (category, is_layout) = match name {
        "GridLayout" | "HorizontalLayout" | "VerticalLayout" => ("Layout", true),
        "Dialog" | "Window" | "PopupWindow" => ("Window Management", false),
        "FocusScope" | "TouchArea" => ("Event Handling", false),
        "Text" => ("Text Handling", false),
        _ => ("Primitives", false),
    };

    let default_properties = match name {
        "Text" | "TextInput" => vec![PropertyChange::new("text", format!("\"{name}\""))],
        "Image" => vec![PropertyChange::new("source", "@image-url(\"EDIT_ME.png\")".to_string())],
        _ => vec![],
    };

    ComponentInformation {
        name: name.to_string(),
        category: category.to_string(),
        is_global: false,
        is_builtin: true,
        is_std_widget: false,
        is_layout,
        fills_parent: is_layout || fills_parent,
        is_exported: true,
        defined_at: None,
        default_properties,
    }
}

fn std_widgets_info(name: &str, is_global: bool) -> ComponentInformation {
    let (category, is_layout) = match name {
        "GridBox" | "HorizontalBox" | "VerticalBox" => ("Layout", true),
        "LineEdit" | "TextEdit" => ("Text Handling", false),
        "Button" | "CheckBox" | "ComboBox" | "Slider" | "SpinBox" | "Switch" => ("Input", false),
        "ProgressIndicator" | "Spinner" => ("Status", false),
        "ListView" | "StandardListView" | "StandardTableView" => ("Views", false),
        _ => ("Widgets", false),
    };

    let default_properties = match name {
        "Button" | "CheckBox" | "LineEdit" | "Switch" | "TextEdit" => {
            vec![PropertyChange::new("text", format!("\"{name}\""))]
        }
        "ComboBox" => {
            vec![PropertyChange::new("model", "[\"first\", \"second\", \"third\"]".to_string())]
        }
        "Slider" | "SpinBox" => vec![
            PropertyChange::new("minimum", "0".to_string()),
            PropertyChange::new("value", "42".to_string()),
            PropertyChange::new("maximum", "100".to_string()),
        ],
        "StandardButton" => vec![PropertyChange::new("kind", "ok".to_string())],
        _ => vec![],
    };

    ComponentInformation {
        name: name.to_string(),
        category: category.to_string(),
        is_global,
        is_builtin: false,
        is_std_widget: true,
        is_layout,
        fills_parent: is_layout,
        is_exported: true,
        defined_at: None,
        default_properties,
    }
}

fn exported_project_component_info(
    name: &str,
    is_global: bool,
    position: Position,
) -> ComponentInformation {
    ComponentInformation {
        name: name.to_string(),
        category: "User Defined".to_string(),
        is_global,
        is_builtin: false,
        is_std_widget: false,
        is_layout: false,
        fills_parent: false,
        is_exported: true,
        defined_at: Some(position),
        default_properties: vec![],
    }
}

#[cfg(feature = "preview-engine")]
fn file_local_component_info(
    name: &str,
    position: Position,
    is_global: bool,
) -> ComponentInformation {
    ComponentInformation {
        name: name.to_string(),
        category: "User Defined".to_string(),
        is_global,
        is_builtin: false,
        is_std_widget: false,
        is_layout: false,
        fills_parent: false,
        is_exported: false,
        defined_at: Some(position),
        default_properties: vec![],
    }
}

#[cfg(feature = "preview-engine")]
pub fn builtin_components(document_cache: &DocumentCache, result: &mut Vec<ComponentInformation>) {
    let registry = document_cache.global_type_registry();
    result.extend(registry.all_elements().iter().filter_map(|(name, ty)| match ty {
        ElementType::Builtin(b) if !b.is_internal => {
            let fills_parent =
                matches!(b.default_size_binding, DefaultSizeBinding::ExpandsToParentGeometry);
            Some(builtin_component_info(name, fills_parent))
        }
        _ => None,
    }));
}

/// Fill the result with all exported components that matches the given filter
pub fn all_exported_components(
    document_cache: &DocumentCache,
    filter: &mut dyn FnMut(&ComponentInformation) -> bool,
    result: &mut Vec<ComponentInformation>,
) {
    for url in document_cache.all_urls() {
        let Some(doc) = document_cache.get_document(&url) else { continue };
        let is_builtin = url.scheme() == "builtin";
        let is_std_widget = is_builtin && url.path().ends_with("/std-widgets.slint");

        for (exported_name, ty) in &*doc.exports {
            let Some(c) = ty.as_ref().left() else {
                continue;
            };

            let to_push = if is_std_widget && !exported_name.as_str().ends_with("Impl") {
                Some(std_widgets_info(exported_name.as_str(), c.is_global()))
            } else if !is_builtin {
                let offset =
                    c.node.as_ref().map(|n| n.text_range().start().into()).unwrap_or_default();
                Some(exported_project_component_info(
                    exported_name.as_str(),
                    c.is_global(),
                    Position { url: url.clone(), offset },
                ))
            } else {
                continue;
            };

            let Some(to_push) = to_push else {
                continue;
            };

            if !filter(&to_push) {
                continue;
            }

            result.push(to_push);
        }
    }

    result.sort_by(|a, b| a.name.cmp(&b.name));
    result.dedup_by(|a, b| a.name == b.name);
}

#[cfg(feature = "preview-engine")]
pub fn file_local_components(
    document_cache: &DocumentCache,
    url: &lsp_types::Url,
    result: &mut Vec<ComponentInformation>,
) {
    let Some(doc) = document_cache.get_document(url) else {
        return;
    };
    let exported_components =
        doc.exports.iter().filter_map(|(_, e)| e.as_ref().left()).cloned().collect::<Vec<_>>();
    for component in &*doc.inner_components {
        // component.exported_global_names is always empty since the pass populating it has not
        // run.
        if !exported_components.iter().any(|rc| std::rc::Rc::ptr_eq(rc, component)) {
            let offset =
                component.node.as_ref().map(|n| n.text_range().start().into()).unwrap_or_default();
            result.push(file_local_component_info(
                &component.id,
                Position { url: url.clone(), offset },
                component.is_global(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_component_catalog() {
        let (dc, _, _) = crate::language::test::loaded_document_cache(r#""#.to_string());

        let mut result = Default::default();
        builtin_components(&dc, &mut result);

        assert!(result.iter().all(|ci| !ci.is_std_widget));
        assert!(result.iter().all(|ci| ci.is_exported));
        assert!(result.iter().all(|ci| ci.is_builtin));
        assert!(result.iter().all(|ci| !ci.is_global));
        assert!(result.iter().any(|ci| &ci.name == "TouchArea"));
        assert!(!result.iter().any(|ci| &ci.name == "AboutSlint"));
        assert!(!result.iter().any(|ci| &ci.name == "ProgressIndicator"));
    }

    #[test]
    fn exported_component_catalog_std_widgets_only() {
        let (dc, _, _) = crate::language::test::loaded_document_cache(r#""#.to_string());

        let mut result = Default::default();
        all_exported_components(&dc, &mut |_| true, &mut result);

        assert!(result.iter().all(|ci| ci.is_std_widget));
        assert!(result.iter().all(|ci| ci.is_exported));
        assert!(result.iter().all(|ci| !ci.is_builtin));
        // assert!(result.iter().all(|ci| ci.is_global)); // mixed!
        assert!(!result.iter().any(|ci| &ci.name == "TouchArea"));
        assert!(result.iter().any(|ci| &ci.name == "AboutSlint"));
        assert!(result.iter().any(|ci| &ci.name == "ProgressIndicator"));
    }

    #[test]
    fn exported_component_catalog_filtered() {
        let (dc, _, _) = crate::language::test::loaded_document_cache(r#""#.to_string());

        let mut result = Default::default();
        all_exported_components(&dc, &mut |_| false, &mut result);

        assert!(result.is_empty());
    }

    #[test]
    fn exported_component_catalog_exported_component() {
        let baseline = {
            let (dc, _, _) = crate::language::test::loaded_document_cache(r#""#.to_string());

            let mut result = Default::default();
            all_exported_components(&dc, &mut |_| true, &mut result);
            result.len()
        };

        let (dc, _, _) = crate::language::test::loaded_document_cache(
            r#"export component Test1 {}"#.to_string(),
        );

        let mut result = Default::default();
        all_exported_components(&dc, &mut |_| true, &mut result);

        assert!(result.iter().any(|ci| &ci.name == "Test1"));
        assert!(!result.iter().any(|ci| &ci.name == "TouchArea"));
        assert!(result.iter().any(|ci| &ci.name == "AboutSlint"));
        assert!(result.iter().any(|ci| &ci.name == "ProgressIndicator"));
        assert_eq!(result.len(), baseline + 1);
    }

    #[test]
    fn local_component_catalog_one_unexported_component() {
        let (dc, url, _) =
            crate::language::test::loaded_document_cache(r#"component Test1 {}"#.to_string());

        let mut result = Default::default();
        file_local_components(&dc, &url, &mut result);
        assert!(result.is_empty()); // Test1 is implicitly exported!
    }

    #[test]
    fn local_component_catalog_two_unexported_components_without_export() {
        let (dc, url, _) = crate::language::test::loaded_document_cache(
            r#"
            component Test1 {}
            component Test2 {}"#
                .to_string(),
        );

        let mut result = Default::default();
        file_local_components(&dc, &url, &mut result);
        assert_eq!(result.len(), 1);

        let test1 = result.iter().find(|ci| &ci.name == "Test1").unwrap();
        assert!(!test1.is_std_widget);
        assert!(!test1.is_builtin);
        assert!(!test1.is_exported);
        assert!(!test1.is_global);
        assert!(!result.iter().any(|ci| &ci.name == "Test2")); // Test2 is implicitly exported
    }
    #[test]
    fn local_component_catalog_two_unexported_components_with_export() {
        let (dc, url, _) = crate::language::test::loaded_document_cache(
            r#"
            component Test1 {}
            export component Export1 {}
            component Test2 {}"#
                .to_string(),
        );

        let mut result = Default::default();
        file_local_components(&dc, &url, &mut result);
        assert_eq!(result.len(), 2);

        let test1 = result.iter().find(|ci| &ci.name == "Test1").unwrap();
        assert!(!test1.is_std_widget);
        assert!(!test1.is_builtin);
        assert!(!test1.is_exported);
        assert!(!test1.is_global);
        let test2 = result.iter().find(|ci| &ci.name == "Test2").unwrap();
        assert!(!test2.is_std_widget);
        assert!(!test2.is_builtin);
        assert!(!test2.is_exported);
        assert!(!test2.is_global);
        assert!(!result.iter().any(|ci| &ci.name == "Export1"));
    }
}
