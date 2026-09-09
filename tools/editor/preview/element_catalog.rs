// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
