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

/// `ElementHandle`` wraps an existing element in a Slint UI. An ElementHandle does not keep
/// the corresponding element in the UI alive. Use [`Self::is_valid()`] to verify that
/// it is still alive.
///
/// Obtain instances of `ElementHandle` by querying your application through
/// [`Self::find_by_accessible_label()`].
#[derive(Clone)]
pub struct ElementHandle(ItemWeak);

impl ElementHandle {
    /// Returns true if the element still exists in the in UI and is valid to access; false otherwise.
    pub fn is_valid(&self) -> bool {
        self.0.upgrade().is_some()
    }

    /// This function searches through the entire tree of elements of `component`, looks for
    /// elements that have a `accessible-label` property with the provided value `label`,
    /// and returns an iterator over the found elements.
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

    /// Invokes the default accessible action on the element. For example a `MyButton` element might declare
    /// an accessible default action that simulates a click, as in the following example:
    ///
    /// ```slint,no-preview
    /// component MyButton {
    ///     // ...
    ///     callback clicked();
    ///     in property <string> text;
    ///
    ///     TouchArea {
    ///         clicked => { root.clicked() }
    ///     }
    ///     accessible-role: button;
    ///     accessible-label: self.text;
    ///     accessible-action-default => { self.clicked(); }
    /// }
    /// ```
    pub fn invoke_accessible_default_action(&self) {
        if let Some(item) = self.0.upgrade() {
            item.accessible_action(&AccessibilityAction::Default)
        }
    }

    /// Returns the value of the element's `accessible-value` property, if present.
    pub fn accessible_value(&self) -> Option<SharedString> {
        self.0
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Value))
    }

    /// Sets the value of the element's `accessible-value` property. Note that you can only set this
    /// property if it is declared in your Slint code.
    pub fn set_accessible_value(&self, value: impl Into<SharedString>) {
        if let Some(item) = self.0.upgrade() {
            item.accessible_action(&AccessibilityAction::SetValue(value.into()))
        }
    }

    /// Returns the value of the `accessible-label` property, if present.
    pub fn accessible_label(&self) -> Option<SharedString> {
        self.0
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Label))
    }

    /// Returns the value of the `accessible-description` property, if present
    pub fn accessible_description(&self) -> Option<SharedString> {
        self.0
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Description))
    }

    /// Returns the value of the `accessible-checked` property, if present
    pub fn accessible_checked(&self) -> Option<bool> {
        self.0
            .upgrade()
            .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Checked))
            .and_then(|item| item.parse().ok())
    }

    /// Returns the size of the element in logical pixels. This corresponds to the value of the `width` and
    /// `height` properties in Slint code. Returns a zero size if the element is not valid.
    pub fn size(&self) -> i_slint_core::api::LogicalSize {
        self.0
            .upgrade()
            .map(|item| {
                let g = item.geometry();
                i_slint_core::lengths::logical_size_to_api(g.size)
            })
            .unwrap_or_default()
    }

    /// Returns the position of the element within the entire window. This corresponds to the value of the
    /// `absolute-position` property in Slint code. Returns a zero position if the element is not valid.
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
