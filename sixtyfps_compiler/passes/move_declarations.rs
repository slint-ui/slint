//! This pass moves all declaration of properties or signal to the root

use crate::{expression_tree::Expression, object_tree::*};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

struct Declarations {
    signals_declaration: Vec<String>,
    property_declarations: HashMap<String, PropertyDeclaration>,
}
impl Declarations {
    fn take_from_element(e: &mut Element) -> Self {
        Declarations {
            signals_declaration: core::mem::take(&mut e.signals_declaration),
            property_declarations: core::mem::take(&mut e.property_declarations),
        }
    }
}

pub fn move_declarations(component: &Rc<Component>) {
    let mut decl = Declarations::take_from_element(&mut *component.root_element.borrow_mut());

    fn fixup_bindings_recursive(
        elem: &Rc<RefCell<Element>>,
        component: &Rc<Component>,
        new_root_bindings: &mut HashMap<String, Expression>,
    ) {
        // take the bindings so we do nt keep the borrow_mut of the element
        let bindings = core::mem::take(&mut elem.borrow_mut().bindings);
        let mut new_bindings = HashMap::with_capacity(bindings.len());
        for (k, mut e) in bindings {
            fixup_bindings(&mut e, component);
            let will_be_moved = elem.borrow().signals_declaration.contains(&k)
                || elem.borrow().property_declarations.contains_key(&k);
            if will_be_moved {
                new_root_bindings.insert(map_name(elem, k.as_str()), e);
            } else {
                new_bindings.insert(k, e);
            }
        }
        //bindings.retain(|name, e| true);
        //component.root_element.borrow_mut().bindings.extend(bindings.)

        elem.borrow_mut().bindings = new_bindings;
        for c in &elem.borrow().children {
            fixup_bindings_recursive(c, component, new_root_bindings)
        }
    }

    let mut new_root_bindings = HashMap::new();
    fixup_bindings_recursive(&component.root_element, component, &mut new_root_bindings);

    fn move_declarations_recursive(elem: &Rc<RefCell<Element>>, decl: &mut Declarations) {
        for c in &elem.borrow().children {
            let elem_decl = Declarations::take_from_element(&mut *c.borrow_mut());
            decl.signals_declaration
                .extend(elem_decl.signals_declaration.into_iter().map(|s| map_name(c, &*s)));
            decl.property_declarations.extend(
                elem_decl.property_declarations.into_iter().map(|(p, d)| (map_name(c, &*p), d)),
            );
            move_declarations_recursive(c, decl);
        }
    }

    move_declarations_recursive(&component.root_element, &mut decl);

    {
        let mut r = component.root_element.borrow_mut();
        r.signals_declaration = decl.signals_declaration;
        r.property_declarations = decl.property_declarations;
        r.bindings.extend(new_root_bindings.into_iter());
    }
}

fn map_name(e: &Rc<RefCell<Element>>, s: &str) -> String {
    format!("{}_{}", e.borrow().id, s)
}

fn fixup_bindings(val: &mut Expression, comp: &Rc<Component>) {
    match val {
        Expression::PropertyReference { component, element, name } => {
            let e = element.upgrade().unwrap();
            let component = component.upgrade().unwrap();
            if Rc::ptr_eq(&component, comp) && e.borrow().property_declarations.contains_key(name) {
                *name = map_name(&e, name.as_str());
                *element = Rc::downgrade(&comp.root_element);
            }
        }
        Expression::SignalReference { component, element, name } => {
            let e = element.upgrade().unwrap();
            let component = component.upgrade().unwrap();
            if Rc::ptr_eq(&component, comp) && e.borrow().signals_declaration.contains(name) {
                *name = map_name(&e, name.as_str());
                *element = Rc::downgrade(&comp.root_element);
            }
        }
        _ => {}
    };
    val.visit_mut(|sub| fixup_bindings(sub, comp))
}
