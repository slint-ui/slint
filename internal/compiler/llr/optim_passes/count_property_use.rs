// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Passes that fills the Property::use_count
//!
//! This pass assume that use_count of all properties is zero

use crate::llr::{
    Animation, BindingExpression, CompilationUnit, EvaluationContext, Expression, ParentCtx,
    PropertyReference,
};

pub fn count_property_use(root: &CompilationUnit) {
    // Visit the root properties that are used.
    // 1. the public properties
    for c in &root.public_components {
        let root_ctx = EvaluationContext::new_sub_component(root, c.item_tree.root, (), None);
        for p in c.public_properties.iter().filter(|p| {
            !matches!(
                p.prop,
                PropertyReference::Function { .. } | PropertyReference::GlobalFunction { .. }
            )
        }) {
            visit_property(&p.prop, &root_ctx);
        }
    }
    for (idx, g) in root.globals.iter_enumerated().filter(|(_, g)| g.exported) {
        let ctx = EvaluationContext::new_global(root, idx, ());
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
            match pr {
                PropertyReference::Local { sub_component_path, property_index } => {
                    let mut sc = sc;
                    for i in sub_component_path {
                        sc = &ctx.compilation_unit.sub_components[sc.sub_components[*i].ty];
                    }
                    if sc.properties[*property_index].use_count.get() == 0 {
                        continue;
                    }
                }
                PropertyReference::InNativeItem { .. } => {}
                _ => unreachable!(),
            }
            let c = expr.use_count.get();
            expr.use_count.set(c + 1);
            if c == 0 {
                visit_binding_expression(expr, ctx)
            }
        }
        // 3. the init code
        for expr in &sc.init_code {
            expr.borrow().visit_property_references(ctx, &mut visit_property);
        }
        // 4. the models
        for (idx, r) in sc.repeated.iter_enumerated() {
            r.model.borrow().visit_property_references(ctx, &mut visit_property);
            if let Some(lv) = &r.listview {
                visit_property(&lv.viewport_y, ctx);
                visit_property(&lv.viewport_width, ctx);
                visit_property(&lv.viewport_height, ctx);
                visit_property(&lv.listview_width, ctx);
                visit_property(&lv.listview_height, ctx);

                let rep_ctx = EvaluationContext::new_sub_component(
                    root,
                    r.sub_tree.root,
                    (),
                    Some(ParentCtx::new(ctx, Some(idx))),
                );
                visit_property(&lv.prop_y, &rep_ctx);
                visit_property(&lv.prop_height, &rep_ctx);
            }
            for idx in r.data_prop.iter().chain(r.index_prop.iter()) {
                // prevent optimizing model properties
                let p = &root.sub_components[r.sub_tree.root].properties[*idx];
                p.use_count.set(2);
            }
        }

        // 5. the layout info
        sc.layout_info_h.borrow().visit_property_references(ctx, &mut visit_property);
        sc.layout_info_v.borrow().visit_property_references(ctx, &mut visit_property);

        // 6. accessibility props and geometries
        for b in sc.accessible_prop.values() {
            b.borrow().visit_property_references(ctx, &mut visit_property)
        }
        for i in sc.geometries.iter().filter_map(Option::as_ref) {
            i.borrow().visit_property_references(ctx, &mut visit_property)
        }

        // 7. aliases (if they were not optimize, they are probably used)
        for (a, b) in &sc.two_way_bindings {
            visit_property(a, ctx);
            visit_property(b, ctx);
        }

        // 8.functions (TODO: only visit used function)
        for f in &sc.functions {
            f.code.visit_property_references(ctx, &mut visit_property);
        }

        // 9. change callbacks
        for (p, e) in &sc.change_callbacks {
            visit_property(p, ctx);
            e.borrow().visit_property_references(ctx, &mut visit_property);
        }

        // 10. popup x/y coordinates
        for popup in &sc.popup_windows {
            let popup_ctx = EvaluationContext::new_sub_component(
                root,
                popup.item_tree.root,
                (),
                Some(ParentCtx::new(ctx, None)),
            );
            popup.position.borrow().visit_property_references(&popup_ctx, &mut visit_property)
        }
        // 11. timer
        for timer in &sc.timers {
            timer.interval.borrow().visit_property_references(ctx, &mut visit_property);
            timer.running.borrow().visit_property_references(ctx, &mut visit_property);
            timer.triggered.borrow().visit_property_references(ctx, &mut visit_property);
        }
    });

    // TODO: only visit used function
    for (idx, g) in root.globals.iter_enumerated() {
        let ctx = EvaluationContext::new_global(root, idx, ());
        for f in &g.functions {
            f.code.visit_property_references(&ctx, &mut visit_property);
        }
    }

    if let Some(p) = &root.popup_menu {
        let ctx = EvaluationContext::new_sub_component(root, p.item_tree.root, (), None);
        visit_property(&p.entries, &ctx);
        visit_property(&p.sub_menu, &ctx);
        visit_property(&p.activated, &ctx);
    }

    clean_unused_bindings(root);
}

fn visit_property(pr: &PropertyReference, ctx: &EvaluationContext) {
    let p_info = ctx.property_info(pr);
    if let Some(p) = &p_info.property_decl {
        p.use_count.set(p.use_count.get() + 1);
    }
    if let Some((binding, map)) = &p_info.binding {
        let c = binding.use_count.get();
        binding.use_count.set(c + 1);
        if c == 0 {
            let ctx2 = map.map_context(ctx);
            visit_binding_expression(binding, &ctx2);
        }
    }
}

fn visit_binding_expression(binding: &BindingExpression, ctx: &EvaluationContext) {
    binding.expression.borrow().visit_property_references(ctx, &mut visit_property);
    match &binding.animation {
        Some(Animation::Static(e) | Animation::Transition(e)) => {
            e.visit_property_references(ctx, &mut visit_property)
        }
        None => (),
    }
}

/// Bindings which have a use_count of zero can be cleared so that we won't ever visit them later.
fn clean_unused_bindings(root: &CompilationUnit) {
    root.for_each_sub_components(&mut |sc, _| {
        for (_, e) in &sc.property_init {
            if e.use_count.get() == 0 {
                e.expression.replace(Expression::CodeBlock(vec![]));
            }
        }
    });
    for g in &root.globals {
        for e in g.init_values.iter().flatten() {
            if e.use_count.get() == 0 {
                e.expression.replace(Expression::CodeBlock(vec![]));
            }
        }
    }
}
