// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// This catalog owns primitive identity and grouping. Presentation belongs in
// ui/components/element-presentation.slint. Add each primitive to both sources;
// metadata_is_complete checks every ElementKind except none and component.

use super::ui::ElementKind;
use std::hash::{Hash, Hasher};

impl Eq for ElementKind {}
impl Hash for ElementKind {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Group {
    Visual,
    InputInteraction,
}

impl Group {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Visual => "Visual",
            Self::InputInteraction => "Input & interaction",
        }
    }
}

pub(super) const GROUPS: [Group; 2] = [Group::Visual, Group::InputInteraction];

pub(super) struct Primitive {
    pub kind: ElementKind,
    pub type_name: &'static str,
    pub group: Group,
}

pub(super) const PRIMITIVES: [Primitive; 4] = [
    Primitive { kind: ElementKind::Rectangle, type_name: "Rectangle", group: Group::Visual },
    Primitive { kind: ElementKind::Text, type_name: "Text", group: Group::Visual },
    Primitive { kind: ElementKind::Image, type_name: "Image", group: Group::Visual },
    Primitive {
        kind: ElementKind::TouchArea,
        type_name: "TouchArea",
        group: Group::InputInteraction,
    },
];

pub(super) fn primitive(kind: ElementKind) -> Option<&'static Primitive> {
    PRIMITIVES.iter().find(|entry| entry.kind == kind)
}

pub(super) fn kind_for_type(type_name: &str) -> ElementKind {
    PRIMITIVES
        .iter()
        .find(|entry| entry.type_name == type_name.trim())
        .map_or(ElementKind::Component, |entry| entry.kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::{DragItem, new_component_data_for_kind};

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
                    return ElementVisuals.for-kind(kind).drag-label;
                }
            }
        "#;
        let result = spin_on::spin_on(Compiler::default().build_from_source(
            source.into(),
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/metadata-probe.slint"),
        ));
        let definition = result.component("MetadataProbe").expect("metadata probe must compile");
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
            if matches!(variant.as_str(), "none" | "component") {
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
                Value::String(format!("{} drag preview", entries[0].type_name).into()),
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
            (ElementKind::TouchArea, "TouchArea", "Input & interaction"),
        ] {
            let entry = primitive(kind).unwrap();
            assert_eq!(entry.type_name, name);
            assert_eq!(entry.group.label(), group);
            assert_eq!(kind_for_type(&format!(" {name} ")), kind);
            assert_eq!(
                DragItem::try_from(new_component_data_for_kind(kind)).unwrap(),
                DragItem::NewComponent { kind }
            );
        }
    }

    #[test]
    fn fallback_kinds_cannot_create_palette_drops() {
        for name in ["", "Window", "CustomComponent"] {
            assert_eq!(kind_for_type(name), ElementKind::Component);
        }
        for kind in [ElementKind::None, ElementKind::Component] {
            assert!(primitive(kind).is_none());
            assert!(DragItem::try_from(new_component_data_for_kind(kind)).is_err());
        }
    }
}
