// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// This catalog owns primitive identity and grouping. Presentation belongs in
// ui/components/element-presentation.slint. Add each primitive to both sources;
// metadata_is_complete checks every ElementKind except none and component.

use crate::ui::ElementKind;
use std::hash::{Hash, Hasher};

impl Eq for ElementKind {}
impl Hash for ElementKind {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Visual,
    InputInteraction,
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Self::Visual => "Visual",
            Self::InputInteraction => "Input & interaction",
        }
    }
}

pub const GROUPS: [Group; 2] = [Group::Visual, Group::InputInteraction];

pub struct Primitive {
    pub kind: ElementKind,
    pub type_name: &'static str,
    pub group: Group,
}

pub const PRIMITIVES: [Primitive; 4] = [
    Primitive { kind: ElementKind::Rectangle, type_name: "Rectangle", group: Group::Visual },
    Primitive { kind: ElementKind::Text, type_name: "Text", group: Group::Visual },
    Primitive { kind: ElementKind::Image, type_name: "Image", group: Group::Visual },
    Primitive {
        kind: ElementKind::TouchArea,
        type_name: "TouchArea",
        group: Group::InputInteraction,
    },
];

pub fn primitive(kind: ElementKind) -> Option<&'static Primitive> {
    PRIMITIVES.iter().find(|entry| entry.kind == kind)
}

pub fn kind_for_type(type_name: &str) -> ElementKind {
    PRIMITIVES
        .iter()
        .find(|entry| entry.type_name == type_name.trim())
        .map_or(ElementKind::Component, |entry| entry.kind)
}
