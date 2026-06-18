// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Hooks properties for live inspection.

use crate::expression_tree::Expression;
use crate::object_tree::{self, ElementRc, PropertyVisibility};

pub fn inject_debug_hooks(
    component: &std::rc::Rc<object_tree::Component>,
    random_state: &std::hash::RandomState,
) {
    object_tree::recurse_elem(&component.root_element, &(), &mut |elem, &()| {
        process_element(elem, random_state);
    });
}

pub fn property_id(element_id: u64, name: &smol_str::SmolStr) -> smol_str::SmolStr {
    smol_str::format_smolstr!("?{element_id}-{name}")
}

fn calculate_element_hash(
    elem: &object_tree::Element,
    index: usize,
    random_state: &std::hash::RandomState,
) -> u64 {
    let node = &elem.debug[index].node;

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

fn process_element(element: &ElementRc, random_state: &std::hash::RandomState) {
    let e = element.borrow();
    // Inject debug hook into the repeaters generated component, instead of its remaining
    // pseudo-element
    if e.repeated.is_some() {
        inject_debug_hooks(e.base_type.as_component(), random_state);
        return;
    }
    // Skip injecting debug hooks for these cases:
    // * @children placeholder (generator skips these too)
    // * non-inlined sub-component instances (base_type = Component)
    if e.is_component_placeholder || e.sub_component().is_some() {
        return;
    }
    drop(e);

    // Step 1: compute and store the element hash on EVERY debug entry.
    // This pass now runs after required-inlining, so an element may carry several debug
    // entries (one per merged source element). The LSP looks up the hash on the entry that
    // matches the selected source location, so each entry needs its own hash. (The old
    // early-pass invariant `debug.len() == 1` no longer holds.)
    let element_hash = {
        let mut elem = element.borrow_mut();
        if elem.debug.is_empty() {
            return;
        }
        for i in 0..elem.debug.len() {
            if elem.debug[i].element_hash == 0 {
                let hash = calculate_element_hash(&elem, i, random_state);
                elem.debug[i].element_hash = hash;
            }
        }
        // Bindings live on this (outermost, editor-selectable) element; a selection resolves to
        // its primary debug entry, so the first entry's hash is the one used to build ids.
        elem.debug[0].element_hash
    };

    // Step 2: Wrap existing bindings.
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
                    }
                }
            };
        });
    }

    // Step 3: Materialize hooked defaults for unbound settable properties.
    // Collect candidates first (releases the borrow), then insert.
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
                    | PropertyVisibility::Input => {}
                    PropertyVisibility::Private
                    | PropertyVisibility::Output
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

    // Now insert the hooked defaults.
    for (name, ty) in candidates {
        let default = Expression::default_value_for_type(&ty);
        let id = property_id(element_hash, &name);
        element.borrow_mut().set_binding_if_not_set(name, || Expression::DebugHook {
            expression: Box::new(default),
            id,
        });
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

    fn component(doc: &crate::object_tree::Document, id: &str) -> Rc<Component> {
        doc.inner_components.iter().find(|c| c.id == id).expect("component").clone()
    }

    fn child(root: &ElementRc, id: &str) -> ElementRc {
        // The unique-id pass suffixes ids (`txt` -> `txt-2`), so match name or `name-N`.
        fn rec(e: &ElementRc, id: &str) -> Option<ElementRc> {
            let this_id = e.borrow().id.clone();
            if this_id == id || this_id.starts_with(&format!("{id}-")) {
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

        // Unbound `text` is now hooked, wrapping the empty-string type default.
        let text_inner = hooked(&txt, "text").expect("txt.text should be a DebugHook");
        assert!(
            matches!(super::super::ignore_debug_hooks(&text_inner), Expression::StringLiteral(s) if s.is_empty()),
            "txt.text default should be the empty-string sentinel, got {text_inner:?}"
        );

        // Unbound `font-size` is hooked wrapping the 0 sentinel — keeps runtime Window inheritance.
        let fs_inner = hooked(&txt, "font-size").expect("txt.font-size should be a DebugHook");
        assert!(
            matches!(super::super::ignore_debug_hooks(&fs_inner), Expression::NumberLiteral(v, _) if *v == 0.),
            "txt.font-size default should be the 0 sentinel, got {fs_inner:?}"
        );

        // An explicitly-set property is *wrapped* (its value preserved), not replaced.
        let bg_inner = hooked(&rect, "background").expect("rect.background should be a DebugHook");
        assert!(
            !matches!(super::super::ignore_debug_hooks(&bg_inner), Expression::Invalid),
            "rect.background should wrap its real value"
        );

        // Top-level elements carry a non-zero element_hash (used to build the hook ids).
        assert_ne!(txt.borrow().debug.first().unwrap().element_hash, 0);
    }
}
