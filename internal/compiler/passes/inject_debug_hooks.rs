// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Hooks properties for live updates (and potentially inspection in the future).
//!
//! This pass runs once, early in compilation — right after the import passes but before any
//! lowering or inlining. At that point elements match 1:1 to the code as written (and every
//! element has exactly one debug entry).
//!
//! For each element the pass does two things:
//!
//! 1. **Wrap existing bindings** in a non-synthetic `Expression::DebugHook` so the editor can
//!    read and override the live value.
//!
//! 2. **Materialize synthetic hooks** for every *unbound* settable property.
//!    These are marked `synthetic: true` (and inserted with `priority = 0`).
//!    The accessors in `object_tree.rs` treat synthetic hooks as "no binding", so later passes
//!    must opt-in to seeing bindings with a synthetic hook.
//!    Exception: transform properties are marked non-synthetic, as they must force the transform
//!    pass to inject a wrapper element for the transform.

use crate::expression_tree::Expression;
use crate::langtype::PropertyLookupMode;
use crate::object_tree::forward_inherited_expression::{
    ForwardedReferenceCache, InheritedExpression, forward_inherited_expression,
};
use crate::object_tree::{self, Element, ElementDebugInfo, ElementRc, PropertyVisibility};
use crate::symbol_counters::SymbolCounters;
use std::rc::Rc;

pub fn inject_debug_hooks(
    root_components: &[Rc<object_tree::Component>],
    random_state: &std::hash::RandomState,
    symbol_counters: &SymbolCounters,
    forwarded_references: &mut ForwardedReferenceCache,
) {
    for component in root_components {
        let root = component.root_element.clone();
        object_tree::recurse_elem(&root, &(), &mut |element, &()| {
            process_existing_bindings(element, random_state);
        });
    }

    // Process the missing bindings after the existing bindings.
    // Injecting missing bindings will insert new forwarding bindings & properties.
    // These should not be hooked as "existing", which is why we need to insert them
    // after all existing bindings are processed.
    for component in root_components {
        let root = component.root_element.clone();
        object_tree::recurse_elem(&root, &(), &mut |element, &()| {
            let is_root = Rc::ptr_eq(element, &root);
            process_missing_bindings(element, symbol_counters, forwarded_references, is_root);
        });
    }
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
    object_tree::recurse_elem_including_sub_components_no_borrow(
        component,
        &(),
        &mut |elem, &()| {
            let elem = elem.borrow();
            for (name, binding_expression) in elem.bindings_including_synthetic() {
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
        },
    );
}

fn calculate_element_hash(
    debug_info: &ElementDebugInfo,
    random_state: &std::hash::RandomState,
) -> u64 {
    // At early-injection time (before any inlining) every element has exactly one debug entry.
    let node = &debug_info.node;

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

fn assign_element_hash(element: &ElementRc, random_state: &std::hash::RandomState) -> u64 {
    let mut elem = element.borrow_mut();

    // Each element in the source has one debug entry.
    // There may be more if elements have been inlined.
    // This should not yet have happened so that we can identify which source element this is.
    debug_assert!(elem.debug.len() == 1);
    let debug_info = &mut elem.debug[0];
    if debug_info.element_hash == 0 {
        let hash = calculate_element_hash(debug_info, random_state);
        debug_info.element_hash = hash;
    }
    debug_info.element_hash
}

fn hook_existing_bindings(element: &ElementRc, element_hash: u64) {
    let elem = element.borrow();
    elem.bindings_including_synthetic().for_each(|(name, be)| {
        // Only hook properties — callback handlers and functions also live in
        // `bindings`, but hook ids are a property-only namespace and overriding a code
        // block with a value makes no sense.
        if !elem
            .lookup_property(name, PropertyLookupMode::InternalName)
            .property_type
            .is_property_type()
        {
            return;
        }
        let expr = std::mem::take(&mut be.borrow_mut().expression);
        be.borrow_mut().expression = {
            let stripped = expr.ignore_debug_hooks();
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

fn property_defaults(
    elem: &Element,
) -> impl Iterator<Item = (smol_str::SmolStr, crate::expression_tree::Expression, bool)> {
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
        .filter(|(name, _)| elem.binding_cell_including_synthetic(name.as_str()).is_none())
        .filter_map(|(name, _ty)| {
            let name_str = name.clone();
            let lookup = elem.lookup_property(&name_str, PropertyLookupMode::InternalName);
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
            Some((name, default, true))
        })
}

// Reserved geometry properties (x, y, width, height) are not in property_list()
// because they are injected by the type system.
// We exclude "z" to avoid spurious property materialization in materialize_fake_properties.
// TODO: Add appropriate debug hook.
fn geometry_properties()
-> impl Iterator<Item = (smol_str::SmolStr, crate::expression_tree::Expression, bool)> {
    crate::typeregister::RESERVED_GEOMETRY_PROPERTIES
        .iter()
        .filter(|(name, _)| *name != "z")
        .filter_map(|(prop_name, ty)| {
            let default = Expression::default_value_for_type(ty);
            if matches!(default, Expression::Invalid) {
                return None;
            }
            Some((smol_str::SmolStr::new_static(prop_name), default, true))
        })
}

// The reserved transform properties are unlike the geometry properties above: no item
// actually has them. They only exist at runtime when the lower_transform_properties pass
// finds a binding and wraps the element in a Transform element.
//
// Make the binding non-synthetic, so the later pass picks them up.
fn transform_properties<'a>(
    element: &'a ElementRc,
) -> impl Iterator<Item = (smol_str::SmolStr, crate::expression_tree::Expression, bool)> + 'a {
    // TODO: Wrap other transform properties.
    const TRANSFORM_PROPS: [&str; 3] =
        ["transform-rotation", "transform-scale-x", "transform-scale-y"];
    TRANSFORM_PROPS.into_iter().map(|property_name| {
        let property_name = smol_str::SmolStr::new_static(property_name);
        let default_expression =
            super::lower_property_to_element::transform_property_default_value(
                element,
                &property_name,
            )
            .unwrap();
        (property_name.clone(), default_expression, false)
    })
}

fn add_hooks_for_non_existent_bindings(
    element: &ElementRc,
    element_hash: u64,
    symbol_counters: &SymbolCounters,
    forwarded_references: &mut ForwardedReferenceCache,
    is_root: bool,
) {
    let elem = element.borrow();
    let mut properties: Vec<_> = property_defaults(&elem).collect();

    // Elements that are (or will become) the root of a component are never wrapped by the
    // property-to-element lowerings, and their geometry is managed specially (runtime-managed
    // for windows, set after inlining otherwise) — treat them all like the component root below.
    // A `PopupWindow` is still an ordinary child element at this point, but the lower_popups
    // pass later turns it into the root of its own component. `builtin_type()` walks through
    // component bases, so instances of `component MyPopup inherits PopupWindow` are covered.
    let becomes_root =
        is_root || element.borrow().builtin_type().is_some_and(|b| b.name == "PopupWindow");

    // Skip root elements because their geometry is either runtime-managed (Window) or set
    // after inlining into a parent component — either way, no compiler pass will upgrade a
    // synthetic hook, which would leave root geometry frozen at 0px.
    if !becomes_root {
        properties.extend(geometry_properties());
    };

    // Root elements (including future popup roots) are skipped — the lowering never wraps a
    // root, so the transform properties are not applicable there — as are elements that don't
    // support transforms at all (non-item types).
    //
    if !becomes_root {
        properties.extend(transform_properties(element));
    }

    drop(elem);
    let unbound_properties = properties.into_iter().filter(|(name, _default, _synthetic)| {
        let elem = element.borrow();
        elem.binding_cell_including_synthetic(name).is_none()
            // Filter invalid reserved properties (e.g. x/y on a Timer, etc.)
            && elem.lookup_property(name, PropertyLookupMode::InternalName).property_type != crate::langtype::Type::Invalid
            && !elem.is_property_target_of_two_way_binding(name)
    });

    for (name, default_expression, synthetic) in unbound_properties {
        let expression = match forward_inherited_expression(
            element,
            &name,
            symbol_counters,
            forwarded_references,
        ) {
            InheritedExpression::Expression(expression) => expression.ignore_debug_hooks().clone(),
            InheritedExpression::TwoWayBinding => continue,
            InheritedExpression::Unbound => default_expression,
        };
        let id = property_id(element_hash, &name);
        let mut binding: crate::expression_tree::BindingExpression =
            Expression::DebugHook { expression: Box::new(expression), id, synthetic }.into();
        binding.priority = 0;
        let mut elem = element.borrow_mut();
        if elem.binding_cell_including_synthetic(&name).is_none() {
            elem.set_binding(name, binding);
        }
    }
}

fn is_hookable(element: &ElementRc) -> bool {
    let element = element.borrow();
    // Skip the @children placeholder (the generator skips these too).
    if element.is_component_placeholder {
        return false;
    }
    if element.debug.is_empty() {
        return false;
    }

    true
}

fn process_existing_bindings(element: &ElementRc, random_state: &std::hash::RandomState) {
    if !is_hookable(element) {
        return;
    }

    let element_hash = assign_element_hash(element, random_state);

    hook_existing_bindings(element, element_hash);
}

fn process_missing_bindings(
    element: &ElementRc,
    symbol_counters: &SymbolCounters,
    forwarded_references: &mut ForwardedReferenceCache,
    is_root: bool,
) {
    if !is_hookable(element) {
        return;
    }

    let element_hash = element.borrow().debug[0].element_hash;
    debug_assert_ne!(element_hash, 0);

    add_hooks_for_non_existent_bindings(
        element,
        element_hash,
        symbol_counters,
        forwarded_references,
        is_root,
    );
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
        config.inline_all_elements = false;
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
        let be = e.binding_cell_including_synthetic(name)?;
        match be.borrow().expression.clone() {
            Expression::DebugHook { expression, .. } => Some(*expression),
            _ => None,
        }
    }

    /// Whether the binding is a synthetic debug hook.
    fn is_synthetic(elem: &ElementRc, name: &str) -> bool {
        let e = elem.borrow();
        let Some(be) = e.binding_cell_including_synthetic(name) else { return false };
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
            matches!(text_inner.ignore_debug_hooks(), Expression::StringLiteral(s) if s.is_empty()),
            "txt.text default should be the empty-string sentinel, got {text_inner:?}"
        );

        // Unbound `font-size` is hooked (synthetic), wrapping the 0 sentinel.
        let fs_inner = hooked(&txt, "font-size").expect("txt.font-size should be a DebugHook");
        assert!(is_synthetic(&txt, "font-size"), "txt.font-size hook should be synthetic");
        assert!(
            matches!(fs_inner.ignore_debug_hooks(), Expression::NumberLiteral(v, _) if *v == 0.),
            "txt.font-size default should be the 0 sentinel, got {fs_inner:?}"
        );

        // An explicitly-set property is *wrapped* (non-synthetic, value preserved).
        let bg_inner = hooked(&rect, "background").expect("rect.background should be a DebugHook");
        assert!(
            !is_synthetic(&rect, "background"),
            "rect.background should be non-synthetic (was explicitly set)"
        );
        assert!(
            !matches!(bg_inner.ignore_debug_hooks(), Expression::Invalid),
            "rect.background should wrap its real value"
        );

        // Top-level elements carry a non-zero element_hash (used to build the hook ids).
        assert_ne!(txt.borrow().debug.first().unwrap().element_hash, 0);
    }

    #[test]
    fn instance_defaults_forwarded_into_hooks() {
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
                .binding_cell_including_synthetic("background")
                .unwrap_or_else(|| panic!("{what}: background must be bound"));
            let expression = binding_expression.borrow().expression.clone();
            let Expression::DebugHook { expression: inner, id, synthetic } = expression else {
                panic!("{what}: background must be a DebugHook, got {expression:?}");
            };
            assert!(synthetic, "{what}: inherited hook must remain synthetic");
            let mut references_tint = false;
            inner.visit_recursive(&mut |expression| {
                if let Expression::PropertyReference(named_reference) = expression
                    && named_reference.name().ends_with("tint")
                {
                    references_tint = true;
                }
            });
            assert!(references_tint, "{what}: background must still reference tint, got {inner:?}");
            assert_eq!(
                property_id(
                    borrowed.debug[0].element_hash,
                    &smol_str::SmolStr::new_static("background")
                ),
                id,
                "{what}: hook id must use the instance element hash"
            );
            assert!(
                matches!(borrowed.base_type, crate::langtype::ElementType::Component(_)),
                "{what}: component boundary must remain"
            );
        };

        let plain = child(&win.root_element, "plain");
        assert_background_preserved(&plain, "plain instance");

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
            if elem.borrow().binding_cell_including_synthetic("background").is_some() {
                found = Some(elem.clone());
            }
        });
        let repeated_item = found.expect("repeated Item element with background binding");
        assert_background_preserved(&repeated_item, "repeated instance");
    }

    #[test]
    fn reuses_forwarded_references_for_hooks_and_states() {
        let doc = compile(
            r#"
            component Item inherits Rectangle {
                in-out property <int> property-value: inner.width / 1px;
                in-out property <int> function-value: inner.compute-value(3);
                in-out property <int> callback-value: inner.compute-callback(5);
                inner := Rectangle {
                    width: 10px;
                    pure function compute-value(value: int) -> int { value + 10 }
                    pure callback compute-callback(int) -> int;
                    compute-callback(value) => value * 2;
                }
            }
            export component Win inherits Window {
                in property <bool> active;
                first := Item { }
                second := Item { }
                states [
                    active when root.active: {
                        first.property-value: 42;
                        first.function-value: 43;
                        first.callback-value: 44;
                    }
                ]
            }
            "#,
        );
        let item = component(&doc, "Item");
        let win = component(&doc, "Win");
        let first = child(&win.root_element, "first");
        let second = child(&win.root_element, "second");

        let declarations = item
            .root_element
            .borrow()
            .property_declarations
            .keys()
            .filter(|name| name.starts_with("forward_reference_"))
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(declarations.len(), 3);

        let forwarded_references = |element: &ElementRc| {
            let element = element.borrow();
            let mut references = std::collections::HashSet::new();
            for property_name in ["property-value", "function-value", "callback-value"] {
                let binding = element
                    .binding_cell_including_synthetic(property_name)
                    .unwrap_or_else(|| panic!("{property_name} must be hooked"));
                binding.borrow().expression.visit_recursive(&mut |expression| match expression {
                    Expression::PropertyReference(named_reference)
                    | Expression::FunctionCall {
                        function:
                            crate::expression_tree::Callable::Callback(named_reference)
                            | crate::expression_tree::Callable::Function(named_reference),
                        ..
                    } if named_reference.name().starts_with("forward_reference_") => {
                        references.insert(named_reference.name().clone());
                    }
                    _ => {}
                });
            }
            references
        };

        let first_references = forwarded_references(&first);
        let second_references = forwarded_references(&second);
        assert!(!first_references.is_empty());
        assert!(!second_references.is_empty());
        assert!(first_references.is_subset(&declarations));
        assert!(second_references.is_subset(&declarations));
        assert!(matches!(first.borrow().base_type, crate::langtype::ElementType::Component(_)));
        assert!(matches!(second.borrow().base_type, crate::langtype::ElementType::Component(_)));
    }

    #[test]
    fn inherited_two_way_binding_has_no_hook() {
        let doc = compile(
            r#"
            component Item inherits Rectangle {
                in-out property <length> linked <=> target;
                in-out property <length> target: 42px;
            }
            export component Win inherits Window {
                item := Item { }
            }
            "#,
        );
        let win = component(&doc, "Win");
        let item = child(&win.root_element, "item");

        assert!(item.borrow().binding_cell_including_synthetic("linked").is_none());
        assert!(item.borrow().binding_cell_including_synthetic("target").is_none());
        assert!(matches!(item.borrow().base_type, crate::langtype::ElementType::Component(_)));
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
        assert!(matches!(binding.value_expression(), Expression::NumberLiteral(v, _) if *v == 1.));
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
            .binding_cell_including_synthetic("transform-rotation")
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
                for (_, binding_expression) in elem.borrow().bindings_including_synthetic() {
                    if let Expression::DebugHook { id, .. } =
                        &binding_expression.borrow().expression
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
