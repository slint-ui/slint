// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore descr rfind unindented

use crate::common::ComponentInformation;
use crate::language::DocumentCache;

use i_slint_compiler::langtype::ElementType;

use std::path::Path;

fn builtin_component_info(name: &str) -> ComponentInformation {
    let category = {
        match name {
            "GridLayout" | "HorizontalLayout" | "VerticalLayout" => "Layout",
            "Dialog" | "Window" | "PopupWindow" => "Window Management",
            "FocusScope" | "TouchArea" => "Event Handling",
            "Text" => "Text Handling",
            _ => "Primitives",
        }
    };

    ComponentInformation {
        name: name.to_string(),
        category: category.to_string(),
        is_global: false,
        is_builtin: true,
        is_std_widget: false,
        is_exported: true,
        file: None,
        offset: 0,
    }
}

fn std_widgets_info(name: &str, is_global: bool) -> ComponentInformation {
    let category = {
        match name {
            "GridBox" | "HorizontalBox" | "VerticalBox" => "Layout",
            "LineEdit" | "TextEdit" => "Text Handling",
            "Button" | "CheckBox" | "ComboBox" | "Slider" | "SpinBox" | "Switch" => "Input",
            "ProgressIndicator" | "Spinner" => "Status",
            "ListView" | "StandardListView" | "StandardTableView" => "Views",
            _ => "Widgets",
        }
    };

    ComponentInformation {
        name: name.to_string(),
        category: category.to_string(),
        is_global,
        is_builtin: false,
        is_std_widget: true,
        is_exported: true,
        file: None,
        offset: 0,
    }
}

fn exported_project_component_info(
    name: &str,
    is_global: bool,
    file: String,
    offset: u32,
) -> ComponentInformation {
    ComponentInformation {
        name: name.to_string(),
        category: "User Defined".to_string(),
        is_global,
        is_builtin: false,
        is_std_widget: false,
        is_exported: true,
        file: Some(file),
        offset,
    }
}

fn file_local_component_info(name: &str, file: String, offset: u32) -> ComponentInformation {
    ComponentInformation {
        name: name.to_string(),
        category: "User Defined".to_string(),
        is_global: false,
        is_builtin: false,
        is_std_widget: false,
        is_exported: false,
        file: Some(file),
        offset,
    }
}

#[allow(unused)]
pub fn builtin_components(document_cache: &DocumentCache, result: &mut Vec<ComponentInformation>) {
    let registry = document_cache.documents.global_type_registry.borrow();
    result.extend(registry.all_elements().iter().filter_map(|(name, ty)| {
        matches!(ty, ElementType::Builtin(b) if !b.is_internal)
            .then_some(builtin_component_info(name))
    }));
}

pub fn all_exported_components(
    document_cache: &DocumentCache,
    filter: &mut dyn FnMut(&ComponentInformation) -> bool,
    result: &mut Vec<ComponentInformation>,
) {
    for file in document_cache.documents.all_files() {
        let Some(doc) = document_cache.documents.get_document(file) else { continue };
        let is_builtin = file.starts_with("builtin:/");
        let is_std_widget = is_builtin
            && file.file_name().map(|f| f.to_str() == Some("std-widgets.slint")).unwrap_or(false);

        for (exported_name, ty) in &*doc.exports {
            let Some(c) = ty.as_ref().left() else {
                continue;
            };

            let to_push = if is_std_widget && !exported_name.as_str().ends_with("Impl") {
                Some(std_widgets_info(exported_name.as_str(), c.is_global()))
            } else if !is_builtin {
                let file = file.to_string_lossy().to_string();
                let offset =
                    c.node.as_ref().map(|n| n.text_range().start().into()).unwrap_or_default();
                Some(exported_project_component_info(
                    exported_name.as_str(),
                    c.is_global(),
                    file,
                    offset,
                ))
            } else {
                None
            };

            let Some(to_push) = to_push else {
                continue;
            };

            if filter(&to_push) {
                continue;
            }

            result.push(to_push);
        }
    }
}

#[allow(unused)]
pub fn file_local_components(
    document_cache: &DocumentCache,
    file: &Path,
    result: &mut Vec<ComponentInformation>,
) {
    let Some(doc) = document_cache.documents.get_document(file) else { return };
    for component in &*doc.inner_components {
        if component.is_global() {
            continue;
        };

        if component.exported_global_names.borrow().is_empty() {
            let offset =
                component.node.as_ref().map(|n| n.text_range().start().into()).unwrap_or_default();
            result.push(file_local_component_info(
                &component.id,
                file.to_string_lossy().to_string(),
                offset,
            ));
        }
    }
}
