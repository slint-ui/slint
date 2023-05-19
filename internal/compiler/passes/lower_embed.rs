// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Passe that transform the PopupWindow element into a component

use crate::diagnostics::BuildDiagnostics;
use crate::langtype::ElementType;
use crate::object_tree::*;
use crate::typeregister::TypeRegister;
use std::rc::Rc;

pub fn lower_embed(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let empty_type = type_register.empty_type();

    recurse_elem_including_sub_components_no_borrow(component, &None, &mut |elem, _| {
        if elem.borrow().is_embedding {
            return lower_embed_item(elem, &empty_type, diag);
        } else {
            Some(elem.clone())
        }
    })
}

fn lower_embed_item(
    embed_element: &ElementRc,
    _empty_type: &ElementType,
    _diag: &mut BuildDiagnostics,
) -> Option<ElementRc> {
    // let mut elem = embed_element.borrow_mut();
    // elem.is_embedding = false; // Move this flag to the placeholder
    //
    // let new = Rc::new(RefCell::new(Element {
    //     base_type: empty_type.clone(),
    //     id: "$EmbedPlaceHolder".to_string(),
    //     property_declarations: Default::default(),
    //     bindings: Default::default(),
    //     property_analysis: Default::default(),
    //     children: Default::default(),
    //     repeated: Default::default(),
    //     node: elem.node.clone(),
    //     enclosing_component: elem.enclosing_component.clone(),
    //     states: Default::default(),
    //     transitions: Default::default(),
    //     child_of_layout: Default::default(),
    //     layout_info_prop: Default::default(),
    //     default_fill_parent: (false, false),
    //     accessibility_props: Default::default(),
    //     named_references: Default::default(),
    //     item_index: Default::default(), // Not determined yet
    //     item_index_of_first_children: Default::default(),
    //     is_flickable_viewport: false,
    //     has_popup_child: false,
    //     is_embedding: true,
    //     is_legacy_syntax: elem.is_legacy_syntax,
    //     inline_depth: elem.inline_depth + 1,
    // }));
    // elem.children.push(new);

    // Map some properties?!

    Some(embed_element.clone())
}
