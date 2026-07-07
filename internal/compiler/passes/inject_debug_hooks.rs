// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Hooks properties for live inspection.
//!
//! This pass runs once, early in compilation — right after the import passes but before any
//! lowering or inlining. At that point every element has exactly one debug entry and
//! source-identity is intact.
//!
//! For each element the pass does two things:
//!
//! 1. **Wrap existing bindings** in a non-synthetic `Expression::DebugHook` so the editor can
//!    read and override the live value.
//!
//! 2. **Materialize synthetic hooks** for every *unbound* settable property, wrapping the
//!    type default.  These are marked `synthetic: true` (and inserted with `priority = 0`).
//!    The compiler's central helpers (`is_binding_set`, `set_binding_if_not_set`, …) in
//!    `object_tree.rs` treat synthetic hooks as "no binding", so later passes see exactly the
//!    same binding landscape as they did before the hooks were injected.  When a later pass
//!    sets a default on a property that carries a synthetic hook, `set_binding_if_not_set`
//!    replaces the hook's inner expression in place (keeping the wrapper and id) and clears
//!    the `synthetic` flag — so the hook ends up wrapping the real compiler-computed value.
//!
//! 3. **Inject a real `transform-rotation` binding** (a *non-synthetic* hook wrapping the 0deg
//!    default) on elements that support it. Transform properties only exist at runtime when the
//!    lowering reifies them onto a `Transform` wrapper element, which requires a visible binding
//!    — see the comment on `transform_candidate` below.

use crate::expression_tree::Expression;
use crate::object_tree::{self, ElementRc, PropertyVisibility};

pub fn inject_debug_hooks(
    component: &std::rc::Rc<object_tree::Component>,
    random_state: &std::hash::RandomState,
) {
    let root = component.root_element.clone();
    object_tree::recurse_elem(&root, &(), &mut |elem, &()| {
        let is_root = std::rc::Rc::ptr_eq(elem, &root);
        process_element(elem, random_state, is_root);
    });
}

pub fn property_id(element_id: u64, name: &smol_str::SmolStr) -> smol_str::SmolStr {
    smol_str::format_smolstr!("?{element_id}-{name}")
}

/// Guard rail, run near the end of the passes when debug hooks are enabled: every synthetic
/// hook that survived must sit on a property that actually exists at runtime (native,
/// declared, or materialized). An orphan would make the interpreter abort at instantiation
/// with "unknown property ..." — catch it at compile time with a source location instead.
///
/// A property still needing materialization at this point (`should_materialize` returns
/// `Some`) is exactly such an orphan.
pub fn validate_no_orphan_synthetic_hooks(component: &std::rc::Rc<object_tree::Component>) {
    if !cfg!(debug_assertions) {
        return;
    }
    object_tree::recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, &()| {
        let elem = elem.borrow();
        for (name, binding_expression) in elem.bindings.iter() {
            if !binding_expression.borrow().expression.is_synthetic_debug_hook() {
                continue;
            }
            if super::materialize_fake_properties::should_materialize(
                &elem.property_declarations,
                &elem.base_type,
                name,
            )
            .is_some()
            {
                panic!(
                    "Orphan synthetic debug hook: property '{name}' on element '{}' ({}) does \
                     not exist at runtime — a pass inserted or kept a synthetic hook for a \
                     property that is neither native, declared, nor materialized",
                    elem.id,
                    elem.debug
                        .first()
                        .map(|d| format!("{:?}", d.node.source_file.path()))
                        .unwrap_or_default(),
                );
            }
        }
    });
}

fn calculate_element_hash(
    elem: &object_tree::Element,
    random_state: &std::hash::RandomState,
) -> u64 {
    // At early-injection time (before any inlining) every element has exactly one debug entry.
    let node = &elem.debug[0].node;

    let elem_path = node.source_file.path();
    let elem_offset = node
        .child_token(crate::parser::SyntaxKind::LBrace)
        .expect("All elements have a opening Brace")
        .text_range()
        .start();

    use std::hash::{BuildHasher, Hasher};
    let mut hasher = random_state.build_hasher();
    hasher.write(elem_path.as_os_str().as_encoded_bytes());
    hasher.write_u32(elem_offset.into());
    hasher.finish()
}

fn process_element(element: &ElementRc, random_state: &std::hash::RandomState, is_root: bool) {
    let e = element.borrow();
    // Before repeater_component::process_repeater_components runs, a repeated element still
    // holds its content as children — recurse_elem will visit them naturally.
    //
    // Component-instance elements (base_type = Component, e.g. `sub := Sub { ... }`) are
    // processed like any other element: their explicit instance bindings are wrapped and
    // their unbound properties get synthetic hooks. The definition's own defaults are NOT
    // consulted here — when the component is inlined, `BindingExpression::merge_with`
    // upgrades a synthetic hook in place with the definition's real binding, so the
    // property stays live-editable per instance (under the instance element's hook id)
    // at its correct default value.
    //
    // Skip only the @children placeholder (the generator skips these too).
    if e.is_component_placeholder {
        return;
    }
    if e.debug.is_empty() {
        return;
    }
    drop(e);

    // Step 1: compute and store the element hash.
    // At early-injection time debug.len() == 1, so a single hash suffices.
    let element_hash = {
        let mut elem = element.borrow_mut();
        if elem.debug[0].element_hash == 0 {
            let hash = calculate_element_hash(&elem, random_state);
            elem.debug[0].element_hash = hash;
        }
        elem.debug[0].element_hash
    };

    // Step 2: Wrap existing real bindings in a non-synthetic DebugHook.
    {
        let elem = element.borrow();
        elem.bindings.iter().for_each(|(name, be)| {
            // Only hook properties — callback handlers and functions also live in
            // `bindings`, but hook ids are a property-only namespace and overriding a code
            // block with a value makes no sense.
            if !elem.lookup_property(name).property_type.is_property_type() {
                return;
            }
            let expr = std::mem::take(&mut be.borrow_mut().expression);
            be.borrow_mut().expression = {
                let stripped = super::ignore_debug_hooks(&expr);
                if matches!(stripped, Expression::Invalid)
                    || matches!(expr, Expression::DebugHook { .. })
                {
                    expr
                } else {
                    Expression::DebugHook {
                        expression: Box::new(expr),
                        id: property_id(element_hash, name),
                        synthetic: false,
                    }
                }
            };
        });
    }

    // Step 3: Materialize synthetic hooks for unbound settable properties.
    // Collect candidates first to release the borrow, then insert.
    let candidates: Vec<(smol_str::SmolStr, crate::langtype::Type)> = {
        let elem = element.borrow();

        // Properties from the base type.
        let base_props = elem.base_type.property_list();

        // Properties from own declarations.
        let own_props: Vec<(smol_str::SmolStr, crate::langtype::Type)> = elem
            .property_declarations
            .iter()
            .map(|(name, decl)| (name.clone(), decl.property_type.clone()))
            .collect();

        base_props
            .into_iter()
            .chain(own_props)
            .filter(|(name, _)| !elem.bindings.contains_key(name.as_str()))
            .filter_map(|(name, _ty)| {
                let name_str = name.clone();
                let lookup = elem.lookup_property(&name_str);
                // Skip functions/callbacks exposed as builtin functions.
                if lookup.builtin_function.is_some() {
                    return None;
                }
                // Only settable visibilities.
                match lookup.property_visibility {
                    PropertyVisibility::Public
                    | PropertyVisibility::InOut
                    | PropertyVisibility::Input
                    | PropertyVisibility::Private => {}
                    PropertyVisibility::Output
                    | PropertyVisibility::Constexpr
                    | PropertyVisibility::Protected
                    | PropertyVisibility::Fake => return None,
                }
                let default = Expression::default_value_for_type(&lookup.property_type);
                if matches!(default, Expression::Invalid) {
                    return None;
                }
                Some((name, lookup.property_type))
            })
            .collect()
    };

    // Elements that are (or will become) the root of a component are never wrapped by the
    // property-to-element lowerings, and their geometry is managed specially (runtime-managed
    // for windows, set after inlining otherwise) — treat them all like the component root below.
    // A `PopupWindow` is still an ordinary child element at this point, but the lower_popups
    // pass later turns it into the root of its own component. `builtin_type()` walks through
    // component bases, so instances of `component MyPopup inherits PopupWindow` are covered.
    let becomes_root = is_root
        || element.borrow().builtin_type().is_some_and(|b| b.name == "PopupWindow");

    // Reserved geometry properties (x, y, width, height) — these are not in property_list()
    // because they are injected globally by the type system, not per builtin element.
    // We exclude "z" to avoid spurious property materialization in materialize_fake_properties.
    // We skip root elements because their geometry is either runtime-managed (Window) or set
    // after inlining into a parent component — either way, no compiler pass will upgrade a
    // synthetic hook, which would leave root geometry frozen at 0px.
    // We also skip elements that don't actually have the property (non-item types like Timer
    // report Type::Invalid for the reserved properties).
    let geometry_candidates: Vec<(smol_str::SmolStr, Expression)> = if becomes_root {
        vec![]
    } else {
        let elem = element.borrow();
        crate::typeregister::RESERVED_GEOMETRY_PROPERTIES
            .iter()
            .filter(|(name, _)| *name != "z")
            .filter_map(|(prop_name, ty)| {
                if elem.bindings.contains_key(*prop_name) {
                    return None;
                }
                if elem.lookup_property(prop_name).property_type == crate::langtype::Type::Invalid
                {
                    return None;
                }
                let default = Expression::default_value_for_type(ty);
                if matches!(default, Expression::Invalid) {
                    return None;
                }
                Some((smol_str::SmolStr::new_static(prop_name), default))
            })
            .collect()
    };

    // The reserved transform properties are unlike the geometry properties above: no item
    // actually has them. They only exist at runtime when the lower_transform_properties pass
    // finds a binding and reifies the property onto an injected `Transform` wrapper element.
    // A synthetic hook is (by design) invisible to other passes, so it would never cause that
    // wrapper to be created — the hook would remain as a binding to a property that never
    // materializes, and the interpreter aborts on the unknown property.
    //
    // To keep every element rotatable in live-preview, inject `transform-rotation` as a
    // *non-synthetic* hook wrapping its default — semantically "as if the user had written
    // `transform-rotation: 0deg;`" — which the lowering then reifies like any real binding.
    // Only `transform-rotation` gets this treatment: it is what the editor overrides live, and
    // its default is context-free. The other transform properties have context-dependent
    // defaults (e.g. `transform-scale-x` follows `transform-scale`, which passes running after
    // this one may still bind) that are correctly computed at lowering time instead.
    //
    // Root elements (including future popup roots) are skipped — the lowering never wraps a
    // root, so the transform properties are not applicable there — as are elements that don't
    // support transforms at all (non-item types).
    let transform_candidate: Option<(smol_str::SmolStr, Expression)> = {
        let elem = element.borrow();
        let property_name = smol_str::SmolStr::new_static("transform-rotation");
        if becomes_root
            || elem.bindings.contains_key(&property_name)
            || elem.lookup_property(&property_name).property_type
                == crate::langtype::Type::Invalid
        {
            None
        } else {
            drop(elem);
            super::lower_property_to_element::transform_property_default_value(
                element,
                &property_name,
            )
            .map(|default_expression| (property_name, default_expression))
        }
    };

    // Insert synthetic hooks for all unbound properties.
    for (name, ty) in candidates {
        let default = Expression::default_value_for_type(&ty);
        let id = property_id(element_hash, &name);
        let mut binding_expressions: crate::expression_tree::BindingExpression =
            Expression::DebugHook { expression: Box::new(default), id, synthetic: true }.into();
        binding_expressions.priority = 0;
        use std::collections::btree_map::Entry;
        if let Entry::Vacant(entry) = element.borrow_mut().bindings.entry(name) {
            entry.insert(binding_expressions.into());
        }
    }

    // Insert synthetic hooks for reserved geometry properties (x, y, width, height).
    for (name, default_expr) in geometry_candidates {
        let id = property_id(element_hash, &name);
        let mut binding_expressions: crate::expression_tree::BindingExpression =
            Expression::DebugHook { expression: Box::new(default_expr), id, synthetic: true }
                .into();
        binding_expressions.priority = 0;
        use std::collections::btree_map::Entry;
        if let Entry::Vacant(v) = element.borrow_mut().bindings.entry(name) {
            v.insert(binding_expressions.into());
        }
    }

    // Insert the transform-rotation hook. Deliberately *non-synthetic*: later passes must see
    // it as a real binding so the Transform wrapper element is created (see above).
    if let Some((name, default_expr)) = transform_candidate {
        let id = property_id(element_hash, &name);
        let mut binding_expressions: crate::expression_tree::BindingExpression =
            Expression::DebugHook { expression: Box::new(default_expr), id, synthetic: false }
                .into();
        binding_expressions.priority = 0;
        use std::collections::btree_map::Entry;
        if let Entry::Vacant(v) = element.borrow_mut().bindings.entry(name) {
            v.insert(binding_expressions.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object_tree::Component;
    use std::rc::Rc;

    fn compile(source: &str) -> crate::object_tree::Document {
        let mut config =
            crate::CompilerConfiguration::new(crate::generator::OutputFormat::Interpreter);
        config.style = Some("fluent".into());
        config.debug_hooks = Some(std::hash::RandomState::new());
        let mut diags = crate::diagnostics::BuildDiagnostics::default();
        let doc_node = crate::parser::parse(
            source.into(),
            Some(std::path::Path::new("test.slint")),
            &mut diags,
        );
        let (doc, diag, _) = spin_on::spin_on(crate::compile_syntax_node(doc_node, diags, config));
        assert!(!diag.has_errors(), "{:?}", diag.to_string_vec());
        doc
    }

    fn component<'a>(doc: &'a crate::object_tree::Document, id: &str) -> Rc<Component> {
        doc.inner_components.iter().find(|c| c.id == id).expect("component").clone()
    }

    fn child(root: &ElementRc, id: &str) -> ElementRc {
        // The unique-id pass suffixes ids with a number (`txt` -> `txt-2`).
        // Only match that numeric suffix — don't match `txt-Transform-2` for `txt`.
        fn rec(e: &ElementRc, id: &str) -> Option<ElementRc> {
            let this_id = e.borrow().id.clone();
            let matches = this_id == id
                || this_id
                    .strip_prefix(&format!("{id}-"))
                    .is_some_and(|suffix| suffix.chars().all(|c| c.is_ascii_digit()));
            if matches {
                return Some(e.clone());
            }
            let children = e.borrow().children.clone();
            children.iter().find_map(|c| rec(c, id))
        }
        rec(root, id).unwrap_or_else(|| panic!("element {id} not found"))
    }

    /// The inner expression of a property's DebugHook, or None if the binding is not a hook.
    fn hooked(elem: &ElementRc, name: &str) -> Option<Expression> {
        let e = elem.borrow();
        let be = e.bindings.get(name)?;
        match be.borrow().expression.clone() {
            Expression::DebugHook { expression, .. } => Some(*expression),
            _ => None,
        }
    }

    /// Whether the binding is a synthetic debug hook.
    fn is_synthetic(elem: &ElementRc, name: &str) -> bool {
        let e = elem.borrow();
        let Some(be) = e.bindings.get(name) else { return false };
        matches!(be.borrow().expression, Expression::DebugHook { synthetic: true, .. })
    }

    #[test]
    fn injects_and_wraps_top_level_only() {
        let doc = compile(
            r#"
            component Sub inherits Rectangle {
                inner-text := Text { }
            }
            export component Foo inherits Window {
                txt := Text { }
                rect := Rectangle { background: red; }
                sub := Sub { }
            }
            "#,
        );

        let foo = component(&doc, "Foo");
        let txt = child(&foo.root_element, "txt");
        let rect = child(&foo.root_element, "rect");

        // Unbound `text` is now hooked (synthetic), wrapping the empty-string type default.
        let text_inner = hooked(&txt, "text").expect("txt.text should be a DebugHook");
        assert!(is_synthetic(&txt, "text"), "txt.text hook should be synthetic (unbound property)");
        assert!(
            matches!(super::super::ignore_debug_hooks(&text_inner), Expression::StringLiteral(s) if s.is_empty()),
            "txt.text default should be the empty-string sentinel, got {text_inner:?}"
        );

        // Unbound `font-size` is hooked (synthetic), wrapping the 0 sentinel.
        let fs_inner = hooked(&txt, "font-size").expect("txt.font-size should be a DebugHook");
        assert!(is_synthetic(&txt, "font-size"), "txt.font-size hook should be synthetic");
        assert!(
            matches!(super::super::ignore_debug_hooks(&fs_inner), Expression::NumberLiteral(v, _) if *v == 0.),
            "txt.font-size default should be the 0 sentinel, got {fs_inner:?}"
        );

        // An explicitly-set property is *wrapped* (non-synthetic, value preserved).
        let bg_inner = hooked(&rect, "background").expect("rect.background should be a DebugHook");
        assert!(
            !is_synthetic(&rect, "background"),
            "rect.background should be non-synthetic (was explicitly set)"
        );
        assert!(
            !matches!(super::super::ignore_debug_hooks(&bg_inner), Expression::Invalid),
            "rect.background should wrap its real value"
        );

        // Top-level elements carry a non-zero element_hash (used to build the hook ids).
        assert_ne!(txt.borrow().debug.first().unwrap().element_hash, 0);
    }

    /// Component-instance elements are hooked too. The synthetic hooks injected on the
    /// instance for the component's unbound properties must be upgraded with the definition's
    /// real default bindings when the component is inlined (`merge_with`) — NOT clobber them.
    /// Regression: repeated instances used to lose `tint: blue` / `background: tint` and
    /// render transparent.
    #[test]
    fn instance_defaults_upgraded_into_hooks() {
        let doc = compile(
            r#"
            component Item inherits Rectangle {
                in property <color> tint: blue;
                background: tint;
            }
            export component Win inherits Window {
                width: 100px; height: 100px;
                plain := Item { }
                for _idx in 2: Item { }
            }
            "#,
        );
        let win = component(&doc, "Win");

        let assert_background_preserved = |element: &ElementRc, what: &str| {
            let borrowed = element.borrow();
            let binding_expression = borrowed
                .bindings
                .get("background")
                .unwrap_or_else(|| panic!("{what}: background must be bound"));
            let expression = binding_expression.borrow().expression.clone();
            let Expression::DebugHook { expression: inner, id, synthetic } = expression else {
                panic!("{what}: background must be a DebugHook, got {expression:?}");
            };
            assert!(!synthetic, "{what}: hook must be upgraded with the definition's binding");
            // The definition's `background: tint` must survive as the hook's inner expression
            // (a reference to the — possibly moved/renamed — tint property, not the
            // transparent type default).
            let mut references_tint = false;
            inner.visit_recursive(&mut |expression| {
                if let Expression::PropertyReference(named_reference) = expression
                    && named_reference.name().ends_with("tint")
                {
                    references_tint = true;
                }
            });
            assert!(references_tint, "{what}: background must still reference tint, got {inner:?}");
            // The hook id must belong to one of the merged source elements (the instance
            // element's hash — hooks were injected before inlining).
            assert!(
                borrowed
                    .debug
                    .iter()
                    .any(|d| property_id(d.element_hash, &smol_str::SmolStr::new_static("background")) == id),
                "{what}: hook id {id} must match a merged element hash"
            );
        };

        // Non-repeated instance: `plain` is the merged element after inlining.
        let plain = child(&win.root_element, "plain");
        assert_background_preserved(&plain, "plain instance");

        // Repeated instance: the repeated element's base is the generated repeater component;
        // the merged instance element is inside it.
        let repeated = win
            .root_element
            .borrow()
            .children
            .iter()
            .find(|c| c.borrow().repeated.is_some())
            .expect("repeated element")
            .clone();
        let repeated_base = repeated.borrow().base_type.as_component().clone();
        let mut found = None;
        object_tree::recurse_elem(&repeated_base.root_element, &(), &mut |elem, &()| {
            if elem.borrow().bindings.contains_key("background") {
                found = Some(elem.clone());
            }
        });
        let repeated_item = found.expect("repeated Item element with background binding");
        assert_background_preserved(&repeated_item, "repeated instance");
    }

    /// Direct unit tests for the synthetic-hook rules in `BindingExpression::merge_with`
    /// (used by inlining to merge a definition's bindings into an instance element).
    #[test]
    fn merge_with_synthetic_hook_rules() {
        use crate::expression_tree::BindingExpression;

        let synthetic_hook = || -> BindingExpression {
            Expression::DebugHook {
                expression: Box::new(Expression::NumberLiteral(0., Default::default())),
                id: "?42-prop".into(),
                synthetic: true,
            }
            .into()
        };
        let real_binding = |value: f64| -> BindingExpression {
            let mut binding: BindingExpression =
                Expression::NumberLiteral(value, Default::default()).into();
            binding.priority = 3;
            binding
        };

        // Synthetic hook + real binding: upgraded in place, wrapper and id survive.
        let mut binding = synthetic_hook();
        assert!(binding.merge_with(&real_binding(7.)), "the other expression must be taken");
        match &binding.expression {
            Expression::DebugHook { expression, id, synthetic } => {
                assert!(!synthetic, "upgraded hook must no longer be synthetic");
                assert_eq!(id, "?42-prop", "the hook id must survive the merge");
                assert!(
                    matches!(**expression, Expression::NumberLiteral(v, _) if v == 7.),
                    "the definition's expression must be taken"
                );
            }
            other => panic!("expected an upgraded DebugHook, got {other:?}"),
        }
        assert_eq!(binding.priority, 3, "the other side's priority must be taken");

        // Synthetic hook + synthetic hook: unchanged, still synthetic ("no binding").
        let mut binding = synthetic_hook();
        assert!(!binding.merge_with(&synthetic_hook()));
        assert!(binding.expression.is_synthetic_debug_hook());

        // Synthetic hook + two-way-only binding: the hook is dropped — its default must not
        // become the two-way's initial value.
        let mut binding = synthetic_hook();
        let mut two_way: BindingExpression = Expression::Invalid.into();
        two_way.two_way_bindings.push(crate::expression_tree::TwoWayBinding::ModelData {
            repeated_element: std::rc::Weak::default(),
            field_access: Default::default(),
        });
        assert!(binding.merge_with(&two_way));
        assert!(matches!(binding.expression, Expression::Invalid));
        assert_eq!(binding.two_way_bindings.len(), 1);

        // Real (non-synthetic hook) binding keeps priority over anything.
        let mut binding: BindingExpression = Expression::DebugHook {
            expression: Box::new(Expression::NumberLiteral(1., Default::default())),
            id: "?42-prop".into(),
            synthetic: false,
        }
        .into();
        assert!(!binding.merge_with(&real_binding(9.)));
        assert!(
            matches!(binding.expression.ignore_debug_hooks(), Expression::NumberLiteral(v, _) if *v == 1.)
        );
    }

    /// The injected `transform-rotation` hook must cause the Transform wrapper element to be
    /// reified so the property actually exists at runtime, and the hook (carrying the source
    /// element's hash id) must survive as a non-synthetic binding. A binding left on a property
    /// that is never materialized would abort the interpreter at instantiation time.
    #[test]
    fn transform_rotation_hook_is_reified() {
        let doc = compile(
            r#"
            export component Foo inherits Window {
                rect := Rectangle { }
            }
            "#,
        );
        let foo = component(&doc, "Foo");
        let rect = child(&foo.root_element, "rect");
        let rect_hash = rect.borrow().debug.first().unwrap().element_hash;
        let rotation_hook_id =
            property_id(rect_hash, &smol_str::SmolStr::new_static("transform-rotation"));

        // A Transform wrapper element must have been injected for the rectangle.
        let transform_element = child(&foo.root_element, "rect-Transform");

        // The rotation hook ends up driving the Transform element (the two-way binding to the
        // rectangle's materialized property is collapsed by the alias optimizations); it must
        // be non-synthetic and wrap the 0 default.
        let binding_holder = transform_element.borrow();
        let binding_expression = binding_holder
            .bindings
            .get("transform-rotation")
            .expect("the Transform element must bind transform-rotation");
        match &binding_expression.borrow().expression {
            Expression::DebugHook { id, synthetic, expression } => {
                assert_eq!(id, &rotation_hook_id, "hook id must be derived from rect's hash");
                assert!(!synthetic, "the injected rotation hook must be non-synthetic");
                assert!(
                    matches!(**expression, Expression::NumberLiteral(v, _) if v == 0.),
                    "the rotation hook must wrap the 0deg default"
                );
            }
            other => panic!("transform-rotation must be a DebugHook, got {other:?}"),
        }
    }

    /// Regression: geometry defaults (width/height) must still be computed even when
    /// debug hooks are active and inject synthetic hooks for unbound geometry properties.
    #[test]
    fn geometry_defaults_still_set_with_debug_hooks() {
        let doc = compile(
            r#"
            export component Foo inherits Window {
                img := Image { source: @image-url("nonexistent.png"); }
            }
            "#,
        );
        let foo = component(&doc, "Foo");
        let img = child(&foo.root_element, "img");
        let img_hash = img.borrow().debug.first().unwrap().element_hash;

        // The geometry properties are materialized into declarations and the declarations
        // (with their bindings) are moved to the root by move_declarations — so look the hook
        // up by its id across the whole component instead of by name on the img element.
        let find_hook_by_id = |wanted_id: &smol_str::SmolStr| -> Option<Expression> {
            let mut found = None;
            object_tree::recurse_elem(&foo.root_element, &(), &mut |elem, &()| {
                for (_, binding_expression) in elem.borrow().bindings.iter() {
                    if let Expression::DebugHook { id, .. } = &binding_expression.borrow().expression
                        && id == wanted_id
                    {
                        found = Some(binding_expression.borrow().expression.clone());
                    }
                }
            });
            found
        };

        for property in ["x", "y", "width", "height"] {
            // The default_geometry pass must have set width and height on the image.
            // If synthetic hooks were treated as real bindings, default_geometry would
            // skip the image, leaving it with no layout binding.  The resulting hook
            // must therefore be non-synthetic: either upgraded by default_geometry itself or
            // by materialize_fake_properties' initialization.
            let hook_id = property_id(img_hash, &smol_str::SmolStr::new_static(property));
            let expression = find_hook_by_id(&hook_id)
                .unwrap_or_else(|| panic!("a debug hook for img.{property} must survive"));
            assert!(
                matches!(expression, Expression::DebugHook { synthetic: false, .. }),
                "img.{property} hook should not be synthetic after default_geometry, got {expression:?}"
            );
        }
    }
}

