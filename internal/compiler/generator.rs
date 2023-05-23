// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
The module responsible for the code generation.

There is one sub module for every language
*/

use std::collections::{BTreeSet, HashSet, VecDeque};
use std::rc::{Rc, Weak};

use crate::expression_tree::{BindingExpression, Expression};
use crate::langtype::ElementType;
use crate::namedreference::NamedReference;
use crate::object_tree::{Component, Document, ElementRc, RepeatedElementInfo};

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
    Llr,
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
            "llr" => Ok(Self::Llr),
            _ => Err(format!("Unknown outpout format {}", s)),
        }
    }
}

pub fn generate(
    format: OutputFormat,
    destination: &mut impl std::io::Write,
    doc: &Document,
) -> std::io::Result<()> {
    #![allow(unused_variables)]
    #![allow(unreachable_code)]

    if matches!(doc.root_component.root_element.borrow().base_type, ElementType::Error) {
        // empty document, nothing to generate
        return Ok(());
    }

    match format {
        #[cfg(feature = "cpp")]
        OutputFormat::Cpp => {
            let output = cpp::generate(doc);
            write!(destination, "{}", output)?;
        }
        #[cfg(feature = "rust")]
        OutputFormat::Rust => {
            let output = rust::generate(doc);
            write!(destination, "{}", output)?;
        }
        OutputFormat::Interpreter => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unsupported output format: The interpreter is not a valid output format yet.",
            )); // Perhaps byte code in the future?
        }
        OutputFormat::Llr => {
            writeln!(
                destination,
                "{:#?}",
                crate::llr::lower_to_item_tree::lower_to_item_tree(&doc.root_component)
            )?;
        }
    }
    Ok(())
}

/// A reference to this trait is passed to the [`build_item_tree`] function.
/// It can be used to build the array for the item tree.
pub trait ItemTreeBuilder {
    /// Some state that contains the code on how to access some particular component
    type SubComponentState: Clone;

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
        sub_component: &Rc<Component>,
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
pub fn build_item_tree<T: ItemTreeBuilder>(
    root_component: &Rc<Component>,
    initial_state: &T::SubComponentState,
    builder: &mut T,
) {
    if let Some(sub_component) = root_component.root_element.borrow().sub_component() {
        assert!(root_component.root_element.borrow().children.is_empty());
        let sub_compo_state =
            builder.enter_component(&root_component.root_element, sub_component, 1, initial_state);
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
            root_component,
            &root_component.root_element,
            0,
            0,
            1,
            1,
            &mut repeater_count,
            builder,
        );
    }

    // Size of the element's children and grand-children including
    // sub-component children, needed to allocate the correct amount of
    // index spaces for sub-components.
    fn item_sub_tree_size(e: &ElementRc) -> usize {
        let mut count = e.borrow().children.len();
        if let Some(sub_component) = e.borrow().sub_component() {
            count += item_sub_tree_size(&sub_component.root_element);
        }
        for i in &e.borrow().children {
            count += item_sub_tree_size(i);
        }
        count
    }

    fn visit_children<T: ItemTreeBuilder>(
        state: &T::SubComponentState,
        children: &[ElementRc],
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

        // Suppose we have this:
        // ```
        // Button := Rectangle { /* some repeater here*/ }
        // StandardButton := Button { /* no children */ }
        // App := Dialog { StandardButton { /* no children */ }}
        // ```
        // The inlining pass ensures that *if* `StandardButton` had children, `Button` would be inlined, but that's not the case here.
        //
        // We are in the stage of visiting the Dialog's children and we'll end up visiting the Button's Rectangle because visit_item()
        // on the StandardButton - a Dialog's child - follows all the way to the Rectangle as native item. We've also determine that
        // StandardButton is a sub-component and we'll call visit_children() on it. Now we are here. However as `StandardButton` has no children,
        // and therefore we would never recurse into `Button`'s children and thus miss the repeater. That is what this condition attempts to
        // detect and chain the children visitation.
        if children.is_empty() {
            if let Some(nested_subcomponent) = parent_item.borrow().sub_component() {
                let sub_component_state = builder.enter_component(
                    parent_item,
                    nested_subcomponent,
                    children_offset,
                    state,
                );
                visit_children(
                    &sub_component_state,
                    &nested_subcomponent.root_element.borrow().children,
                    nested_subcomponent,
                    &nested_subcomponent.root_element,
                    parent_index,
                    relative_parent_index,
                    children_offset,
                    relative_children_offset,
                    repeater_count,
                    builder,
                );
                return;
            }
        }

        let mut offset = children_offset + children.len() as u32;

        let mut sub_component_states = VecDeque::new();

        for child in children.iter() {
            if let Some(sub_component) = child.borrow().sub_component() {
                let sub_component_state =
                    builder.enter_component(child, sub_component, offset, state);
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
        match &item.borrow().repeated {
            Some(RepeatedElementInfo::Repeater(_)) => {
                builder.push_repeated_item(item, *repeater_count, parent_index, component_state);
                *repeater_count += 1;
            }
            Some(RepeatedElementInfo::Embedding(_)) => {
                todo!();
            }
            None => {
                let mut item = item.clone();
                let mut component_state = component_state.clone();
                while let Some((base, state)) = {
                    let base = item.borrow().sub_component().map(|c| {
                        (
                            c.root_element.clone(),
                            builder.enter_component(&item, c, children_offset, &component_state),
                        )
                    });
                    base
                } {
                    item = base;
                    component_state = state;
                }
                builder.push_native_item(&item, children_offset, parent_index, &component_state)
            }
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
        if binding_expression.analysis.as_ref().map_or(false, |a| a.is_const) {
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
                                &be.borrow(),
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
                &binding_expression.borrow(),
                &mut handle_property,
                &mut processed,
            );
        }
    });
}

/// Call the given function for each constant property in the Component so one can set
/// `set_constant` on it.
pub fn for_each_const_properties(component: &Rc<Component>, mut f: impl FnMut(&ElementRc, &str)) {
    crate::object_tree::recurse_elem(&component.root_element, &(), &mut |elem: &ElementRc, ()| {
        if elem.borrow().repeated.is_some() {
            return;
        }
        let mut e = elem.clone();
        let mut all_prop = BTreeSet::new();
        loop {
            all_prop.extend(
                e.borrow()
                    .property_declarations
                    .iter()
                    .filter(|(_, x)| {
                        x.property_type.is_property_type() &&
                            !matches!( &x.property_type, crate::langtype::Type::Struct { name: Some(name), .. } if name.ends_with("::StateInfo"))
                    })
                    .map(|(k, _)| k.clone()),
            );
            match &e.clone().borrow().base_type {
                ElementType::Component(c) => {
                    e = c.root_element.clone();
                }
                ElementType::Native(n) => {
                    let mut n = n;
                    loop {
                        all_prop.extend(
                            n.properties
                                .iter()
                                .filter(|(k, x)| {
                                    x.ty.is_property_type()
                                        && !k.starts_with("viewport-")
                                        && k.as_str() != "commands"
                                })
                                .map(|(k, _)| k.clone()),
                        );
                        match n.parent.as_ref() {
                            Some(p) => n = p,
                            None => break,
                        }
                    }
                    break;
                }
                ElementType::Builtin(_) => {
                    unreachable!("builtin element should have been resolved")
                }
                ElementType::Global | ElementType::Error => break,
            }
        }
        for c in all_prop {
            if NamedReference::new(elem, &c).is_constant() {
                f(elem, &c);
            }
        }
    });
}

/// Convert a ascii kebab string to pascal case
pub fn to_pascal_case(str: &str) -> String {
    let mut result = Vec::with_capacity(str.len());
    let mut next_upper = true;
    for x in str.as_bytes() {
        if *x == b'-' {
            next_upper = true;
        } else if next_upper {
            result.push(x.to_ascii_uppercase());
            next_upper = false;
        } else {
            result.push(*x);
        }
    }
    String::from_utf8(result).unwrap()
}

/// Convert a ascii pascal case string to kebab case
pub fn to_kebab_case(str: &str) -> String {
    let mut result = Vec::with_capacity(str.len());
    for x in str.as_bytes() {
        if x.is_ascii_uppercase() {
            if !result.is_empty() {
                result.push(b'-');
            }
            result.push(x.to_ascii_lowercase());
        } else {
            result.push(*x);
        }
    }
    String::from_utf8(result).unwrap()
}

#[test]
fn case_conversions() {
    assert_eq!(to_kebab_case("HelloWorld"), "hello-world");
    assert_eq!(to_pascal_case("hello-world"), "HelloWorld");
}
