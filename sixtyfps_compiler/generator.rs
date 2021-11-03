/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
The module responsible for the code generation.

There is one sub module for every language
*/

use std::collections::{HashSet, VecDeque};
use std::rc::{Rc, Weak};

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BindingExpression, Expression};
use crate::langtype::Type;
use crate::namedreference::NamedReference;
use crate::object_tree::{Component, Document, ElementRc};

#[cfg(feature = "cpp")]
mod cpp;

#[cfg(feature = "rust")]
pub mod rust;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum OutputFormat {
    #[cfg(feature = "cpp")]
    Cpp,
    #[cfg(feature = "rust")]
    Rust,
    Interpreter,
}

impl OutputFormat {
    pub fn guess_from_extension(path: &std::path::Path) -> Option<Self> {
        match path.extension().and_then(|ext| ext.to_str()) {
            #[cfg(feature = "cpp")]
            Some("cpp") | Some("cxx") | Some("h") | Some("hpp") => Some(Self::Cpp),
            #[cfg(feature = "rust")]
            Some("rs") => Some(Self::Rust),
            _ => None,
        }
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            #[cfg(feature = "cpp")]
            "cpp" => Ok(Self::Cpp),
            #[cfg(feature = "rust")]
            "rust" => Ok(Self::Rust),
            _ => Err(format!("Unknown outpout format {}", s)),
        }
    }
}

pub fn generate(
    format: OutputFormat,
    destination: &mut impl std::io::Write,
    doc: &Document,
    diag: &mut BuildDiagnostics,
) -> std::io::Result<()> {
    #![allow(unused_variables)]
    #![allow(unreachable_code)]

    if matches!(doc.root_component.root_element.borrow().base_type, Type::Invalid | Type::Void) {
        // empty document, nothing to generate
        return Ok(());
    }

    match format {
        #[cfg(feature = "cpp")]
        OutputFormat::Cpp => {
            if let Some(output) = cpp::generate(doc, diag) {
                write!(destination, "{}", output)?;
            }
        }
        #[cfg(feature = "rust")]
        OutputFormat::Rust => {
            if let Some(output) = rust::generate(doc, diag) {
                write!(destination, "{}", output)?;
            }
        }
        OutputFormat::Interpreter => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unsupported output format: The interpreter is not a valid output format yet.",
            )); // Perhaps byte code in the future?
        }
    }
    Ok(())
}

/// Visit each item in order in which they should appear in the children tree array.
/// The parameter of the visitor are
///  1. the item
///  2. the first_children_offset,
///  3. the parent index
/// DEPRECATED: Port to build_item_tree and remove this function.
#[allow(dead_code)]
pub fn build_array_helper(component: &Component, mut visit_item: impl FnMut(&ElementRc, u32, u32)) {
    visit_item(&component.root_element, 1, 0);
    visit_children(&component.root_element, 0, 1, &mut visit_item);

    fn sub_children_count(e: &ElementRc) -> usize {
        let mut count = e.borrow().children.len();
        for i in &e.borrow().children {
            count += sub_children_count(i);
        }
        count
    }

    fn visit_children(
        item: &ElementRc,
        index: u32,
        children_offset: u32,
        visit_item: &mut impl FnMut(&ElementRc, u32, u32),
    ) {
        debug_assert_eq!(index, item.borrow().item_index.get().map(|x| *x as u32).unwrap_or(index));
        let mut offset = children_offset + item.borrow().children.len() as u32;

        for i in &item.borrow().children {
            visit_item(i, offset, index);
            offset += sub_children_count(i) as u32;
        }

        let mut offset = children_offset + item.borrow().children.len() as u32;
        let mut index = children_offset;

        for e in &item.borrow().children {
            visit_children(e, index, offset, visit_item);
            index += 1;
            offset += sub_children_count(e) as u32;
        }
    }
}

/// A reference to this trait is passed to the [`build_item_tree`] function.
/// It can be used to build the array for the item tree.
pub trait ItemTreeBuilder {
    /// Some state that contains the code on how to access some particular component
    type SubComponentState;

    fn push_repeated_item(
        &mut self,
        item: &crate::object_tree::ElementRc,
        repeater_count: u32,
        parent_index: u32,
        component_state: &Self::SubComponentState,
    );
    fn push_native_item(
        &mut self,
        item: &ElementRc,
        children_offset: u32,
        parent_index: u32,
        component_state: &Self::SubComponentState,
    );
    /// Called when a component is entered, this allow to change the component_state.
    /// The returned SubComponentState will be used for all the items within that component
    fn enter_component(
        &mut self,
        item: &ElementRc,
        children_offset: u32,
        component_state: &Self::SubComponentState,
    ) -> Self::SubComponentState;
    /// Called before the children of a component are entered.
    fn enter_component_children(
        &mut self,
        item: &ElementRc,
        repeater_count: u32,
        component_state: &Self::SubComponentState,
        sub_component_state: &Self::SubComponentState,
    );
}

/// Visit each item in order in which they should appear in the children tree array.
#[allow(dead_code)]
pub fn build_item_tree<T: ItemTreeBuilder>(
    root_component: &Rc<Component>,
    initial_state: &T::SubComponentState,
    builder: &mut T,
) {
    if let Some(sub_component) = root_component.root_element.borrow().sub_component() {
        assert!(root_component.root_element.borrow().children.is_empty());
        let sub_compo_state =
            builder.enter_component(&root_component.root_element, 1, initial_state);
        builder.enter_component_children(
            &root_component.root_element,
            0,
            initial_state,
            &sub_compo_state,
        );
        build_item_tree::<T>(sub_component, &sub_compo_state, builder);
    } else {
        let mut repeater_count = 0;
        visit_item(initial_state, &root_component.root_element, 1, &mut repeater_count, 0, builder);

        visit_children(
            initial_state,
            &root_component.root_element.borrow().children,
            &root_component,
            &root_component.root_element,
            0,
            0,
            1,
            1,
            &mut repeater_count,
            builder,
        );
    }

    // Size of the element's children and grand-children,
    // needed to calculate the sub-component relative children offset indices
    fn sub_children_count(e: &ElementRc) -> usize {
        let mut count = e.borrow().children.len();
        for i in &e.borrow().children {
            count += sub_children_count(i);
        }
        count
    }

    // Size of the element's children and grand-children including
    // sub-component children, needed to allocate the correct amount of
    // index spaces for sub-components.
    fn item_sub_tree_size(e: &ElementRc) -> usize {
        let e = if let Some(sub_component) = e.borrow().sub_component() {
            sub_component.root_element.clone()
        } else {
            e.clone()
        };
        let mut count = e.borrow().children.len();
        for i in &e.borrow().children {
            count += item_sub_tree_size(i);
        }
        count
    }

    fn visit_children<T: ItemTreeBuilder>(
        state: &T::SubComponentState,
        children: &Vec<ElementRc>,
        component: &Rc<Component>,
        parent_item: &ElementRc,
        parent_index: u32,
        relative_parent_index: u32,
        children_offset: u32,
        relative_children_offset: u32,
        repeater_count: &mut u32,
        builder: &mut T,
    ) {
        debug_assert_eq!(
            relative_parent_index,
            parent_item.borrow().item_index.get().map(|x| *x as u32).unwrap_or(parent_index)
        );
        let mut offset = children_offset + children.len() as u32;

        let mut sub_component_states = VecDeque::new();

        for child in children.iter() {
            if let Some(sub_component) = child.borrow().sub_component() {
                let sub_component_state = builder.enter_component(child, offset, state);
                visit_item(
                    &sub_component_state,
                    &sub_component.root_element,
                    offset,
                    repeater_count,
                    parent_index,
                    builder,
                );
                sub_component_states.push_back(sub_component_state);
            } else {
                visit_item(state, child, offset, repeater_count, parent_index, builder);
            }
            offset += item_sub_tree_size(child) as u32;
        }

        let mut offset = children_offset + children.len() as u32;
        let mut relative_offset = relative_children_offset + children.len() as u32;
        let mut index = children_offset;
        let mut relative_index = relative_children_offset;

        for e in children.iter() {
            if let Some(sub_component) = e.borrow().sub_component() {
                let sub_tree_state = sub_component_states.pop_front().unwrap();
                builder.enter_component_children(e, *repeater_count, state, &sub_tree_state);
                visit_children(
                    &sub_tree_state,
                    &sub_component.root_element.borrow().children,
                    sub_component,
                    &sub_component.root_element,
                    index,
                    0,
                    offset,
                    1,
                    repeater_count,
                    builder,
                );
            } else {
                visit_children(
                    state,
                    &e.borrow().children,
                    component,
                    e,
                    index,
                    relative_index,
                    offset,
                    relative_offset,
                    repeater_count,
                    builder,
                );
            }

            index += 1;
            relative_index += 1;
            let size = item_sub_tree_size(e) as u32;
            offset += size;
            relative_offset += size;
        }
    }

    fn visit_item<T: ItemTreeBuilder>(
        component_state: &T::SubComponentState,
        item: &ElementRc,
        children_offset: u32,
        repeater_count: &mut u32,
        parent_index: u32,
        builder: &mut T,
    ) {
        if item.borrow().repeated.is_some() {
            builder.push_repeated_item(item, *repeater_count, parent_index, component_state);
            *repeater_count += 1;
            return;
        } else {
            let mut item = item.clone();
            while let Some(base) = {
                let base = item.borrow().sub_component().map(|c| c.root_element.clone());
                base
            } {
                item = base;
            }
            builder.push_native_item(&item, children_offset, parent_index, component_state)
        }
    }
}

/// Will call the `handle_property` callback for every property that needs to be initialized.
/// This function makes sure to call them in order so that if constant binding need to access
/// constant properties, these are already initialized
pub fn handle_property_bindings_init(
    component: &Rc<Component>,
    mut handle_property: impl FnMut(&ElementRc, &str, &BindingExpression),
) {
    fn handle_property_inner(
        component: &Weak<Component>,
        elem: &ElementRc,
        prop_name: &str,
        binding_expression: &BindingExpression,
        handle_property: &mut impl FnMut(&ElementRc, &str, &BindingExpression),
        processed: &mut HashSet<NamedReference>,
    ) {
        let nr = NamedReference::new(elem, prop_name);
        if processed.contains(&nr) {
            return;
        }
        processed.insert(nr);
        if binding_expression.analysis.borrow().as_ref().map_or(false, |a| a.is_const) {
            // We must first handle all dependent properties in case it is a constant property

            binding_expression.expression.visit_recursive(&mut |e| {
                if let Expression::PropertyReference(nr) = e {
                    let elem = nr.element();
                    if Weak::ptr_eq(&elem.borrow().enclosing_component, component) {
                        if let Some(be) = elem.borrow().bindings.get(nr.name()) {
                            handle_property_inner(
                                component,
                                &elem,
                                nr.name(),
                                be,
                                handle_property,
                                processed,
                            );
                        }
                    }
                }
            })
        }
        handle_property(elem, prop_name, binding_expression);
    }

    let mut processed = HashSet::new();
    crate::object_tree::recurse_elem(&component.root_element, &(), &mut |elem: &ElementRc, ()| {
        for (prop_name, binding_expression) in &elem.borrow().bindings {
            handle_property_inner(
                &Rc::downgrade(component),
                elem,
                prop_name,
                binding_expression,
                &mut handle_property,
                &mut processed,
            );
        }
    });
}
