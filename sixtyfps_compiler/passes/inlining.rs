//! Inline each object_tree::Component within the main Component

use crate::{expression_tree::Expression, object_tree::*, typeregister::Type};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub fn inline(doc: &Document) {
    fn inline_elements_recursively(elem: &Rc<RefCell<Element>>, component: &Rc<Component>) {
        let base = elem.borrow().base_type.clone();
        if let Type::Component(c) = base {
            // First, make sure that the component itself is properly inlined
            inline_elements_recursively(&c.root_element, &c);
            // Inline this component.
            inline_element(elem, &c, component);
        }

        for child in &elem.borrow().children {
            inline_elements_recursively(child, component);
        }
    }
    inline_elements_recursively(&doc.root_component.root_element, &doc.root_component)
}

fn clone_tuple<U: Clone, V: Clone>((u, v): (&U, &V)) -> (U, V) {
    (u.clone(), v.clone())
}

fn element_key(e: &Rc<RefCell<Element>>) -> usize {
    &**e as *const RefCell<Element> as usize
}

fn inline_element(
    elem: &Rc<RefCell<Element>>,
    inlined_component: &Rc<Component>,
    root_component: &Rc<Component>,
) {
    // inlined_component must be the base type of this element
    debug_assert_eq!(
        format!("{:?}", elem.borrow().base_type),
        format!("{:?}", Type::Component(inlined_component.clone()))
    );

    let mut elem_mut = elem.borrow_mut();
    elem_mut.base_type = inlined_component.root_element.borrow().base_type.clone();
    elem_mut.property_declarations.extend(
        inlined_component.root_element.borrow().property_declarations.iter().map(clone_tuple),
    );
    elem_mut
        .signals_declaration
        .extend_from_slice(&inlined_component.root_element.borrow().signals_declaration);

    // Map the old element to the new
    let mut mapping = HashMap::new();
    mapping.insert(element_key(&inlined_component.root_element), elem.clone());

    let mut new_children = vec![];
    new_children
        .reserve(elem_mut.children.len() + inlined_component.root_element.borrow().children.len());
    new_children.extend(
        inlined_component
            .root_element
            .borrow()
            .children
            .iter()
            .map(|x| duplicate_element_with_mapping(x, &mut mapping)),
    );
    new_children.append(&mut elem_mut.children);
    elem_mut.children = new_children;

    elem_mut.bindings.extend(
        inlined_component
            .root_element
            .borrow()
            .bindings
            .iter()
            .map(|(k, val)| (k.clone(), fixup_binding(val, &mapping, root_component))),
    );

    //core::mem::drop(elem_mut);

    for (key, e) in &mapping {
        if *key == element_key(&inlined_component.root_element) {
            continue; // the root has been processed
        }
        for (_, expr) in &mut e.borrow_mut().bindings {
            *expr = fixup_binding(expr, &mapping, root_component)
        }
    }
}

// Duplicate the element elem and all its children. And fill the mapping to point from the old to the new
fn duplicate_element_with_mapping(
    element: &Rc<RefCell<Element>>,
    mapping: &mut HashMap<usize, Rc<RefCell<Element>>>,
) -> Rc<RefCell<Element>> {
    let elem = element.borrow();
    let new = Rc::new(RefCell::new(Element {
        base_type: elem.base_type.clone(),
        id: elem.id.clone(),
        property_declarations: elem.property_declarations.clone(),
        signals_declaration: elem.signals_declaration.clone(),
        // We will do the mapping of the binding later
        bindings: elem.bindings.clone(),
        children: elem
            .children
            .iter()
            .map(|x| duplicate_element_with_mapping(x, mapping))
            .collect(),
    }));
    mapping.insert(element_key(element), new.clone());
    new
}

fn fixup_binding(
    val: &Expression,
    mapping: &HashMap<usize, Rc<RefCell<Element>>>,
    root_component: &Rc<Component>,
) -> Expression {
    match val {
        Expression::PropertyReference { element, name, .. } => Expression::PropertyReference {
            component: Rc::downgrade(root_component),
            element: element
                .upgrade()
                .and_then(|e| mapping.get(&element_key(&e)))
                .map(Rc::downgrade)
                .unwrap(),
            name: name.clone(),
        },
        Expression::SignalReference { element, name, .. } => Expression::SignalReference {
            component: Rc::downgrade(root_component),
            element: element
                .upgrade()
                .and_then(|e| mapping.get(&element_key(&e)))
                .map(Rc::downgrade)
                .unwrap(),
            name: name.clone(),
        },
        x @ _ => x.clone(),
    }
}
