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
            (ElementKind::Widget, "Button", "Widgets"),
            (ElementKind::Widget, "CheckBox", "Widgets"),
            (ElementKind::Widget, "LineEdit", "Widgets"),
        ] {
            let entry = entry(name).unwrap();
            assert_eq!(entry.kind, kind);
            assert_eq!(entry.group.label(), group);
            assert_eq!(kind_for_type(&format!(" {name} ")), kind);
            assert_eq!(
                DragItem::try_from(new_component_data(name.into())).unwrap(),
                DragItem::NewComponent { kind, type_name: name }
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
    fn fallback_kinds_cannot_create_palette_drops() {
        for name in ["", "Window", "CustomComponent"] {
            assert_eq!(kind_for_type(name), ElementKind::Component);
            assert!(entry(name).is_none());
            assert!(DragItem::try_from(new_component_data(name.into())).is_err());
        }
    }
}
