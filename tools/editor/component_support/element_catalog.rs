// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
    Controls,
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Self::Visual => "Visual",
            Self::InputInteraction => "Input & interaction",
            Self::Controls => "Controls",
        }
    }
}

pub const GROUPS: [Group; 3] = [Group::Visual, Group::InputInteraction, Group::Controls];

pub struct PaletteElement {
    pub kind: ElementKind,
    pub type_name: &'static str,
    pub label: &'static str,
    pub group: Group,
}

pub const ELEMENTS: [PaletteElement; 7] = [
    PaletteElement {
        kind: ElementKind::Rectangle,
        type_name: "Rectangle",
        label: "Rectangle",
        group: Group::Visual,
    },
    PaletteElement {
        kind: ElementKind::Text,
        type_name: "Text",
        label: "Text",
        group: Group::Visual,
    },
    PaletteElement {
        kind: ElementKind::Image,
        type_name: "Image",
        label: "Image",
        group: Group::Visual,
    },
    PaletteElement {
        kind: ElementKind::TouchArea,
        type_name: "TouchArea",
        label: "TouchArea",
        group: Group::InputInteraction,
    },
    PaletteElement {
        kind: ElementKind::Button,
        type_name: "ControlButton",
        label: "Button",
        group: Group::Controls,
    },
    PaletteElement {
        kind: ElementKind::Slider,
        type_name: "ControlSlider",
        label: "Slider",
        group: Group::Controls,
    },
    PaletteElement {
        kind: ElementKind::ComboBox,
        type_name: "ControlComboBox",
        label: "ComboBox",
        group: Group::Controls,
    },
];

pub fn element(kind: ElementKind) -> Option<&'static PaletteElement> {
    ELEMENTS.iter().find(|entry| entry.kind == kind)
}

pub fn kind_for_type(type_name: &str) -> ElementKind {
    ELEMENTS
        .iter()
        .find(|entry| entry.type_name == type_name.trim())
        .map_or(ElementKind::Component, |entry| entry.kind)
}
