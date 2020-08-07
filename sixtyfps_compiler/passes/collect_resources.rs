use crate::expression_tree::Expression;
use crate::object_tree::*;
use std::rc::Rc;

pub fn collect_resources(component: &Rc<Component>) {
    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        visit_element_expressions(elem, |e, _| collect_resources_from_expression(e, component));
    })
}

fn collect_resources_from_expression(e: &Expression, component: &Rc<Component>) {
    match e {
        Expression::ResourceReference { absolute_source_path } => {
            let mut resources = component.referenced_file_resources.borrow_mut();
            let maybe_id = resources.len();
            resources.entry(absolute_source_path.clone()).or_insert(maybe_id);
        }
        _ => {}
    };

    e.visit(|e| collect_resources_from_expression(e, component));
}
