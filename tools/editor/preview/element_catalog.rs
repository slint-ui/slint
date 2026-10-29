// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub use slint_editor::component_support::element_catalog::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::ui::ElementKind;
    use crate::preview::{DragItem, new_component_data};

    #[test]
    fn metadata_is_complete() {
        use i_slint_compiler::langtype::Type;
        use slint_interpreter::{Compiler, Value};

        i_slint_backend_testing::init_no_event_loop();
        let source = r#"
            import { ElementKind } from "api.slint";
            import { ElementVisuals } from "components/element-presentation.slint";
            export { ElementKind }
            export component MetadataProbe inherits Window {
                public pure function label(kind: ElementKind) -> string {
                    return ElementVisuals.for-kind(kind).label;
                }
            }
        "#;
        let mut compiler = Compiler::default();
        compiler.compiler_configuration(i_slint_core::InternalToken).experimental_features =
            i_slint_compiler::ExperimentalFeatures::all();
        let result = spin_on::spin_on(compiler.build_from_source(
            source.into(),
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/metadata-probe.slint"),
        ));
        let definition = result.component("MetadataProbe").unwrap_or_else(|| {
            panic!("metadata probe must compile: {:?}", result.diagnostics().collect::<Vec<_>>())
        });
        let instance = definition.create().unwrap();
        let enumeration = result
            .structs_and_enums(i_slint_core::InternalToken)
            .find_map(|ty| match ty {
                Type::Enumeration(enumeration) if enumeration.name == "ElementKind" => {
                    Some(enumeration)
                }
                _ => None,
            })
            .expect("ElementKind must be exported");
        let mut count = 0;
        for variant in &enumeration.values {
            if matches!(variant.as_str(), "none" | "component" | "widget") {
                continue;
            }
            count += 1;
            let entries: Vec<_> = PRIMITIVES
                .iter()
                .filter(|entry| {
                    format!("{:?}", entry.kind).to_lowercase() == variant.replace('-', "")
                })
                .collect();
            assert_eq!(entries.len(), 1, "{variant} must have exactly one catalog entry");
            let label = instance
                .invoke(
                    "label",
                    &[Value::EnumerationValue("ElementKind".into(), variant.to_string())],
                )
                .unwrap();
            assert_eq!(
                label,
                Value::String(entries[0].type_name.into()),
                "{variant} must have presentation metadata",
            );
        }
        assert_eq!(count, PRIMITIVES.len(), "catalog must contain only primitive kinds");
    }

    #[test]
    fn primitive_names_and_drop_payloads_share_the_catalog() {
        for (kind, name, group) in [
            (ElementKind::Image, "Image", "Visual"),
            (ElementKind::Rectangle, "Rectangle", "Visual"),
            (ElementKind::Text, "Text", "Visual"),
            (ElementKind::StyledText, "StyledText", "Visual"),
            (ElementKind::TouchArea, "TouchArea", "Input & interaction"),
            (ElementKind::Widget, "Button", "std-widgets"),
            (ElementKind::Widget, "CheckBox", "std-widgets"),
            (ElementKind::Widget, "LineEdit", "std-widgets"),
        ] {
            let entry = entry(name).unwrap();
            assert_eq!(entry.kind, kind);
            assert_eq!(entry.group.label(), group);
            assert_eq!(kind_for_type(&format!(" {name} ")), kind);
            assert_eq!(
                DragItem::try_from(new_component_data(name.into(), Default::default())).unwrap(),
                DragItem::NewComponent {
                    kind,
                    type_name: name.into(),
                    source_uri: Default::default()
                }
            );
        }
    }

    #[test]
    fn widgets_compile_with_default_properties_in_every_style() {
        use i_slint_editor_preview::component_catalog::all_exported_components;
        use i_slint_editor_preview::document_cache::CompilerConfiguration;

        for style in ["fluent", "material", "cupertino", "cosmic", "qt"] {
            let mut config = CompilerConfiguration::default();
            config.compiler_config.style = Some(style.into());
            let mut document_cache = i_slint_editor_preview::DocumentCache::new(config);
            spin_on::spin_on(document_cache.preload_builtins());
            let mut components = Vec::new();
            all_exported_components(&document_cache, &mut |_| true, &mut components);

            let elements = WIDGETS
                .iter()
                .map(|widget| {
                    let component = components
                        .iter()
                        .find(|component| {
                            component.is_std_widget && component.name == widget.type_name
                        })
                        .unwrap_or_else(|| panic!("{style} must export {}", widget.type_name));
                    assert_eq!(
                        *component,
                        i_slint_editor_preview::component_catalog::std_widgets_info(
                            widget.type_name,
                            false
                        ),
                    );
                    let properties = component
                        .default_properties
                        .iter()
                        .map(|property| format!("{}: {};", property.name, property.value))
                        .collect::<String>();
                    format!("{} {{ {properties} }}\n", widget.type_name)
                })
                .collect::<String>();
            let names = WIDGETS.iter().map(|widget| widget.type_name).collect::<Vec<_>>();
            let source = format!(
                "import {{ {} }} from \"std-widgets.slint\";\n\
                 export component Probe inherits Window {{\n{elements}}}",
                names.join(", "),
            );
            let mut compiler = slint_interpreter::Compiler::default();
            compiler.set_style(style.into());
            let result = spin_on::spin_on(
                compiler.build_from_source(source, std::path::PathBuf::from("probe.slint")),
            );
            assert!(
                !result.has_errors(),
                "{style}: {:?}",
                result.diagnostics().collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn palette_names_are_normalized_in_drag_payloads() {
        assert_eq!(
            DragItem::try_from(new_component_data(" Text ".into(), Default::default())).unwrap(),
            DragItem::NewComponent {
                kind: ElementKind::Text,
                type_name: "Text".into(),
                source_uri: Default::default()
            },
        );
    }

    #[test]
    fn project_drag_payloads_keep_the_definition_uri_and_reject_stale_entries() {
        use i_slint_editor_preview::component_catalog::{Position, std_widgets_info};
        let mut first = std_widgets_info("Button", false);
        first.name = "SharedCard".into();
        first.is_std_widget = false;
        first.defined_at =
            Some(Position::new(lsp_types::Url::parse("file:///one/Card.slint").unwrap(), 0.into()));
        let mut second = first.clone();
        second.defined_at =
            Some(Position::new(lsp_types::Url::parse("file:///two/Card.slint").unwrap(), 0.into()));
        let saved = crate::preview::PREVIEW_STATE.with_borrow_mut(|state| {
            std::mem::replace(&mut state.known_components, vec![first, second])
        });
        for uri in ["file:///one/Card.slint", "file:///two/Card.slint"] {
            assert_eq!(
                DragItem::try_from(new_component_data("SharedCard".into(), uri.into())).unwrap(),
                DragItem::NewComponent {
                    kind: ElementKind::Component,
                    type_name: "SharedCard".into(),
                    source_uri: uri.into()
                },
            );
            let component = crate::preview::palette_component("SharedCard", uri).unwrap();
            assert_eq!(component.defined_at.unwrap().url().as_str(), uri);
        }
        assert!(
            DragItem::try_from(new_component_data(
                "SharedCard".into(),
                "file:///missing/Card.slint".into()
            ))
            .is_err()
        );
        crate::preview::PREVIEW_STATE.with_borrow_mut(|state| {
            state.known_components = saved;
        });
        assert!(
            crate::preview::palette_component("SharedCard", "file:///one/Card.slint").is_none()
        );
    }

    #[test]
    fn fallback_kinds_cannot_create_palette_drops() {
        for name in [
            "",
            "Timer",
            "KeyBinding",
            "Path",
            "MoveTo",
            "Row",
            "PopupWindow",
            "Tooltip",
            "ContextMenuArea",
            "MenuBar",
            "Menu",
            "MenuItem",
            "MenuSeparator",
            "SystemTrayIcon",
            "Window",
            "Dialog",
            "CustomComponent",
        ] {
            assert_eq!(kind_for_type(name), ElementKind::Component);
            assert!(entry(name).is_none());
            assert!(
                DragItem::try_from(new_component_data(name.into(), Default::default())).is_err()
            );
        }
    }

    #[test]
    fn primitives_resolve_and_insert_with_canvas_geometry() {
        use crate::preview::drop_location;
        use i_slint_editor_preview::{
            component_catalog::builtin_components,
            editing::{PropertyChange, text_edit},
            test,
        };

        i_slint_backend_testing::init_no_event_loop();
        let source = "export component Main inherits Window { target := Rectangle { } }";
        let url = test::main_test_file_name().to_url().unwrap();
        let document_cache = test::recompile_test_with_sources(
            "fluent",
            std::collections::HashMap::from([(url.clone(), source.into())]),
            true,
            i_slint_compiler::ExperimentalFeatures::all(),
        );
        let mut components = Vec::new();
        builtin_components(&document_cache, &mut components);
        crate::preview::PREVIEW_STATE.with_borrow_mut(|state| {
            state.known_components = components;
            state.source_code.insert(
                url.clone(),
                crate::preview::SourceCodeCacheEntry { version: Some(1), code: source.into() },
            );
        });
        let target = document_cache
            .element_at_offset(&url, (source.find("Rectangle").unwrap() as u32).into())
            .unwrap();
        let drop = drop_location::DropInformation {
            insert_info: drop_location::insert_position_at_end(&target).unwrap(),
            target_element_node: target,
            drop_mark: None,
            child_index: 0,
        };
        for entry in PRIMITIVES {
            let component =
                crate::preview::palette_component(entry.type_name, "").unwrap_or_else(|| {
                    panic!("{} must resolve in the built-in catalog", entry.type_name)
                });
            let mut properties = vec![
                PropertyChange::new("x", "20px".into()),
                PropertyChange::new("y", "24px".into()),
                PropertyChange::new("width", "160px".into()),
                PropertyChange::new("height", "96px".into()),
            ];
            drop_location::extend_with_new_properties(
                &mut properties,
                &component.default_properties,
                drop_location::visual_properties_for_drop(&component),
            );
            let (edit, _) = drop_location::create_drop_element_workspace_edit_with_properties(
                &document_cache,
                &component,
                &drop,
                &properties,
            )
            .unwrap();
            let edited = text_edit::apply_workspace_edit(&document_cache, &edit).unwrap();
            assert_eq!(
                drop_location::workspace_edit_compiles(&document_cache, &edit),
                crate::preview::CompilationResult::ChangeCompiles,
                "{}: {}",
                entry.type_name,
                edited[0].contents
            );
        }
        crate::preview::PREVIEW_STATE.with_borrow_mut(|state| {
            state.source_code.clear();
            state.known_components.clear();
        });
    }
}
