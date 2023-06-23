// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

//! Passes that fills the Property::use_count
//!
//! This pass assume that use_count of all properties is zero

use crate::llr::{EvaluationContext, Expression, ParentCtx, PropertyReference, PublicComponent};

pub fn count_property_use(root: &PublicComponent) {
    // Visit the root properties that are used.
    // 1. the public properties
    let root_ctx = EvaluationContext::new_sub_component(root, &root.item_tree.root, (), None);
    for p in root.public_properties.iter().filter(|p| {
        !matches!(
            p.prop,
            PropertyReference::Function { .. } | PropertyReference::GlobalFunction { .. }
        )
    }) {
        visit_property(&p.prop, &root_ctx);
    }
    for g in root.globals.iter().filter(|g| g.exported) {
        let ctx = EvaluationContext::new_global(root, g, ());
        for p in g.public_properties.iter().filter(|p| {
            !matches!(
                p.prop,
                PropertyReference::Function { .. } | PropertyReference::GlobalFunction { .. }
            )
        }) {
            visit_property(&p.prop, &ctx);
        }
    }

    root.for_each_sub_components(&mut |sc, ctx| {
        // 2. the native items and bindings of used properties
        for (pr, expr) in &sc.property_init {
            let c = expr.use_count.get();
            if c > 0 {
                continue;
            }
            match pr {
                PropertyReference::Local { sub_component_path, property_index } => {
                    let mut sc = sc;
                    for i in sub_component_path {
                        sc = &sc.sub_components[*i].ty;
                    }
                    if sc.properties[*property_index].use_count.get() == 0 {
                        continue;
                    }
                }
                PropertyReference::InNativeItem { .. } => {}
                _ => unreachable!(),
            }
            expr.use_count.set(c + 1);
            expr.expression.borrow().visit_recursive(&mut |e| visit_expression(e, ctx));
        }
        // 3. the init code
        for expr in &sc.init_code {
            expr.borrow().visit_recursive(&mut |e| visit_expression(e, ctx));
        }
        // 4. the models
        for (idx, r) in sc.repeated.iter().enumerate() {
            r.model.borrow().visit_recursive(&mut |e| visit_expression(e, ctx));
            if let Some(lv) = &r.listview {
                visit_property(&lv.viewport_y, ctx);
                visit_property(&lv.viewport_width, ctx);
                visit_property(&lv.viewport_height, ctx);
                visit_property(&lv.listview_width, ctx);
                visit_property(&lv.listview_height, ctx);

                let rep_ctx = EvaluationContext::new_sub_component(
                    root,
                    &r.sub_tree.root,
                    (),
                    Some(ParentCtx::new(ctx, Some(idx))),
                );
                visit_property(&lv.prop_y, &rep_ctx);
                visit_property(&lv.prop_width, &rep_ctx);
                visit_property(&lv.prop_height, &rep_ctx);
            }
            for idx in r.data_prop.iter().chain(r.index_prop.iter()) {
                // prevent optimizing model properties
                let p = &r.sub_tree.root.properties[*idx];
                p.use_count.set(2);
            }
        }

        // 5. the layout info
        sc.layout_info_h.borrow().visit_recursive(&mut |e| visit_expression(e, ctx));
        sc.layout_info_v.borrow().visit_recursive(&mut |e| visit_expression(e, ctx));

        // 6. accessibility props
        for b in sc.accessible_prop.values() {
            b.borrow().visit_recursive(&mut |e| visit_expression(e, ctx))
        }

        // 7. aliases (if they were not optimize, they are probably used)
        for (a, b) in &sc.two_way_bindings {
            visit_property(a, ctx);
            visit_property(b, ctx);
        }

        // 8.functions (TODO: only visit used function)
        for f in &sc.functions {
            f.code.visit_recursive(&mut |e| visit_expression(e, ctx));
        }
    });

    // TODO: only visit used function
    for g in root.globals.iter() {
        let ctx = EvaluationContext::new_global(root, g, ());
        for f in &g.functions {
            f.code.visit_recursive(&mut |e| visit_expression(e, &ctx));
        }
    }
}

fn visit_property(pr: &PropertyReference, ctx: &EvaluationContext) {
    let p_info = super::inline_expressions::property_binding_and_analysis(ctx, pr);
    if let Some(p) = &p_info.property_decl {
        p.use_count.set(p.use_count.get() + 1);
    }
    if let Some((binding, map)) = &p_info.binding {
        let c = binding.use_count.get();
        binding.use_count.set(c + 1);
        if c == 0 {
            let ctx2 = map.map_context(ctx);
            binding.expression.borrow().visit_recursive(&mut |e| visit_expression(e, &ctx2))
        }
    }
}

fn visit_expression(expr: &Expression, ctx: &EvaluationContext) {
    let p = match expr {
        Expression::PropertyReference(p) => p,
        Expression::CallBackCall { callback, .. } => callback,
        Expression::PropertyAssignment { property, .. } => property,
        // FIXME  (should be fine anyway because we mark these as not optimizable)
        Expression::ModelDataAssignment { .. } => return,
        Expression::LayoutCacheAccess { layout_cache_prop, .. } => layout_cache_prop,
        _ => return,
    };
    visit_property(p, ctx)
}
