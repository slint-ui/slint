// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore descr rfind unindented

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

use crate::common::{ComponentInformation, DocumentCache, Position, PropertyChange};
#[cfg(feature = "preview-engine")]
use i_slint_compiler::langtype::ElementType;

#[cfg(feature = "preview-engine")]
fn builtin_component_info(name: &str) -> ComponentInformation {
    let is_layout = matches!(name, "GridLayout" | "HorizontalLayout" | "VerticalLayout");
    let is_interactive =
        matches!(name, "Flickable" | "FocusScope" | "SwipeGestureHandler" | "TouchArea");

    let default_properties = match name {
        "Text" | "TextInput" => vec![PropertyChange::new("text", format!("\"{name}\""))],
        "Image" => vec![PropertyChange::new("source", "@image-url(\"EDIT_ME.png\")".to_string())],
        _ => vec![],
    };

    let component = {
        if ["Flickable", "SwipeGestureHandler", "TouchArea"].contains(&name) {
            "Gestures"
        } else if ["FocusScope", "TextInput"].contains(&name) {
            "Keyboard Input"
        } else if name.ends_with("Layout") {
            "Basic Layouts"
        } else if ["Dialog", "PopupWindow", "Window"].contains(&name) {
            "Window"
        } else {
            "Basic Elements"
        }
    };

    ComponentInformation {
        name: name.to_string(),
        category: component.to_string(),
        is_global: false,
        is_builtin: true,
        is_std_widget: false,
        is_layout,
        is_interactive,
        is_exported: true,
        defined_at: None,
        default_properties,
    }
}

fn std_widgets_info(name: &str, is_global: bool) -> ComponentInformation {
    let is_layout = matches!(name, "GridBox" | "HorizontalBox" | "VerticalBox");

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
        category: "Std-Widgets".to_string(),
        is_global,
        is_builtin: false,
        is_std_widget: true,
        is_layout,
        is_interactive: false,
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
        category: position.url.to_string(),
        is_global,
        is_builtin: false,
        is_std_widget: false,
        is_layout: false,
        is_interactive: false,
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
        category: position.url.to_string(),
        is_global,
        is_builtin: false,
        is_std_widget: false,
        is_layout: false,
        is_interactive: false,
        is_exported: false,
        defined_at: Some(position),
        default_properties: vec![],
    }
}

#[cfg(feature = "preview-engine")]
pub fn builtin_components(document_cache: &DocumentCache, result: &mut Vec<ComponentInformation>) {
    let registry = document_cache.global_type_registry();
    result.extend(registry.all_elements().iter().filter_map(|(name, ty)| match ty {
        ElementType::Builtin(b) if !b.is_internal && !b.is_non_item_type => {
            let info = builtin_component_info(name);
            (info.category != "Window").then_some(info)
        }
        _ => None,
    }));
}

fn libraryize_url(document_cache: &DocumentCache, url: lsp_types::Url) -> lsp_types::Url {
    let url_path = i_slint_compiler::pathutils::clean_path(&url.to_file_path().unwrap_or_default());
    if let Some((library_name, library_path)) = document_cache
        .compiler_configuration()
        .library_paths
        .iter()
        .map(|(n, p)| (n, i_slint_compiler::pathutils::clean_path(p)))
        .find(|(_, path)| url_path.starts_with(path) || url_path == **path)
    {
        if url_path == library_path {
            let mut url = url.clone();
            url.set_path(&format!("/@{library_name}"));
            url
        } else if let Ok(short_path) = url_path.strip_prefix(library_path) {
            let short_path = short_path.to_string_lossy();
            let mut url = url.clone();
            url.set_path(&format!("/@{library_name}/{short_path}"));
            url
        } else {
            url
        }
    } else {
        url
    }
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

        let url = libraryize_url(document_cache, url);

        for (exported_name, ty) in &*doc.exports {
            let Some(c) = ty.as_ref().left() else {
                continue;
            };

            let to_push = if is_std_widget
                && !exported_name.as_str().ends_with("Impl")
                && !exported_name.as_str().ends_with("Popup")
                && exported_name.as_str() != "PopupWindow"
            {
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
}

#[cfg(feature = "preview-engine")]
pub fn file_local_components(
    document_cache: &DocumentCache,
    url: &lsp_types::Url,
    result: &mut Vec<ComponentInformation>,
) {
    let library_url = libraryize_url(document_cache, url.clone());
    if library_url.path() != url.path() {
        return;
    }
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

#[cfg(all(test, feature = "preview-engine"))]
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
        assert!(result
            .iter()
            .any(|ci| &ci.name == "TouchArea" && ci.is_interactive && !ci.is_layout));
        assert!(result
            .iter()
            .any(|ci| &ci.name == "HorizontalLayout" && !ci.is_interactive && ci.is_layout));
        assert!(!result.iter().any(|ci| &ci.name == "AboutSlint"));
        assert!(!result.iter().any(|ci| &ci.name == "ProgressIndicator"));
        assert!(!result.iter().any(|ci| &ci.name == "Timer"));
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
