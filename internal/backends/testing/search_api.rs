// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use i_slint_core::accessibility::{AccessibilityAction, AccessibleStringProperty};
use i_slint_core::item_tree::{ItemTreeRc, ItemVisitorResult, ItemWeak, TraversalOrder};
use i_slint_core::items::ItemRc;
use i_slint_core::window::WindowInner;
use i_slint_core::{SharedString, SharedVector};

pub(crate) fn search_item(
    item_tree: &ItemTreeRc,
    mut filter: impl FnMut(&ItemRc) -> bool,
) -> SharedVector<ItemWeak> {
    let mut result = SharedVector::default();
    i_slint_core::item_tree::visit_items(
        item_tree,
        TraversalOrder::BackToFront,
        |parent_tree, _, index, _| {
            let item_rc = ItemRc::new(parent_tree.clone(), index);
            if filter(&item_rc) {
                result.push(item_rc.downgrade());
            }
            ItemVisitorResult::Continue(())
        },
        (),
    );
    result
}

pub struct ElementHandle(ItemWeak);

impl ElementHandle {
    pub fn is_valid(&self) -> bool {
        self.0.upgrade().is_some()
    }

    pub fn find_by_accessible_label(
        component: &impl i_slint_core::api::ComponentHandle,
        label: &str,
    ) -> impl Iterator<Item = Self> {
        // dirty way to get the ItemTreeRc:
        let item_tree = WindowInner::from_pub(component.window()).component();
        let result = search_item(&item_tree, |item| {
            item.accessible_string_property(AccessibleStringProperty::Label)
                .is_some_and(|x| x == label)
        });
        result.into_iter().map(|x| ElementHandle(x))
    }

    pub fn invoke_default_action(&self) {
        if let Some(item) = self.0.upgrade() {
            item.accessible_action(&AccessibilityAction::Default)
        }
    }

    pub fn accessible_value(&self) -> Option<SharedString> {
        self.0
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Value))
    }

    pub fn set_accessible_value(&self, value: impl Into<SharedString>) {
        if let Some(item) = self.0.upgrade() {
            item.accessible_action(&AccessibilityAction::SetValue(value.into()))
        }
    }

    pub fn accessible_label(&self) -> Option<SharedString> {
        self.0
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Label))
    }

    pub fn size(&self) -> i_slint_core::api::LogicalSize {
        self.0
            .upgrade()
            .map(|item| {
                let g = item.geometry();
                i_slint_core::lengths::logical_size_to_api(g.size)
            })
            .unwrap_or_default()
    }

    pub fn absolute_position(&self) -> i_slint_core::api::LogicalPosition {
        self.0
            .upgrade()
            .map(|item| {
                let g = item.geometry();
                let p = item.map_to_window(g.origin);
                i_slint_core::lengths::logical_position_to_api(p)
            })
            .unwrap_or_default()
    }
}
