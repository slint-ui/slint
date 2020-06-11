use crate::expression_tree::Expression;
use crate::object_tree::*;
use std::rc::Rc;

pub fn collect_resources(component: &Rc<Component>) {
    fn collect_resources_recursively(elem: &ElementRc, component: &Rc<Component>) {
        for e in &elem.borrow().children {
            collect_resources_recursively(e, component)
        }

        let bindings = &elem.borrow().bindings;
        for e in bindings.values() {
            fn collect_resources_from_expression(e: &Expression, component: &Rc<Component>) {
                match e {
                    Expression::ResourceReference { absolute_source_path } => {
                        let mut resources = component.embedded_file_resources.borrow_mut();
                        let maybe_id = resources.len();
                        resources.entry(absolute_source_path.clone()).or_insert(maybe_id);
                    }
                    _ => {}
                };

                e.visit(|e| collect_resources_from_expression(e, component));
            }
            collect_resources_from_expression(&e, component);
        }
    }
    collect_resources_recursively(&component.root_element, component)
}
