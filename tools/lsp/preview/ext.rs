// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::layout;
use i_slint_core::lengths::{LogicalPoint, LogicalRect};

use crate::common;
use crate::preview::ui;

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
}

impl ElementRcNodeExt for common::ElementRcNode {
    fn layout_kind(&self) -> crate::preview::ui::LayoutKind {
        self.with_element_debug(|di| match &di.layout {
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
        component_instance.element_positions(self.as_element())
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
        self.geometries(component_instance).iter().find(|g| rect.contains_rect(g)).cloned()
    }
}
