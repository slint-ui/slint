// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::element_type::ElementType;
use i_slint_compiler::expression_types::Orientation;
use i_slint_compiler::layout::LayoutKind;
use i_slint_compiler::object_tree::{DEFAULT_SLOT_NAME, ElementRc};
use i_slint_core::lengths::{LogicalPoint, LogicalRect};

use crate::preview::ui;

use slint_interpreter::ComponentInstance;
use slint_interpreter::highlight::HighlightedRect;

pub trait ElementRcNodeExt {
    fn layout_kind(&self) -> crate::preview::ui::LayoutKind;

    /// The kind of layout that places the children of this element.
    /// Unlike `layout_kind`, this also covers components that put their `@children`
    /// into a layout, such as `GroupBox`.
    fn children_layout_kind(&self) -> crate::preview::ui::LayoutKind;

    /// Find all geometries for the given `ElementRcNode`
    fn geometries(&self, component_instance: &ComponentInstance) -> Vec<HighlightedRect>;

    /// Find the first geometry of `ElementRcNode` that includes the point `x`, `y`
    fn geometry_at(
        &self,
        component_instance: &ComponentInstance,
        position: LogicalPoint,
    ) -> Option<HighlightedRect>;

    /// Find the first geometry of ElementRcNode in `rect`
    fn geometry_in(
        &self,
        component_instance: &ComponentInstance,
        rect: &LogicalRect,
    ) -> Option<HighlightedRect>;
}

impl ElementRcNodeExt for i_slint_editor_preview::ElementRcNode {
    fn layout_kind(&self) -> crate::preview::ui::LayoutKind {
        layout_kind(&self.element)
    }

    fn children_layout_kind(&self) -> crate::preview::ui::LayoutKind {
        children_layout_kind(&self.element)
    }

    fn geometries(&self, component_instance: &ComponentInstance) -> Vec<HighlightedRect> {
        component_instance.component_positions(&self.location())
    }

    fn geometry_at(
        &self,
        component_instance: &ComponentInstance,
        position: LogicalPoint,
    ) -> Option<HighlightedRect> {
        self.geometries(component_instance).iter().find(|g| g.rect.contains(position)).cloned()
    }

    fn geometry_in(
        &self,
        component_instance: &ComponentInstance,
        rect: &LogicalRect,
    ) -> Option<HighlightedRect> {
        self.geometries(component_instance).iter().find(|g| rect.contains_rect(&g.rect)).cloned()
    }
}

fn layout_kind(element: &ElementRc) -> ui::LayoutKind {
    let builtin = element.borrow().builtin_type();
    match builtin.and_then(|builtin| LayoutKind::from_builtin_name(&builtin.name)) {
        Some(LayoutKind::Grid) => ui::LayoutKind::Grid,
        Some(LayoutKind::Flexbox) => ui::LayoutKind::Flexbox,
        Some(LayoutKind::Box(Orientation::Horizontal)) => ui::LayoutKind::Horizontal,
        Some(LayoutKind::Box(Orientation::Vertical)) => ui::LayoutKind::Vertical,
        None => ui::LayoutKind::None,
    }
}

fn children_layout_kind(element: &ElementRc) -> ui::LayoutKind {
    let kind = layout_kind(element);
    if kind != ui::LayoutKind::None {
        return kind;
    }
    let ElementType::Component(component) = element.borrow().base_type.clone() else {
        return ui::LayoutKind::None;
    };
    let insertion_parent = component
        .child_insertion_points
        .borrow()
        .get(DEFAULT_SLOT_NAME)
        .map(|cip| cip.parent.clone());
    insertion_parent.map_or(ui::LayoutKind::None, |parent| children_layout_kind(&parent))
}

#[cfg(test)]
mod tests {
    use super::ElementRcNodeExt;
    use crate::preview::ui;

    use i_slint_compiler::object_tree::ElementLocation;
    use i_slint_compiler::source_path::SourcePath;
    use i_slint_editor_preview::test;

    #[test]
    fn children_layout_kind_follows_children_placeholder() {
        let code = r#"import { GroupBox } from "std-widgets.slint";

component Column {
    VerticalLayout {
        @children
    }
}

component Plain {
    @children
}

export component Main {
    group := GroupBox { }
    column := Column { }
    plain := Plain { }
    row := HorizontalLayout { }
}
"#;
        let (document_cache, url, _) = test::loaded_document_cache(code.to_string());
        let path = SourcePath::from_url(&url);
        let element = |name: &str| {
            let offset = code.find(&format!("{name} := ")).unwrap() + name.len() + 4;
            let location = ElementLocation { path: path.clone(), offset: (offset as u32).into() };
            document_cache.element_node_at(&location).unwrap()
        };

        assert_eq!(element("group").layout_kind(), ui::LayoutKind::None);
        assert_eq!(element("group").children_layout_kind(), ui::LayoutKind::Grid);
        assert_eq!(element("column").children_layout_kind(), ui::LayoutKind::Vertical);
        assert_eq!(element("plain").children_layout_kind(), ui::LayoutKind::None);
        assert_eq!(element("row").children_layout_kind(), ui::LayoutKind::Horizontal);
    }
}
