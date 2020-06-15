//! This pass moves all declaration of properties or signal to the root

use crate::{
    expression_tree::{Expression, NamedReference},
    object_tree::*,
    typeregister::Type,
};
use std::collections::HashMap;
use std::rc::Rc;

struct Declarations {
    property_declarations: HashMap<String, PropertyDeclaration>,
}
impl Declarations {
    fn take_from_element(e: &mut Element) -> Self {
        Declarations { property_declarations: core::mem::take(&mut e.property_declarations) }
    }
}

pub fn move_declarations(component: &Rc<Component>) {
    simplify_optimized_items(component.optimized_elements.borrow().as_slice());

    let mut decl = Declarations::take_from_element(&mut *component.root_element.borrow_mut());
    decl.property_declarations.values_mut().for_each(|d| d.expose_in_public_api = true);

    let mut new_root_bindings = HashMap::new();
    recurse_elem(&component.root_element, &mut |elem| {
        // take the bindings so we do nt keep the borrow_mut of the element
        let bindings = core::mem::take(&mut elem.borrow_mut().bindings);
        let mut new_bindings = HashMap::with_capacity(bindings.len());
        for (k, mut e) in bindings {
            fixup_bindings(&mut e, component);
            let will_be_moved = elem.borrow().property_declarations.contains_key(&k);
            if will_be_moved {
                new_root_bindings.insert(map_name(elem, k.as_str()), e);
            } else {
                new_bindings.insert(k, e);
            }
        }
        elem.borrow_mut().bindings = new_bindings;
        if let Some(r) = &mut elem.borrow_mut().repeated {
            fixup_bindings(&mut r.model, component);
        }
    });

    recurse_elem(&component.root_element, &mut |elem| {
        let elem_decl = Declarations::take_from_element(&mut *elem.borrow_mut());
        decl.property_declarations.extend(
            elem_decl.property_declarations.into_iter().map(|(p, d)| (map_name(elem, &*p), d)),
        );
    });

    {
        let mut r = component.root_element.borrow_mut();
        r.property_declarations = decl.property_declarations;
        r.bindings.extend(new_root_bindings.into_iter());
    }

    // By now, the optimized item should be unused
    #[cfg(debug_assertions)]
    assert_optized_item_unused(component.optimized_elements.borrow().as_slice());
    core::mem::take(&mut *component.optimized_elements.borrow_mut());
}

fn map_name(e: &ElementRc, s: &str) -> String {
    format!("{}_{}", e.borrow().id, s)
}

fn fixup_bindings(val: &mut Expression, comp: &Rc<Component>) {
    match val {
        Expression::PropertyReference(NamedReference { element, name })
        | Expression::SignalReference(NamedReference { element, name }) => {
            let e = element.upgrade().unwrap();
            let component = e.borrow().enclosing_component.upgrade().unwrap();
            if Rc::ptr_eq(&component, comp) && e.borrow().property_declarations.contains_key(name) {
                *name = map_name(&e, name.as_str());
                *element = Rc::downgrade(&comp.root_element);
            }
        }
        _ => {}
    };
    val.visit_mut(|sub| fixup_bindings(sub, comp))
}

/// Optimized item are not used for the fact that they are items, but their properties
/// might still be used.  So we must pretend all the properties are declared in the
/// item itself so the move_declaration pass can move the delcaration in the component root
fn simplify_optimized_items(items: &[ElementRc]) {
    for elem in items {
        recurse_elem(elem, &mut |elem| {
            let mut base_type_it = core::mem::take(&mut elem.borrow_mut().base_type);
            loop {
                base_type_it = match base_type_it {
                    Type::Component(c) => {
                        elem.borrow_mut().property_declarations.extend(
                            c.root_element
                                .borrow()
                                .property_declarations
                                .iter()
                                .map(|(k, v)| (k.clone(), v.clone())),
                        );
                        todo!(
                            "Move the bindings from the component as well.
                        But this actually should not happen because of inlining"
                        );
                        #[allow(unreachable_code)]
                        c.root_element.borrow().base_type.clone()
                    }
                    Type::Builtin(c) => {
                        // This assume that all properties of builtin items are fine with the default value
                        elem.borrow_mut().property_declarations.extend(c.properties.iter().map(
                            |(k, v)| {
                                (
                                    k.clone(),
                                    PropertyDeclaration {
                                        property_type: v.clone(),
                                        ..Default::default()
                                    },
                                )
                            },
                        ));
                        Type::Invalid
                    }
                    _ => break,
                }
            }
        })
    }
}

/// Check there are no longer references to optimized items
#[cfg(debug_assertions)]
fn assert_optized_item_unused(items: &[ElementRc]) {
    for e in items {
        recurse_elem(e, &mut |e| {
            assert_eq!(Rc::strong_count(e), 1);
            assert_eq!(Rc::weak_count(e), 0);
        });
    }
}
