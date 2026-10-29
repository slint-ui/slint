// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Lowers the `@actor` annotation of a component to a `CreateActor` call in
//! the component's constructor code. The Rust generator turns that call into
//! the creation of the component's facade and the invocation of the actor that
//! the application registered for the component.
//!
//! The call names the identity of the component's defining file, so that
//! same-named components from different files get different generated types.
//! The pass also records the component's interface on the document, for the
//! generated facade and message types.

use crate::builtin_functions::BuiltinFunction;
use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{Callable, Expression, NamedReference};
use crate::langtype::Type;
use crate::object_tree::{ActorInterface, Component, Document, InitCodeEntry};
use smol_str::{SmolStr, format_smolstr};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub fn lower_actor(
    doc: &Document,
    type_loader: &crate::typeloader::TypeLoader,
    diag: &mut BuildDiagnostics,
) {
    let Some(main_file) = doc.node.as_ref().map(|n| n.source_file.path().clone()) else {
        return;
    };
    let mut lowered = HashSet::<*const Component>::new();
    // Different identities can map to the same module path, for example
    // `a-b.slint` and `a_b.slint`; their generated types would clash.
    let mut generated_names = HashMap::<(Vec<String>, SmolStr), String>::new();
    doc.visit_all_used_components(|component| {
        let Some(node) = component.node.as_ref() else { return };
        let Some(at_actor) = node.AtActor() else { return };
        if !lowered.insert(Rc::as_ptr(component)) {
            return;
        }
        let name = &component.id;
        let defining_file = node.source_file.path();
        // Only files on this host have a path relative to the main file. Code
        // compiled from a string, such as the `slint!` macro, has a relative path:
        // only the main file itself has one.
        let identity = match (defining_file.as_native_path(), main_file.as_native_path()) {
            _ if *defining_file == main_file => String::new(),
            (Some(defining), Some(main)) if defining.is_absolute() => {
                let (identity, escapes) = crate::generator::actor_file_identity(
                    defining,
                    main,
                    &type_loader.compiler_config,
                );
            if escapes {
                diag.push_warning(
                    format!(
                        "The @actor component '{name}' is defined outside the main file's directory, \
                        the library paths, and the include paths, so its identity '{identity}' depends on \
                        the file layout. Import it through a library for a stable identity"
                    ),
                        &at_actor,
                    );
                }
                identity
            }
            _ => defining_file.to_string(),
        };
        let segments = crate::generator::actor_identity_module_segments(&identity);
        match generated_names.entry((segments, name.clone())) {
            Entry::Occupied(other) => diag.push_error(
                format!(
                    "The generated types of the @actor component '{name}' from '{identity}' \
                    clash with the ones from '{}'. Rename one of the files",
                    other.get()
                ),
                &at_actor,
            ),
            Entry::Vacant(entry) => {
                entry.insert(identity.clone());
            }
        }
        lower_component(component, identity, doc, diag);
    });
}

fn lower_component(
    component: &Rc<Component>,
    identity: String,
    doc: &Document,
    diag: &mut BuildDiagnostics,
) {
    let Some(node) = component.node.as_ref() else { return };
    let root = component.root_element.clone();
    let mut arguments = vec![
        Expression::StringLiteral(component.id.clone()),
        Expression::StringLiteral(identity.as_str().into()),
    ];
    let mut interface = ActorInterface {
        name: component.id.clone(),
        identity,
        callbacks: Vec::new(),
        properties: Vec::new(),
    };
    let declarations = root
        .borrow()
        .property_declarations
        .iter()
        .map(|(name, decl)| (name.clone(), decl.property_type.clone(), decl.node.clone()))
        .collect::<Vec<_>>();
    for (name, ty, decl_node) in declarations {
        let tag = match &ty {
            Type::Callback(function) => {
                if function.return_type != Type::Void {
                    let span: &dyn Spanned =
                        if decl_node.is_some() { &decl_node } else { node as &dyn Spanned };
                    diag.push_error(
                        format!(
                            "Callback '{name}' of the @actor component '{}' can't return a value. \
                            Actors receive callbacks as messages; use a property that the actor sets for the result",
                            component.id
                        ),
                        span,
                    );
                    continue;
                }
                interface.callbacks.push((name.clone(), function.args.clone()));
                format_smolstr!("cb:{name}")
            }
            Type::Function(_) => continue,
            _ if ty.is_property_type() => {
                interface.properties.push((name.clone(), ty.clone()));
                format_smolstr!("prop:{name}")
            }
            _ => continue,
        };
        arguments.push(Expression::StringLiteral(tag));
        arguments.push(Expression::PropertyReference(NamedReference::new(&root, name)));
    }
    let call = Expression::FunctionCall {
        function: BuiltinFunction::CreateActor.into(),
        arguments,
        source_location: Some(node.to_source_location()),
    };
    mark_referenced_externally(&call);
    component
        .init_code
        .borrow_mut()
        .constructor_code
        .push(InitCodeEntry { owner: Rc::downgrade(&root), code: call });
    doc.actor_interfaces.borrow_mut().push(interface);
}

/// The actor reads and sets the referenced properties and installs handlers for
/// the referenced callbacks at run-time, so none of them may be optimized as if
/// only the component used them. The actor acts from within the component, so
/// they count as set and read inside it, too: otherwise a use site of the
/// component could treat a property without binding as constant.
fn mark_referenced_externally(call: &Expression) {
    let Expression::FunctionCall { arguments, .. } = call else { return };
    for argument in arguments {
        if let Expression::PropertyReference(nr) = argument {
            let element = nr.element();
            let element = element.borrow();
            let mut analysis = element.property_analysis.borrow_mut();
            let analysis = analysis.entry(nr.name().clone()).or_default();
            analysis.is_set = true;
            analysis.is_read = true;
            analysis.is_set_externally = true;
            analysis.is_read_externally = true;
        }
    }
}

/// Inlining merges the property analysis of the inlined component into its use
/// site and loses the external flags set by [`lower_actor`]. Restore them for
/// every `CreateActor` call, so that later passes such as the constant
/// propagation keep the referenced properties intact.
pub fn protect_actor_references(doc: &Document) {
    doc.visit_all_used_components(|component| {
        for expr in component.init_code.borrow().iter() {
            expr.visit_recursive(&mut |e| {
                if matches!(
                    e,
                    Expression::FunctionCall {
                        function: Callable::Builtin(BuiltinFunction::CreateActor),
                        ..
                    }
                ) {
                    mark_referenced_externally(e);
                }
            });
        }
    });
}
