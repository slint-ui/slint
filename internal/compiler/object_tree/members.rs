// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;

macro_rules! unwrap_or_continue {
    ($e:expr ; $diag:expr) => {
        match $e {
            Some(x) => x,
            None => {
                debug_assert!($diag.has_errors()); // error should have been reported at parsing time
                continue;
            }
        }
    };
}

type PendingBindings<T> = Vec<(SmolStr, T, syntax_nodes::DeclaredIdentifier)>;

impl Element {
    pub(super) fn declare_properties(
        r: &mut Element,
        node: &syntax_nodes::Element,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) -> (
        PendingBindings<syntax_nodes::BindingExpression>,
        PendingBindings<syntax_nodes::TwoWayBinding>,
    ) {
        let is_interface = matches!(r.base_type, ElementType::Interface(_));
        #[cfg(feature = "slint-sc")]
        let is_component_root = is_component_root(node);
        let mut property_bindings = Vec::new();
        let mut two_way_bindings = Vec::new();

        for prop_decl in node.PropertyDeclaration() {
            // Only the root element's properties become part of the component's API
            #[cfg(feature = "slint-sc")]
            if !is_component_root {
                diag.slint_sc_error(
                    "Declaring a property on an element other than the root is",
                    &prop_decl,
                );
            }
            let prop_type = prop_decl
                .Type()
                .map(|type_node| type_from_node(type_node, diag, tr))
                // Type::Void is used for two way bindings without type specified
                .unwrap_or(Type::InferredProperty);

            let unresolved_prop_name =
                unwrap_or_continue!(parser::identifier_text(&prop_decl.DeclaredIdentifier()); diag);
            let declaration = r.member_declaration(&unresolved_prop_name);
            let name_token =
                prop_decl.DeclaredIdentifier().child_token(SyntaxKind::Identifier).unwrap();
            if let MemberDeclaration::Conflict { existing_type, declared_in } = &declaration {
                match existing_type {
                    Type::Callback { .. } => diag.push_error(
                        format!("Cannot declare property '{unresolved_prop_name}' when a callback with the same name exists"),
                        &name_token,
                    ),
                    Type::Function { .. } => diag.push_error(
                        format!("Cannot declare property '{unresolved_prop_name}' when a function with the same name exists"),
                        &name_token,
                    ),
                    _ => diag.push_error(
                        cannot_override_message(Some("property"), &unresolved_prop_name, declared_in),
                        &name_token,
                    ),
                }
                continue;
            }
            let prop_name = declaration.register(r, &unresolved_prop_name, &name_token, diag);
            let shadowed_name =
                (prop_name != unresolved_prop_name).then(|| unresolved_prop_name.clone());

            let mut visibility = None;
            for token in prop_decl.children_with_tokens() {
                if token.kind() != SyntaxKind::Identifier {
                    continue;
                }
                match (token.as_token().unwrap().text(), visibility) {
                    ("in", None) => visibility = Some(PropertyVisibility::Input),
                    ("in", Some(_)) => diag.push_error("Extra 'in' keyword".into(), &token),
                    ("out", None) => visibility = Some(PropertyVisibility::Output),
                    ("out", Some(_)) => diag.push_error("Extra 'out' keyword".into(), &token),
                    ("in-out" | "in_out", None) => visibility = Some(PropertyVisibility::InOut),
                    ("in-out" | "in_out", Some(_)) => {
                        diag.push_error("Extra 'in-out' keyword".into(), &token)
                    }
                    ("private", None) => visibility = Some(PropertyVisibility::Private),
                    ("private", Some(_)) => {
                        diag.push_error("Extra 'private' keyword".into(), &token)
                    }
                    _ => (),
                }
            }
            let visibility = visibility.unwrap_or({
                if r.is_legacy_syntax {
                    PropertyVisibility::InOut
                } else {
                    PropertyVisibility::Private
                }
            });

            if is_interface {
                if let Some(binding_expression) = &prop_decl.BindingExpression() {
                    diag.push_error(
                        "Interface properties cannot have default values".into(),
                        binding_expression,
                    )
                }
                if let Some(two_way) = &prop_decl.TwoWayBinding() {
                    diag.push_error(
                        "Interface properties cannot have default bindings".into(),
                        two_way,
                    )
                }
                if visibility == PropertyVisibility::Private {
                    diag.push_error(
                        "'private' properties are inaccessible in an interface".into(),
                        &prop_decl,
                    );
                }
            }

            let deprecated = member_deprecation(prop_decl.PropertyDeprecation(), diag);

            r.property_declarations.insert(
                prop_name.clone(),
                PropertyDeclaration {
                    property_type: prop_type,
                    node: Some(prop_decl.clone().into()),
                    visibility,
                    shadowed_name,
                    shadowable: shadowable_attribute(prop_decl.ShadowableAttribute(), tr, diag),
                    deprecated,
                    ..Default::default()
                },
            );

            if let Some(csn) = prop_decl.BindingExpression() {
                property_bindings.push((prop_name.clone(), csn, prop_decl.DeclaredIdentifier()));
            }

            if let Some(csn) = prop_decl.TwoWayBinding() {
                #[cfg(feature = "slint-sc")]
                diag.slint_sc_error("Two-way bindings are", &csn);
                two_way_bindings.push((prop_name, csn, prop_decl.DeclaredIdentifier()));
            }
        }
        (property_bindings, two_way_bindings)
    }

    pub(super) fn declare_callbacks(
        r: &mut Element,
        node: &syntax_nodes::Element,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) {
        #[cfg(feature = "slint-sc")]
        let is_component_root = is_component_root(node);
        for sig_decl in node.CallbackDeclaration() {
            let name =
                unwrap_or_continue!(parser::identifier_text(&sig_decl.DeclaredIdentifier()); diag);

            let pure = Some(
                sig_decl.child_token(SyntaxKind::Identifier).is_some_and(|t| t.text() == "pure"),
            );

            #[cfg(feature = "slint-sc")]
            {
                // Only the root element's callbacks become part of the component's API
                if !is_component_root {
                    diag.slint_sc_error(
                        "Declaring a callback on an element other than the root is",
                        &sig_decl,
                    );
                }
                if pure == Some(true) {
                    diag.slint_sc_error("Pure callbacks are", &sig_decl);
                }
                if let Some(param) = sig_decl.CallbackDeclarationParameter().next() {
                    diag.slint_sc_error("Callback parameters are", &param);
                }
                if let Some(ret) = sig_decl.ReturnType() {
                    diag.slint_sc_error("Callback return types are", &ret);
                }
            }

            let declaration = r.member_declaration(&name);
            if let MemberDeclaration::Conflict { existing_type, declared_in } = &declaration {
                if matches!(existing_type, Type::Callback { .. }) {
                    // Already declared on this very element, rather than inherited
                    if r.declaration(&name).is_some() {
                        diag.push_error(
                            "Duplicated callback declaration".into(),
                            &sig_decl.DeclaredIdentifier(),
                        );
                    } else {
                        diag.push_error(
                            cannot_override_message(Some("callback"), &name, declared_in),
                            &sig_decl.DeclaredIdentifier(),
                        )
                    }
                } else {
                    diag.push_error(
                        format!(
                            "Cannot declare callback '{name}' when a {} with the same name exists",
                            if matches!(existing_type, Type::Function { .. }) {
                                "function"
                            } else {
                                "property"
                            }
                        ),
                        &sig_decl.DeclaredIdentifier(),
                    );
                }
                continue;
            }
            let shadowable = shadowable_attribute(sig_decl.ShadowableAttribute(), tr, diag);
            let deprecated = member_deprecation(sig_decl.PropertyDeprecation(), diag);
            let source_name = name;
            let name = declaration.register(r, &source_name, &sig_decl.DeclaredIdentifier(), diag);
            let shadowed_name = (name != source_name).then_some(source_name);

            if let Some(csn) = sig_decl.TwoWayBinding() {
                #[cfg(feature = "slint-sc")]
                diag.slint_sc_error("Callback aliases are", &csn);
                r.bindings
                    .0
                    .insert(name.clone(), BindingExpression::new_uncompiled(csn.into()).into());
                r.property_declarations.insert(
                    name,
                    PropertyDeclaration {
                        property_type: Type::InferredCallback,
                        node: Some(sig_decl.into()),
                        visibility: PropertyVisibility::InOut,
                        pure,
                        shadowed_name,
                        shadowable,
                        deprecated,
                        ..Default::default()
                    },
                );
                continue;
            }

            let args = sig_decl
                .CallbackDeclarationParameter()
                .map(|p| type_from_node(p.Type(), diag, tr))
                .collect();
            let return_type = sig_decl
                .ReturnType()
                .map(|ret_ty| type_from_node(ret_ty.Type(), diag, tr))
                .unwrap_or(Type::Void);
            let arg_names = sig_decl
                .CallbackDeclarationParameter()
                .map(|a| {
                    a.DeclaredIdentifier()
                        .and_then(|x| parser::identifier_text(&x))
                        .unwrap_or_default()
                })
                .collect();
            r.property_declarations.insert(
                name,
                PropertyDeclaration {
                    property_type: Type::Callback(Arc::new(Function {
                        return_type,
                        args,
                        arg_names,
                    })),
                    node: Some(sig_decl.into()),
                    visibility: PropertyVisibility::InOut,
                    pure,
                    shadowed_name,
                    shadowable,
                    deprecated,
                    ..Default::default()
                },
            );
        }
    }

    pub(super) fn declare_functions(
        r: &mut Element,
        node: &syntax_nodes::Element,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) {
        let is_interface = matches!(r.base_type, ElementType::Interface(_));
        for func in node.Function() {
            #[cfg(feature = "slint-sc")]
            diag.slint_sc_error("Function declarations are", &func);
            let name =
                unwrap_or_continue!(parser::identifier_text(&func.DeclaredIdentifier()); diag);

            let member_decl = r.member_declaration(&name);
            if let MemberDeclaration::Conflict { existing_type, declared_in } = &member_decl {
                if matches!(existing_type, Type::Callback { .. } | Type::Function { .. }) {
                    diag.push_error(
                        cannot_override_message(None, &name, declared_in),
                        &func.DeclaredIdentifier(),
                    )
                } else {
                    diag.push_error(
                        format!("Cannot declare function '{name}' when a property with the same name exists"),
                        &func.DeclaredIdentifier(),
                    );
                }
                continue;
            }
            let source_name = name;
            let name = member_decl.register(r, &source_name, &func.DeclaredIdentifier(), diag);
            let shadowed_name = (name != source_name).then_some(source_name);

            let mut args = Vec::new();
            let mut arg_names = Vec::new();
            for a in func.ArgumentDeclaration() {
                args.push(type_from_node(a.Type(), diag, tr));
                let name =
                    unwrap_or_continue!(parser::identifier_text(&a.DeclaredIdentifier()); diag);
                if arg_names.contains(&name) {
                    diag.push_error(
                        format!("Duplicated argument name '{name}'"),
                        &a.DeclaredIdentifier(),
                    );
                }
                arg_names.push(name);
            }
            let return_type = func
                .ReturnType()
                .map_or(Type::Void, |ret_ty| type_from_node(ret_ty.Type(), diag, tr));

            let mut visibility = PropertyVisibility::Private;
            let mut pure = None;
            for token in func.children_with_tokens() {
                if token.kind() != SyntaxKind::Identifier {
                    continue;
                }
                match token.as_token().unwrap().text() {
                    "pure" => pure = Some(true),
                    "public" => {
                        visibility = PropertyVisibility::Public;
                        pure = pure.or(Some(false));
                    }
                    "protected" => {
                        visibility = PropertyVisibility::Protected;
                        pure = pure.or(Some(false));
                    }
                    _ => (),
                }
            }

            if is_interface && visibility != PropertyVisibility::Public {
                diag.push_error(
                    "Function declarations in an interface must be public".into(),
                    &func,
                );
            }

            let declaration = PropertyDeclaration {
                property_type: Type::Function(Arc::new(Function { return_type, args, arg_names })),
                node: Some(func.clone().into()),
                visibility,
                pure,
                shadowed_name,
                shadowable: shadowable_attribute(func.ShadowableAttribute(), tr, diag),
                deprecated: member_deprecation(func.PropertyDeprecation(), diag),
                ..Default::default()
            };

            match (is_interface, func.CodeBlock()) {
                (true, Some(code_block)) => {
                    diag.push_error(
                        "Function declarations in interfaces must not have a body".into(),
                        &code_block,
                    );
                    continue;
                }
                (true, None) => {
                    // Do not create a binding for this function, as it is just a declaration without body. It will be
                    // implemented by the component that implements the interface.
                    r.property_declarations.insert(name, declaration);
                    continue;
                }
                (_, None) => {
                    diag.push_error("Functions must have a code block".into(), &func);
                }
                (_, Some(_)) => {}
            }

            if r.bindings
                .0
                .insert(name.clone(), BindingExpression::new_uncompiled(func.clone().into()).into())
                .is_some()
            {
                assert!(diag.has_errors());
            }

            r.property_declarations.insert(name, declaration);
        }
    }

    pub(super) fn add_callback_handlers(
        r: &mut Element,
        node: &syntax_nodes::Element,
        diag: &mut BuildDiagnostics,
    ) {
        #[cfg(feature = "slint-sc")]
        let is_component_root = is_component_root(node);
        for con_node in node.CallbackConnection() {
            let unresolved_name = unwrap_or_continue!(parser::identifier_text(&con_node); diag);
            let lookup_result =
                r.lookup_property(&unresolved_name, PropertyLookupMode::ComponentLocal);
            #[cfg(feature = "slint-sc")]
            {
                // A callback declared in the file is in the subset by construction;
                // a builtin one only when marked in its declaration, which keeps
                // `init` and the rest of TouchArea out.
                if !r.is_user_declared_member(&unresolved_name) && !lookup_result.is_slint_sc {
                    diag.slint_sc_error(
                        &format!("The callback '{unresolved_name}' is"),
                        &con_node.child_token(SyntaxKind::Identifier).unwrap(),
                    );
                }
                // The application implements the callbacks of the root element,
                // so a handler here would be a second answer to one invocation.
                if is_component_root
                    && r.property_declarations
                        .get(lookup_result.internal_or_resolved_name().as_str())
                        .is_some_and(|d| d.node.is_some())
                {
                    diag.slint_sc_error(
                        "A handler for a callback declared on the root element is",
                        &con_node.child_token(SyntaxKind::Identifier).unwrap(),
                    );
                }
                if let Some(param) = con_node.DeclaredIdentifier().next() {
                    diag.slint_sc_error("Callback handler parameters are", &param);
                }
            }
            // Setting a handler on a deprecated callback from outside the declaring component warns,
            // like assigning a deprecated property does.
            let deprecation =
                lookup_result.deprecated.clone().filter(|_| !lookup_result.is_local_to_component);
            let resolved_name = lookup_result.internal_or_resolved_name();
            let property_type = lookup_result.property_type;
            if let Type::Callback(callback) = &property_type {
                let num_arg = con_node.DeclaredIdentifier().count();
                if num_arg > callback.args.len() {
                    diag.push_error(
                        format!(
                            "'{}' only has {} arguments, but {} were provided",
                            unresolved_name,
                            callback.args.len(),
                            num_arg
                        ),
                        &con_node.child_token(SyntaxKind::Identifier).unwrap(),
                    );
                }
            } else if property_type == Type::InferredCallback {
                // argument matching will happen later
            } else {
                if r.base_type != ElementType::Error {
                    diag.push_error(
                        format!("'{}' is not a callback in {}", unresolved_name, r.base_type),
                        &con_node.child_token(SyntaxKind::Identifier).unwrap(),
                    );
                }
                continue;
            }
            if let Some(message) = &deprecation {
                diag.push_member_deprecation_warning(
                    "callback",
                    &unresolved_name,
                    message,
                    &con_node.child_token(SyntaxKind::Identifier).unwrap(),
                );
            }
            match r.bindings.0.entry(resolved_name) {
                Entry::Vacant(e) => {
                    e.insert(BindingExpression::new_uncompiled(con_node.clone().into()).into());
                }
                Entry::Occupied(mut e) => {
                    // A global may implement a callback declared in another global: the
                    // callback is declared as a two-way alias (`callback foo <=> Other.foo;`)
                    // and also given a handler (`foo => { ... }`). The alias node stays on
                    // the declaration, and the handler takes the binding expression slot.
                    let is_global_alias = r.base_type == ElementType::Global
                        && matches!(
                            &e.get().borrow().expression,
                            Expression::Uncompiled(node) if node.kind() == SyntaxKind::TwoWayBinding
                        );
                    if is_global_alias {
                        // Keep the handler as the binding and point its span at the handler
                        // name, so a duplicate-implementation error refers to the
                        // implementation rather than the alias. The alias is recovered from
                        // the declaration node, so dropping it from the binding is fine.
                        let mut handler =
                            BindingExpression::new_uncompiled(con_node.clone().into());
                        if let Some(name) = con_node.child_token(SyntaxKind::Identifier) {
                            handler.span = Some(name.to_source_location());
                        }
                        e.insert(handler.into());
                    } else {
                        diag.push_error(
                            "Duplicated callback".into(),
                            &con_node.child_token(SyntaxKind::Identifier).unwrap(),
                        );
                    }
                }
            }
        }
    }

    pub(super) fn add_property_animations(
        r: &mut Element,
        node: &syntax_nodes::Element,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) {
        for anim in node.PropertyAnimation() {
            #[cfg(feature = "slint-sc")]
            diag.slint_sc_error("Animations are", &anim);
            if let Some(star) = anim.child_token(SyntaxKind::Star) {
                diag.push_error(
                    "catch-all property is only allowed within transitions".into(),
                    &star,
                )
            };
            for prop_name_token in anim.QualifiedName() {
                match QualifiedTypeName::from_node(prop_name_token.clone()).members.as_slice() {
                    [unresolved_prop_name] => {
                        if r.base_type == ElementType::Error {
                            continue;
                        };
                        let lookup_result = r.lookup_property(
                            unresolved_prop_name,
                            PropertyLookupMode::ComponentLocal,
                        );
                        let valid_assign = lookup_result.is_valid_for_assignment();
                        let binding_name = lookup_result.internal_or_resolved_name();
                        if let Some(anim_element) = animation_element_from_node(
                            &anim,
                            &prop_name_token,
                            lookup_result.property_type.clone(),
                            diag,
                            tr,
                        ) {
                            if !valid_assign {
                                diag.push_error(
                                    format!(
                                        "Cannot animate '{}' property '{}'",
                                        lookup_result.property_visibility, unresolved_prop_name
                                    ),
                                    &prop_name_token,
                                );
                            }

                            if unresolved_prop_name != lookup_result.resolved_name.as_ref() {
                                diag.push_property_deprecation_warning(
                                    unresolved_prop_name,
                                    &lookup_result.resolved_name,
                                    &prop_name_token,
                                );
                            } else if let Some(message) = lookup_result
                                .deprecated
                                .as_ref()
                                .filter(|_| !lookup_result.is_local_to_component)
                            {
                                diag.push_member_deprecation_warning(
                                    "property",
                                    unresolved_prop_name,
                                    message,
                                    &prop_name_token,
                                );
                            }

                            let expr_binding =
                                r.bindings.0.entry(binding_name).or_insert_with(|| {
                                    let mut r = BindingExpression::from(Expression::Invalid);
                                    r.priority = 1;
                                    r.from_source = true;
                                    r.span = Some(prop_name_token.to_source_location());
                                    r.into()
                                });
                            if expr_binding
                                .get_mut()
                                .animation
                                .replace(PropertyAnimation::Static(anim_element))
                                .is_some()
                            {
                                diag.push_error("Duplicated animation".into(), &prop_name_token)
                            }
                        }
                    }
                    _ => diag.push_error(
                        "Can only refer to property in the current element".into(),
                        &prop_name_token,
                    ),
                }
            }
        }
    }

    pub(super) fn add_change_callbacks(
        r: &mut Element,
        node: &syntax_nodes::Element,
        diag: &mut BuildDiagnostics,
    ) {
        for ch in node.PropertyChangedCallback() {
            #[cfg(feature = "slint-sc")]
            diag.slint_sc_error("Change callbacks are", &ch);
            let Some(prop) = parser::identifier_text(&ch.DeclaredIdentifier()) else { continue };
            let lookup_result = r.lookup_property(&prop, PropertyLookupMode::ComponentLocal);
            if !lookup_result.is_valid() {
                if r.base_type != ElementType::Error {
                    diag.push_error(
                        format!("Property '{prop}' does not exist"),
                        &ch.DeclaredIdentifier(),
                    );
                }
            } else if !lookup_result.property_type.is_property_type() {
                let what = match lookup_result.property_type {
                    Type::Function { .. } => "a function",
                    Type::Callback { .. } => "a callback",
                    _ => "not a property",
                };
                diag.push_error(
                    format!(
                        "Change callback can only be set on properties, and '{prop}' is {what}"
                    ),
                    &ch.DeclaredIdentifier(),
                );
            } else if lookup_result.property_visibility == PropertyVisibility::Private
                && !lookup_result.is_local_to_component
            {
                diag.push_error(
                    format!("Change callback on a private property '{prop}'"),
                    &ch.DeclaredIdentifier(),
                );
            }
            let handler = Expression::Uncompiled(ch.clone().into());
            match r.change_callbacks.entry(lookup_result.internal_or_resolved_name()) {
                Entry::Vacant(e) => {
                    e.insert(vec![handler].into());
                }
                Entry::Occupied(mut e) => {
                    diag.push_error(
                        format!("Duplicated change callback on '{prop}'"),
                        &ch.DeclaredIdentifier(),
                    );
                    e.get_mut().get_mut().push(handler);
                }
            }
        }
    }
}
