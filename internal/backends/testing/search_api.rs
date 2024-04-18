// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use i_slint_core::accessibility::{AccessibilityAction, AccessibleStringProperty};
use i_slint_core::item_tree::{ItemTreeRc, ItemVisitorResult, TraversalOrder};
use i_slint_core::items::ItemRc;
use i_slint_core::window::WindowInner;
use i_slint_core::SharedString;

fn search_item(item_tree: &ItemTreeRc, mut filter: impl FnMut(&ItemRc) -> bool) -> Vec<ItemRc> {
    let mut result = vec![];
    i_slint_core::item_tree::visit_items(
        item_tree,
        TraversalOrder::BackToFront,
        |parent_tree, _, index, _| {
            let item_rc = ItemRc::new(parent_tree.clone(), index);
            if filter(&item_rc) {
                result.push(item_rc);
            }
            ItemVisitorResult::Continue(())
        },
        (),
    );
    result
}

pub struct ElementHandle(ItemRc);

impl ElementHandle {
    pub fn find_by_accessible_label(
        component: &impl i_slint_core::api::ComponentHandle,
        label: &str,
    ) -> impl Iterator<Item = Self> {
        // dirty way to get the ItemTreeRc:
        let item_tree = WindowInner::from_pub(component.window()).component();
        let result = search_item(&item_tree, |item| {
            item.accessible_string_property(AccessibleStringProperty::Label) == label
        });
        result.into_iter().map(|x| ElementHandle(x))
    }

    pub fn invoke_default_action(&self) {
        self.0.accessible_action(&AccessibilityAction::Default)
    }

    pub fn accessible_value(&self) -> SharedString {
        self.0.accessible_string_property(AccessibleStringProperty::Value)
    }

    pub fn set_accessible_value(&self, value: SharedString) {
        self.0.accessible_action(&AccessibilityAction::SetValue(value))
    }

    pub fn accessible_label(&self) -> SharedString {
        self.0.accessible_string_property(AccessibleStringProperty::Label)
    }

    pub fn size(&self) -> i_slint_core::api::LogicalSize {
        let g = self.0.geometry();
        i_slint_core::lengths::logical_size_to_api(g.size)
    }

    pub fn absolute_position(&self) -> i_slint_core::api::LogicalPosition {
        let g = self.0.geometry();
        let p = self.0.map_to_window(g.origin);
        i_slint_core::lengths::logical_position_to_api(p)
    }
}
