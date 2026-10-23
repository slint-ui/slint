// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// This catalog owns palette entry identity and grouping. Presentation belongs in
// ui/components/element-presentation.slint. Add each primitive to both sources;
// metadata_is_complete checks every ElementKind except none, component, and widget.

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
    Widgets,
}

impl Group {
    pub fn label(self) -> String {
        match self {
            Self::Visual => tr::tr!("Visual"),
            Self::InputInteraction => tr::tr!("Input & interaction"),
            Self::Widgets => tr::tr!("Widgets"),
        }
    }
}

pub const GROUPS: [Group; 3] = [Group::Visual, Group::InputInteraction, Group::Widgets];

pub struct Entry {
    pub kind: ElementKind,
    pub type_name: &'static str,
    pub group: Group,
}

pub const PRIMITIVES: [Entry; 5] = [
    Entry { kind: ElementKind::Rectangle, type_name: "Rectangle", group: Group::Visual },
    Entry { kind: ElementKind::Text, type_name: "Text", group: Group::Visual },
    Entry { kind: ElementKind::StyledText, type_name: "StyledText", group: Group::Visual },
    Entry { kind: ElementKind::Image, type_name: "Image", group: Group::Visual },
    Entry { kind: ElementKind::TouchArea, type_name: "TouchArea", group: Group::InputInteraction },
];

pub const WIDGETS: [Entry; 22] = [
    widget("AboutSlint"),
    widget("Button"),
    widget("CheckBox"),
    widget("ComboBox"),
    widget("GridBox"),
    widget("GroupBox"),
    widget("HorizontalBox"),
    widget("LineEdit"),
    widget("ListView"),
    widget("ProgressIndicator"),
    widget("RadioGroup"),
    widget("ScrollView"),
    widget("Slider"),
    widget("SpinBox"),
    widget("Spinner"),
    widget("StandardButton"),
    widget("StandardListView"),
    widget("StandardTableView"),
    widget("Switch"),
    widget("TabWidget"),
    widget("TextEdit"),
    widget("VerticalBox"),
];

const fn widget(type_name: &'static str) -> Entry {
    Entry { kind: ElementKind::Widget, type_name, group: Group::Widgets }
}

pub fn entries() -> impl Iterator<Item = &'static Entry> {
    PRIMITIVES.iter().chain(WIDGETS.iter())
}

pub fn entry(type_name: &str) -> Option<&'static Entry> {
    entries().find(|entry| entry.type_name == type_name.trim())
}

pub fn kind_for_type(type_name: &str) -> ElementKind {
    entry(type_name).map_or(ElementKind::Component, |entry| entry.kind)
}
