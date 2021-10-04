/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use crate::expression_tree::{Expression, ImageReference};
use crate::object_tree::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub fn embed_resources(component: &Rc<Component>, embed_files_by_default: bool) {
    let global_embedded_resources = &component.embedded_file_resources;

    for component in
        component.used_types.borrow().sub_components.iter().chain(std::iter::once(component))
    {
        visit_all_expressions(component, |e, _| {
            embed_resources_from_expression(e, global_embedded_resources, embed_files_by_default)
        });
    }
}

fn embed_resources_from_expression(
    e: &mut Expression,
    global_embedded_resources: &RefCell<HashMap<String, usize>>,
    embed_files_by_default: bool,
) {
    if let Expression::ImageReference(ref mut resource_ref) = e {
        match resource_ref {
            ImageReference::AbsolutePath(path)
                if embed_files_by_default || path.starts_with("builtin:/") =>
            {
                let mut resources = global_embedded_resources.borrow_mut();
                let maybe_id = resources.len();
                let resource_id = *resources.entry(path.clone()).or_insert(maybe_id);
                *resource_ref = ImageReference::EmbeddedData {
                    resource_id,
                    extension: std::path::Path::new(path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|x| x.to_string())
                        .unwrap_or_default(),
                }
            }
            _ => {}
        }
    };

    e.visit_mut(|mut e| {
        embed_resources_from_expression(&mut e, global_embedded_resources, embed_files_by_default)
    });
}
