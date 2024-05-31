// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Assign the Element::item_index on each elements

use std::rc::Rc;

use crate::object_tree::{Component, ElementRc};

/// The item indices are generated and assigned to the ElementRc's item_index for each
/// element in the component. The indices are local to the component.
///
/// For sub-components the structure of the tree becomes a little complicated, which is best
/// illustrated using an example:
/// ```slint
/// SubCompo := Rectangle { Image {} }
/// MainCompo := Window {
///     TouchArea {}
///     SubCompo {}
///     Text {
///         Path {}
///     }
/// }
/// ```
/// The item tree for `MainCompo` with its local indices is as follows:
/// 0: Window (children: 3, children_offset: 1, parent_index: 0)
/// 1: TouchArea (children: 0, children_offset: X, parent_index: 0)
/// 2: Rectangle (children: 1, children_offset: 4, parent_index: 0) // SubCompo's root element
/// 3: Text      (children: 1, children_offset: 5, parent_index: 0)
/// 4: Image     (children: 0, children_offset: X, parent_index: 2) // SubCompo's child(ren)
/// 5: Path      (children: 0, children_offset: X, parent_index: 3)
pub fn generate_item_indices(component: &Rc<Component>) {
    // In order to create the local indices like in the above example (0-5) we use the same function
    // that is also used for building the item tree. It recurses into all sub-components, but we skip
    // them, by checking if the SubComponentState is true.
    // The immediate children of for example the Window element are emitted first. When a sub-component
    // is encountered (like `SubCompo`) the root element is emitted, but the children later. This simulates
    // the structure as if the SubCompo was inlined, but it also means that the local item indices must be
    // counted continuously.
    crate::generator::build_item_tree(component, &false, &mut Helper { current_item_index: 0 });
    for p in component.popup_windows.borrow().iter() {
        generate_item_indices(&p.component)
    }
}

struct Helper {
    current_item_index: u32,
}
impl crate::generator::ItemTreeBuilder for Helper {
    // true when not at the root
    type SubComponentState = bool;

    fn push_repeated_item(
        &mut self,
        item: &ElementRc,
        _repeater_count: u32,
        _parent_index: u32,
        component_state: &Self::SubComponentState,
    ) {
        if !component_state {
            item.borrow().item_index.set(self.current_item_index).unwrap();
            if let crate::langtype::ElementType::Component(c) = &item.borrow().base_type {
                generate_item_indices(c);
            }
        }
        self.current_item_index += 1;
    }

    fn push_component_placeholder_item(
        &mut self,
        item: &crate::object_tree::ElementRc,
        _container_count: u32,
        _parent_index: u32,
        component_state: &Self::SubComponentState,
    ) {
        if !component_state {
            item.borrow().item_index.set(self.current_item_index).unwrap();
        }
        self.current_item_index += 1;
    }

    fn push_native_item(
        &mut self,
        item: &ElementRc,
        children_offset: u32,
        _parent_index: u32,
        component_state: &Self::SubComponentState,
    ) {
        if !component_state {
            item.borrow().item_index.set(self.current_item_index).unwrap();
            item.borrow().item_index_of_first_children.set(children_offset as _).unwrap();
        }
        self.current_item_index += 1;
    }

    fn enter_component(
        &mut self,
        item: &ElementRc,
        _sub_component: &Rc<Component>,
        children_offset: u32,
        component_state: &Self::SubComponentState,
    ) -> Self::SubComponentState {
        if !component_state {
            item.borrow().item_index.set(self.current_item_index).unwrap();
            item.borrow().item_index_of_first_children.set(children_offset as _).unwrap();
        }
        true
    }

    fn enter_component_children(
        &mut self,
        _item: &ElementRc,
        _repeater_count: u32,
        _component_state: &Self::SubComponentState,
        _sub_component_state: &Self::SubComponentState,
    ) {
    }
}
