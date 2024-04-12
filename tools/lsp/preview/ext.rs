// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use i_slint_compiler::layout;
use i_slint_core::lengths::{LogicalPoint, LogicalRect};

use crate::common;
use crate::preview::ui;
use crate::util;

use slint_interpreter::ComponentInstance;

pub trait ElementRcNodeExt {
    fn layout_kind(&self) -> crate::preview::ui::LayoutKind;

    /// Find all geometries for the given `ElementRcNode`
    fn geometries(
        &self,
        component_instance: &ComponentInstance,
    ) -> Vec<i_slint_core::lengths::LogicalRect>;

    /// Find the first geometry of `ElementRcNode` that includes the point `x`, `y`
    fn geometry_at(
        &self,
        component_instance: &ComponentInstance,
        position: LogicalPoint,
    ) -> Option<i_slint_core::lengths::LogicalRect>;

    /// Find the first geometry of ElementRcNode in `rect`
    fn geometry_in(
        &self,
        component_instance: &ComponentInstance,
        rect: &LogicalRect,
    ) -> Option<i_slint_core::lengths::LogicalRect>;

    /// Checks wether the given type is acceptable as a child
    fn accepts_child_type(
        &self,
        component_instance: &ComponentInstance,
        component_type: &str,
    ) -> bool;
}

impl ElementRcNodeExt for common::ElementRcNode {
    fn layout_kind(&self) -> crate::preview::ui::LayoutKind {
        self.with_element_debug(|_, l| match l {
            Some(layout::Layout::GridLayout(_)) => ui::LayoutKind::Grid,
            Some(layout::Layout::BoxLayout(layout::BoxLayout {
                orientation: layout::Orientation::Horizontal,
                ..
            })) => ui::LayoutKind::Horizontal,
            Some(layout::Layout::BoxLayout(layout::BoxLayout {
                orientation: layout::Orientation::Vertical,
                ..
            })) => ui::LayoutKind::Vertical,
            _ => ui::LayoutKind::None,
        })
    }

    fn geometries(
        &self,
        component_instance: &ComponentInstance,
    ) -> Vec<i_slint_core::lengths::LogicalRect> {
        component_instance.element_positions(&self.as_element())
    }

    fn geometry_at(
        &self,
        component_instance: &ComponentInstance,
        position: LogicalPoint,
    ) -> Option<i_slint_core::lengths::LogicalRect> {
        self.geometries(component_instance).iter().find(|g| g.contains(position)).cloned()
    }

    fn geometry_in(
        &self,
        component_instance: &ComponentInstance,
        rect: &LogicalRect,
    ) -> Option<i_slint_core::lengths::LogicalRect> {
        self.geometries(component_instance)
            .iter()
            .find(|g| rect.contains_rect(g))
            .cloned()
    }

    fn accepts_child_type(
        &self,
        component_instance: &ComponentInstance,
        component_type: &str,
    ) -> bool {
        let tl = component_instance.definition().type_loader();
        let (path, _) = self.path_and_offset();
        let Some(doc) = tl.get_document(&path) else {
            return false;
        };
        let Some(element_type) = self.with_element_node(|node| {
            util::lookup_current_element_type((node.clone()).into(), &doc.local_registry)
        }) else {
            return false;
        };

        self.layout_kind() != ui::LayoutKind::None
            || element_type.accepts_child_element(component_type, &doc.local_registry).is_ok()
    }
}

