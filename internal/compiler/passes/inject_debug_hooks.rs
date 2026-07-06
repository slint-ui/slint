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
    // Skip:
    // * @children placeholder (generator skips these too)
    // * non-inlined sub-component instances (base_type = Component)
    if e.is_component_placeholder || e.sub_component().is_some() {
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

    // Reserved geometry properties (x, y, width, height) — these are not in property_list()
    // because they are injected globally by the type system, not per builtin element.
    // We exclude "z" to avoid spurious property materialization in materialize_fake_properties.
    // We skip the component root element because its geometry is either runtime-managed (Window)
    // or set after inlining into a parent component — either way, no compiler pass will upgrade
    // a synthetic hook, which would leave root geometry frozen at 0px.
    let geometry_candidates: Vec<(smol_str::SmolStr, Expression)> = if is_root {
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
                let default = Expression::default_value_for_type(ty);
                if matches!(default, Expression::Invalid) {
                    return None;
                }
                Some((smol_str::SmolStr::new_static(prop_name), default))
            })
            .collect()
    };

    // Also include reserved transform-property defaults.
    let transform_candidates: Vec<(smol_str::SmolStr, Expression)> = {
        let elem = element.borrow();
        if elem.debug.is_empty() {
            vec![]
        } else {
            crate::typeregister::RESERVED_TRANSFORM_PROPERTIES
                .iter()
                .filter_map(|(prop_name, _)| {
                    if elem.bindings.contains_key(*prop_name) {
                        return None;
                    }
                    let default_expr =
                        super::lower_property_to_element::transform_property_default_value(
                            element, prop_name,
                        )?;
                    Some((smol_str::SmolStr::new_static(prop_name), default_expr))
                })
                .collect()
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

    // Insert synthetic hooks for the transform properties.
    for (name, default_expr) in transform_candidates {
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

        for property in ["x", "y", "width", "height"] {
            // The default_geometry pass must have set width and height on the image.
            // If synthetic hooks were treated as real bindings, default_geometry would
            // skip the image, leaving it with no layout binding.  The resulting hook
            // must therefore be non-synthetic (upgraded from the compiler-computed default).
            let element = img.borrow();
            let binding_expression = element
                .bindings
                .get(property)
                .unwrap_or_else(|| panic!("img.{property} must be bound after default_geometry"));
            // A synthetic hook would mean default_geometry never ran.
            let expression = &binding_expression.borrow().expression;
            // There must be a debug hook, but it must not be synthetic, as the pass injects a "real"
            // binding
            assert!(
                matches!(expression, Expression::DebugHook { synthetic: false, .. }),
                "img.width should not be synthetic after default_geometry, got {expression:?}"
            );
        }
    }
}
