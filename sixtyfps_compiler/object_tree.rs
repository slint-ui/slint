/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
 This module contains the intermediate representation of the code in the form of an object tree
*/

use crate::diagnostics::{FileDiagnostics, Spanned, SpannedWithSourceFile};
use crate::expression_tree::{Expression, ExpressionSpanned, NamedReference};
use crate::langtype::{NativeClass, Type};
use crate::parser::{identifier_text, syntax_nodes, SyntaxKind, SyntaxNodeWithSourceFile};
use crate::typeregister::TypeRegister;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// The full document (a complete file)
#[derive(Default, Debug)]
pub struct Document {
    //     node: SyntaxNode,
    pub inner_components: Vec<Rc<Component>>,
    pub inner_structs: Vec<Type>,
    pub root_component: Rc<Component>,
    pub local_registry: TypeRegister,
    exports: Exports,
}

impl Document {
    pub fn from_node(
        node: syntax_nodes::Document,
        diag: &mut FileDiagnostics,
        parent_registry: &Rc<RefCell<TypeRegister>>,
    ) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Document);

        let mut local_registry = TypeRegister::new(parent_registry);
        let mut inner_components = vec![];
        let mut inner_structs = vec![];

        let mut process_component =
            |n: syntax_nodes::Component,
             diag: &mut FileDiagnostics,
             local_registry: &mut TypeRegister| {
                let compo = Component::from_node(n, diag, local_registry);
                local_registry.add(compo.clone());
                inner_components.push(compo);
            };
        let mut process_struct =
            |n: syntax_nodes::StructDeclaration,
             diag: &mut FileDiagnostics,
             local_registry: &mut TypeRegister| {
                let mut ty = type_struct_from_node(n.ObjectType(), diag, local_registry);
                if let Type::Object { name, .. } = &mut ty {
                    *name = identifier_text(&n.DeclaredIdentifier());
                } else {
                    assert!(diag.has_error());
                    return;
                }
                local_registry.insert_type(ty.clone());
                inner_structs.push(ty);
            };

        for n in node.children() {
            match n.kind() {
                SyntaxKind::Component => process_component(n.into(), diag, &mut local_registry),
                SyntaxKind::StructDeclaration => {
                    process_struct(n.into(), diag, &mut local_registry)
                }
                SyntaxKind::ExportsList => {
                    for n in n.children() {
                        match n.kind() {
                            SyntaxKind::Component => {
                                process_component(n.into(), diag, &mut local_registry)
                            }
                            SyntaxKind::StructDeclaration => {
                                process_struct(n.into(), diag, &mut local_registry)
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            };
        }
        let exports = Exports::from_node(&node, &inner_components, &parent_registry, diag);

        Document {
            // FIXME: one should use the `component` hint instead of always returning the last
            root_component: inner_components.last().cloned().unwrap_or_default(),
            inner_components,
            inner_structs,
            local_registry,
            exports,
        }
    }

    pub fn exports(&self) -> &Vec<(String, Rc<Component>)> {
        &self.exports.0
    }
}

/// A component is a type in the language which can be instantiated,
/// Or is materialized for repeated expression.
#[derive(Default, Debug)]
pub struct Component {
    //     node: SyntaxNode,
    pub id: String,
    pub root_element: ElementRc,

    /// The parent element within the parent component if this component represents a repeated element
    pub parent_element: Weak<RefCell<Element>>,

    /// List of elements that are not attached to the root anymore because they have been
    /// optimized away, but their properties may still be in use
    pub optimized_elements: RefCell<Vec<ElementRc>>,

    /// Map of resources to referenced in the sources, indexed by their absolute path on
    /// disk on the build system and valued by a unique integer id, that can be used by the
    /// generator for symbol generation.
    pub referenced_file_resources: RefCell<HashMap<String, usize>>,

    /// Copied from the compiler configuration, generators can use this to detect if file resources
    /// should be embedded.
    pub embed_file_resources: Cell<bool>,

    /// All layouts in this component
    pub layouts: RefCell<crate::layout::LayoutVec>,

    /// When creating this component and inserting "children", append them to the children of
    /// the element pointer to by this field.
    pub child_insertion_point: RefCell<Option<ElementRc>>,

    /// Code to be inserted into the constructor
    pub setup_code: RefCell<Vec<Expression>>,

    /// All the globals used by this component and its children.
    /// FIXME: can we have cycle?
    pub used_global: RefCell<Vec<Rc<Component>>>,
}

impl Component {
    pub fn from_node(
        node: syntax_nodes::Component,
        diag: &mut FileDiagnostics,
        tr: &TypeRegister,
    ) -> Rc<Self> {
        let mut child_insertion_point = None;
        let c = Component {
            id: identifier_text(&node.DeclaredIdentifier()).unwrap_or_default(),
            root_element: Element::from_node(
                node.Element(),
                "root".into(),
                Type::Invalid,
                &mut child_insertion_point,
                diag,
                tr,
            ),
            child_insertion_point: RefCell::new(child_insertion_point),
            ..Default::default()
        };
        let c = Rc::new(c);
        let weak = Rc::downgrade(&c);
        recurse_elem(&c.root_element, &(), &mut |e, _| {
            e.borrow_mut().enclosing_component = weak.clone()
        });
        c
    }

    /// This component is a global component introduced with the "global" keyword
    pub fn is_global(&self) -> bool {
        self.root_element.borrow().base_type == Type::Void
    }
}

#[derive(Clone, Debug, Default)]
pub struct PropertyDeclaration {
    pub property_type: Type,
    pub type_node: Option<SyntaxNodeWithSourceFile>,
    /// Tells if getter and setter will be added to expose in the native language API
    pub expose_in_public_api: bool,
    /// Public API property exposed as an alias: it shouldn't be generated but instead forward to the alias.
    pub is_alias: Option<NamedReference>,
}

impl From<Type> for PropertyDeclaration {
    fn from(ty: Type) -> Self {
        PropertyDeclaration { property_type: ty, ..Self::default() }
    }
}

/// An Element is an instentation of a Component
#[derive(Default, Debug)]
pub struct Element {
    /// The id as named in the original .60 file.
    ///
    /// Note that it can only be used for lookup before inlining.
    /// After inlining there can be duplicated id in the component.
    /// The id are then re-assigned unique id in the assign_id pass
    pub id: String,
    //pub base: QualifiedTypeName,
    pub base_type: crate::langtype::Type,
    /// Currently contains also the signals. FIXME: should that be changed?
    pub bindings: HashMap<String, ExpressionSpanned>,
    pub children: Vec<ElementRc>,
    /// The component which contains this element.
    pub enclosing_component: Weak<Component>,

    pub property_declarations: HashMap<String, PropertyDeclaration>,

    pub property_animations: HashMap<String, ElementRc>,

    /// Tis element is part of a `for <xxx> in <model>:
    pub repeated: Option<RepeatedElementInfo>,

    pub states: Vec<State>,
    pub transitions: Vec<Transition>,

    pub child_of_layout: bool,

    /// The AST node, if available
    pub node: Option<syntax_nodes::Element>,
}

impl Spanned for Element {
    fn span(&self) -> crate::diagnostics::Span {
        self.node.as_ref().map(|n| n.span()).unwrap_or_default()
    }
}

impl SpannedWithSourceFile for Element {
    fn source_file(&self) -> Option<&Rc<std::path::PathBuf>> {
        self.node.as_ref().map(|n| n.0.source_file.as_ref()).flatten()
    }
}

#[derive(Debug, Clone)]
pub struct ListViewInfo {
    pub viewport_y: NamedReference,
    pub viewport_height: NamedReference,
    pub viewport_width: NamedReference,
    pub listview_height: NamedReference,
    pub listview_width: NamedReference,
}

#[derive(Debug, Clone)]
/// If the parent element is a repeated element, this has information about the models
pub struct RepeatedElementInfo {
    pub model: Expression,
    pub model_data_id: String,
    pub index_id: String,
    /// A conditional element is just a for whose model is a bolean expression
    ///
    /// When this is true, the model is of type bolean instead of Model
    pub is_conditional_element: bool,
    /// When the for is the delegate of a ListView
    pub is_listview: Option<ListViewInfo>,
}

pub type ElementRc = Rc<RefCell<Element>>;

impl Element {
    pub fn from_node(
        node: syntax_nodes::Element,
        id: String,
        parent_type: Type,
        component_child_insertion_point: &mut Option<ElementRc>,
        diag: &mut FileDiagnostics,
        tr: &TypeRegister,
    ) -> ElementRc {
        let (base_type, name_for_looup_errors) = if let Some(base_node) = node.QualifiedName() {
            let base = QualifiedTypeName::from_node(base_node.clone());
            let base_string = base.to_string();
            let base_type = match parent_type.lookup_type_for_child_element(&base_string, tr) {
                Ok(ty) => ty,
                Err(err) => {
                    diag.push_error(err, &base_node);
                    return ElementRc::default();
                }
            };
            assert!(base_type.is_object_type());
            if let Type::Component(c) = &base_type {
                if c.is_global() {
                    diag.push_error(
                        "Cannot create an instance of a global component".into(),
                        &base_node,
                    )
                }
            }
            (base_type, format!(" in {}", base))
        } else {
            if parent_type != Type::Invalid {
                // This should normally never happen because the parser does not allow for this
                assert!(diag.has_error());
                return ElementRc::default();
            }

            // This must be a global component it can only have properties and signal
            let mut error_on = |node: &dyn Spanned, what: &str| {
                diag.push_error(format!("A global component cannot have {}", what), node);
            };
            node.SubElement().for_each(|n| error_on(&n, "sub elements"));
            node.RepeatedElement().for_each(|n| error_on(&n, "sub elements"));
            node.ChildrenPlaceholder().map(|n| error_on(&n, "sub elements"));
            node.SignalConnection().for_each(|n| error_on(&n, "signal connections"));
            node.PropertyAnimation().for_each(|n| error_on(&n, "animations"));
            node.States().for_each(|n| error_on(&n, "states"));
            node.Transitions().for_each(|n| error_on(&n, "transitions"));
            (Type::Void, String::new())
        };
        let mut r = Element {
            id,
            base_type: base_type.clone(),
            node: Some(node.clone()),
            ..Default::default()
        };

        for prop_decl in node.PropertyDeclaration() {
            let type_node = prop_decl.Type();
            let prop_type = type_from_node(type_node.clone(), diag, tr);
            let prop_name = identifier_text(&prop_decl.DeclaredIdentifier()).unwrap();
            if !matches!(r.lookup_property(&prop_name), Type::Invalid) {
                diag.push_error(
                    format!("Cannot override property '{}'", prop_name),
                    &prop_decl.DeclaredIdentifier().child_token(SyntaxKind::Identifier).unwrap(),
                )
            }

            r.property_declarations.insert(
                prop_name.clone(),
                PropertyDeclaration {
                    property_type: prop_type,
                    type_node: Some(type_node.into()),
                    ..Default::default()
                },
            );

            if let Some(csn) = prop_decl.BindingExpression() {
                if r.bindings
                    .insert(prop_name.clone(), ExpressionSpanned::new_uncompiled(csn.into()))
                    .is_some()
                {
                    diag.push_error(
                        "Duplicated property binding".into(),
                        &prop_decl.DeclaredIdentifier(),
                    );
                }
            }
            if let Some(csn) = prop_decl.TwoWayBinding() {
                if r.bindings
                    .insert(prop_name, ExpressionSpanned::new_uncompiled(csn.into()))
                    .is_some()
                {
                    diag.push_error(
                        "Duplicated property binding".into(),
                        &prop_decl.DeclaredIdentifier(),
                    );
                }
            }
        }

        r.parse_bindings(
            &name_for_looup_errors,
            node.Binding().filter_map(|b| {
                Some((b.child_token(SyntaxKind::Identifier)?, b.BindingExpression().into()))
            }),
            diag,
        );
        r.parse_bindings(
            &name_for_looup_errors,
            node.TwoWayBinding()
                .filter_map(|b| Some((b.child_token(SyntaxKind::Identifier)?, b.into()))),
            diag,
        );

        match &r.base_type {
            Type::Builtin(builtin_base) => {
                for (prop, expr) in &builtin_base.default_bindings {
                    r.bindings.entry(prop.clone()).or_insert(expr.clone().into());
                }
            }
            _ => {}
        }

        for sig_decl in node.SignalDeclaration() {
            let name = identifier_text(&sig_decl.DeclaredIdentifier()).unwrap();
            let args = sig_decl.Type().map(|node_ty| type_from_node(node_ty, diag, tr)).collect();
            r.property_declarations.insert(
                name,
                PropertyDeclaration {
                    property_type: Type::Signal { args },
                    type_node: Some(sig_decl.into()),
                    ..Default::default()
                },
            );
        }

        for con_node in node.SignalConnection() {
            let name = match identifier_text(&con_node) {
                Some(x) => x,
                None => continue,
            };
            let prop_type = r.lookup_property(&name);
            if let Type::Signal { args } = prop_type {
                let num_arg = con_node.DeclaredIdentifier().count();
                if num_arg > args.len() {
                    diag.push_error(
                        format!(
                            "'{}' only has {} arguments, but {} were provided",
                            name,
                            args.len(),
                            num_arg
                        ),
                        &con_node.child_token(SyntaxKind::Identifier).unwrap(),
                    );
                }
                if r.bindings
                    .insert(name, ExpressionSpanned::new_uncompiled(con_node.clone().into()))
                    .is_some()
                {
                    diag.push_error(
                        "Duplicated signal".into(),
                        &con_node.child_token(SyntaxKind::Identifier).unwrap(),
                    );
                }
            } else {
                diag.push_error(
                    format!("'{}' is not a signal{}", name, name_for_looup_errors),
                    &con_node.child_token(SyntaxKind::Identifier).unwrap(),
                );
            }
        }

        for anim in node.PropertyAnimation() {
            if let Some(star) = anim.child_token(SyntaxKind::Star) {
                diag.push_error(
                    "catch-all property is only allowed within transitions".into(),
                    &star,
                )
            };
            for prop_name_token in anim.QualifiedName() {
                match QualifiedTypeName::from_node(prop_name_token.clone()).members.as_slice() {
                    [prop_name] => {
                        let prop_type = r.lookup_property(&prop_name);
                        if let Some(anim_element) = animation_element_from_node(
                            &anim,
                            &prop_name_token,
                            prop_type,
                            diag,
                            tr,
                        ) {
                            if r.property_animations
                                .insert(prop_name.clone(), anim_element)
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

        let mut children_placeholder = None;
        let r = ElementRc::new(RefCell::new(r));

        for se in node.children() {
            if se.kind() == SyntaxKind::SubElement {
                let id = identifier_text(&se).unwrap_or_default();
                if matches!(id.as_ref(), "parent" | "self" | "root") {
                    diag.push_error(
                        format!("'{}' is a reserved id", id),
                        &se.child_token(SyntaxKind::Identifier).unwrap(),
                    )
                }
                if let Some(element_node) = se.child_node(SyntaxKind::Element) {
                    let parent_type = r.borrow().base_type.clone();
                    r.borrow_mut().children.push(Element::from_node(
                        element_node.into(),
                        id,
                        parent_type,
                        component_child_insertion_point,
                        diag,
                        tr,
                    ));
                } else {
                    assert!(diag.has_error());
                }
            } else if se.kind() == SyntaxKind::RepeatedElement {
                let rep = Element::from_repeated_node(
                    se.into(),
                    &r,
                    component_child_insertion_point,
                    diag,
                    tr,
                );
                r.borrow_mut().children.push(rep);
            } else if se.kind() == SyntaxKind::ConditionalElement {
                let rep = Element::from_conditional_node(
                    se.into(),
                    r.borrow().base_type.clone(),
                    component_child_insertion_point,
                    diag,
                    tr,
                );
                r.borrow_mut().children.push(rep);
            } else if se.kind() == SyntaxKind::ChildrenPlaceholder {
                if children_placeholder.is_some() {
                    diag.push_error(
                        "The $children placeholder can only appear once in an element".into(),
                        &se,
                    )
                } else {
                    children_placeholder = Some(se.clone());
                }
            }
        }

        if let Some(children_placeholder) = children_placeholder {
            if component_child_insertion_point.is_some() {
                diag.push_error(
                    "The $children placeholder can only appear once in an element hierarchy".into(),
                    &children_placeholder,
                )
            } else {
                *component_child_insertion_point = Some(r.clone());
            }
        }

        for state in node.States().flat_map(|s| s.State()) {
            let s = State {
                id: identifier_text(&state.DeclaredIdentifier()).unwrap_or_default(),
                condition: state.Expression().map(|e| Expression::Uncompiled(e.into())),
                property_changes: state
                    .StatePropertyChange()
                    .map(|s| {
                        let (ne, _) =
                            lookup_property_from_qualified_name(s.QualifiedName(), &r, diag);
                        (ne, Expression::Uncompiled(s.BindingExpression().into()))
                    })
                    .collect(),
            };
            r.borrow_mut().states.push(s);
        }

        for trs in node.Transitions().flat_map(|s| s.Transition()) {
            if let Some(star) = trs.child_token(SyntaxKind::Star) {
                diag.push_error("TODO: catch-all not yet implemented".into(), &star);
            };
            let trans = Transition {
                is_out: identifier_text(&trs).unwrap_or_default() == "out",
                state_id: identifier_text(&trs.DeclaredIdentifier()).unwrap_or_default(),
                property_animations: trs
                    .PropertyAnimation()
                    .flat_map(|pa| pa.QualifiedName().map(move |qn| (pa.clone(), qn)))
                    .filter_map(|(pa, qn)| {
                        let (ne, prop_type) =
                            lookup_property_from_qualified_name(qn.clone(), &r, diag);
                        if prop_type == Type::Invalid {
                            debug_assert!(diag.has_error()); // Error should have been reported already
                            return None;
                        }
                        animation_element_from_node(&pa, &qn, prop_type, diag, tr)
                            .map(|anim_element| (ne, anim_element))
                    })
                    .collect(),
            };
            r.borrow_mut().transitions.push(trans);
        }

        r
    }

    fn from_repeated_node(
        node: syntax_nodes::RepeatedElement,
        parent: &ElementRc,
        component_child_insertion_point: &mut Option<ElementRc>,
        diag: &mut FileDiagnostics,
        tr: &TypeRegister,
    ) -> ElementRc {
        let is_listview = if parent.borrow().base_type.to_string() == "ListView" {
            Some(ListViewInfo {
                viewport_y: NamedReference::new(parent, "viewport_y"),
                viewport_height: NamedReference::new(parent, "viewport_height"),
                viewport_width: NamedReference::new(parent, "viewport_width"),
                listview_height: NamedReference::new(parent, "visible_height"),
                listview_width: NamedReference::new(parent, "visible_width"),
            })
        } else {
            None
        };
        let rei = RepeatedElementInfo {
            model: Expression::Uncompiled(node.Expression().into()),
            model_data_id: node
                .DeclaredIdentifier()
                .and_then(|n| identifier_text(&n))
                .unwrap_or_default(),
            index_id: node.RepeatedIndex().and_then(|r| identifier_text(&r)).unwrap_or_default(),
            is_conditional_element: false,
            is_listview,
        };
        let e = Element::from_node(
            node.Element(),
            String::new(),
            parent.borrow().base_type.to_owned(),
            component_child_insertion_point,
            diag,
            tr,
        );
        e.borrow_mut().repeated = Some(rei);
        e
    }

    fn from_conditional_node(
        node: syntax_nodes::ConditionalElement,
        parent_type: Type,
        component_child_insertion_point: &mut Option<ElementRc>,
        diag: &mut FileDiagnostics,
        tr: &TypeRegister,
    ) -> ElementRc {
        let rei = RepeatedElementInfo {
            model: Expression::Uncompiled(node.Expression().into()),
            model_data_id: String::new(),
            index_id: String::new(),
            is_conditional_element: true,
            is_listview: None,
        };
        let e = Element::from_node(
            node.Element(),
            String::new(),
            parent_type,
            component_child_insertion_point,
            diag,
            tr,
        );
        e.borrow_mut().repeated = Some(rei);
        e
    }

    /// Return the type of a property in this element or its base
    pub fn lookup_property(&self, name: &str) -> Type {
        self.property_declarations
            .get(name)
            .cloned()
            .map(|decl| decl.property_type)
            .unwrap_or_else(|| self.base_type.lookup_property(name))
    }

    /// Return the Span of this element in the AST for error reporting
    pub fn span(&self) -> crate::diagnostics::Span {
        self.node.as_ref().map(|n| n.span()).unwrap_or_default()
    }

    fn parse_bindings(
        &mut self,
        name_for_lookup_error: &str,
        bindings: impl Iterator<
            Item = (crate::parser::SyntaxTokenWithSourceFile, SyntaxNodeWithSourceFile),
        >,
        diag: &mut FileDiagnostics,
    ) {
        for (name_token, b) in bindings {
            let name = crate::parser::normalize_identifier(name_token.text());
            let prop_type = self.lookup_property(&name);
            if !prop_type.is_property_type() {
                diag.push_error(
                    match prop_type {
                        Type::Invalid => {
                            format!("Unknown property {}{}", name, name_for_lookup_error)
                        }
                        Type::Signal { .. } => {
                            format!("'{}' is a signal. Use `=>` to connect", name)
                        }
                        _ => format!("Cannot assign to {}{}", name, name_for_lookup_error),
                    },
                    &name_token,
                );
            }
            if self.bindings.insert(name, ExpressionSpanned::new_uncompiled(b)).is_some() {
                diag.push_error("Duplicated property binding".into(), &name_token);
            }
        }
    }

    pub fn native_class(&self) -> Option<Rc<NativeClass>> {
        let mut base_type = self.base_type.clone();
        loop {
            match &base_type {
                Type::Component(component) => {
                    base_type = component.root_element.clone().borrow().base_type.clone();
                }
                Type::Builtin(builtin) => break Some(builtin.native_class.clone()),
                Type::Native(native) => break Some(native.clone()),
                _ => break None,
            }
        }
    }
}

fn type_from_node(node: syntax_nodes::Type, diag: &mut FileDiagnostics, tr: &TypeRegister) -> Type {
    if let Some(qualified_type_node) = node.QualifiedName() {
        let qualified_type = QualifiedTypeName::from_node(qualified_type_node.clone());

        let prop_type = tr.lookup_qualified(&qualified_type.members);

        if prop_type == Type::Invalid {
            diag.push_error(
                format!("Unknown type '{}'", qualified_type.to_string()),
                &qualified_type_node,
            );
        }
        prop_type
    } else if let Some(object_node) = node.ObjectType() {
        type_struct_from_node(object_node, diag, tr)
    } else if let Some(array_node) = node.ArrayType() {
        Type::Array(Box::new(type_from_node(array_node.Type(), diag, tr)))
    } else {
        assert!(diag.has_error());
        Type::Invalid
    }
}

fn type_struct_from_node(
    object_node: syntax_nodes::ObjectType,
    diag: &mut FileDiagnostics,
    tr: &TypeRegister,
) -> Type {
    let fields = object_node
        .ObjectTypeMember()
        .map(|member| {
            (identifier_text(&member).unwrap_or_default(), type_from_node(member.Type(), diag, tr))
        })
        .collect();
    Type::Object { fields, name: None }
}

fn animation_element_from_node(
    anim: &syntax_nodes::PropertyAnimation,
    prop_name: &syntax_nodes::QualifiedName,
    prop_type: Type,
    diag: &mut FileDiagnostics,
    tr: &TypeRegister,
) -> Option<ElementRc> {
    let anim_type = tr.property_animation_type_for_property(prop_type);
    if !matches!(anim_type, Type::Builtin(..)) {
        diag.push_error(
            format!("'{}' is not an animatable property", prop_name.text().to_string().trim()),
            prop_name,
        );
        None
    } else {
        let name_for_lookup_errors =
            format!(" in {}", anim_type.as_builtin().native_class.class_name);
        let mut anim_element =
            Element { id: "".into(), base_type: anim_type, node: None, ..Default::default() };
        anim_element.parse_bindings(
            &name_for_lookup_errors,
            anim.Binding().filter_map(|b| {
                Some((b.child_token(SyntaxKind::Identifier)?, b.BindingExpression().into()))
            }),
            diag,
        );
        Some(Rc::new(RefCell::new(anim_element)))
    }
}

#[derive(Default, Debug, Clone)]
pub struct QualifiedTypeName {
    members: Vec<String>,
}

impl QualifiedTypeName {
    pub fn from_node(node: syntax_nodes::QualifiedName) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::QualifiedName);
        let members = node
            .children_with_tokens()
            .filter(|n| n.kind() == SyntaxKind::Identifier)
            .filter_map(|x| x.as_token().map(|x| crate::parser::normalize_identifier(x.text())))
            .collect();
        Self { members }
    }
}

impl std::fmt::Display for QualifiedTypeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.members.join("."))
    }
}

/// Return a NamedReference, if the reference is invalid, there will be a diagnostic
fn lookup_property_from_qualified_name(
    node: syntax_nodes::QualifiedName,
    r: &Rc<RefCell<Element>>,
    diag: &mut FileDiagnostics,
) -> (NamedReference, Type) {
    let qualname = QualifiedTypeName::from_node(node.clone());
    match qualname.members.as_slice() {
        [prop_name] => {
            let ty = r.borrow().lookup_property(prop_name.as_ref());
            if !ty.is_property_type() {
                diag.push_error(format!("'{}' is not a valid property", qualname), &node);
            }
            (NamedReference { element: Rc::downgrade(&r), name: prop_name.clone() }, ty)
        }
        [elem_id, prop_name] => {
            let (element, ty) = if let Some(element) = find_element_by_id(&r, elem_id.as_ref()) {
                let ty = element.borrow().lookup_property(prop_name.as_ref());
                if !ty.is_property_type() {
                    diag.push_error(format!("'{}' not found in '{}'", prop_name, elem_id), &node);
                }
                (Rc::downgrade(&element), ty)
            } else {
                diag.push_error(format!("'{}' is not a valid element id", elem_id), &node);
                (Weak::new(), Type::Invalid)
            };
            (NamedReference { element, name: prop_name.clone() }, ty)
        }
        _ => {
            diag.push_error(format!("'{}' is not a valid property", qualname), &node);
            (NamedReference { element: Default::default(), name: String::default() }, Type::Invalid)
        }
    }
}

/// FIXME: this is duplicated the resolving pass. Also, we should use a hash table
fn find_element_by_id(e: &ElementRc, name: &str) -> Option<ElementRc> {
    if e.borrow().id == name {
        return Some(e.clone());
    }
    for x in &e.borrow().children {
        if x.borrow().repeated.is_some() {
            continue;
        }
        if let Some(x) = find_element_by_id(x, name) {
            return Some(x);
        }
    }

    None
}

/// Call the visitor for each children of the element recursively, starting with the element itself
///
/// The state returned by the visitor is passed to the children
pub fn recurse_elem<State>(
    elem: &ElementRc,
    state: &State,
    vis: &mut impl FnMut(&ElementRc, &State) -> State,
) {
    let state = vis(elem, state);
    for sub in &elem.borrow().children {
        recurse_elem(sub, &state, vis);
    }
}

/// Same as [`recurse_elem`] but include the elements form sub_components
pub fn recurse_elem_including_sub_components<State>(
    elem: &ElementRc,
    state: &State,
    vis: &mut impl FnMut(&ElementRc, &State) -> State,
) {
    let state = vis(elem, state);
    for sub in &elem.borrow().children {
        recurse_elem(sub, &state, &mut |elem, state| {
            if elem.borrow().repeated.is_some() {
                if let Type::Component(base) = &elem.borrow().base_type {
                    recurse_elem_including_sub_components(&base.root_element, state, vis);
                }
            }
            vis(elem, state)
        });
    }
}

/// Same as recurse_elem, but will take the children from the element as to not keep the element borrow
pub fn recurse_elem_no_borrow<State>(
    elem: &ElementRc,
    state: &State,
    vis: &mut impl FnMut(&ElementRc, &State) -> State,
) {
    let state = vis(elem, state);
    let children = std::mem::take(&mut elem.borrow_mut().children);
    for sub in &children {
        recurse_elem_no_borrow(sub, &state, vis);
    }
    elem.borrow_mut().children = children;
}

/// Same as [`recurse_elem`] but include the elements form sub_components
pub fn recurse_elem_including_sub_components_no_borrow<State>(
    elem: &ElementRc,
    state: &State,
    vis: &mut impl FnMut(&ElementRc, &State) -> State,
) {
    let state = vis(elem, state);
    for sub in &elem.borrow().children {
        recurse_elem_no_borrow(sub, &state, &mut |elem, state| {
            if elem.borrow().repeated.is_some() {
                if let Type::Component(base) = &elem.borrow().base_type {
                    recurse_elem_including_sub_components_no_borrow(&base.root_element, state, vis);
                }
            }
            vis(elem, state)
        });
    }
}

/// This visit the binding attached to this element, but does not recurse in children elements
/// Also does not recurse within the expressions.
///
/// This code will temporarily move the bindings or states member so it can call the visitor without
/// maintaining a borrow on the RefCell.
pub fn visit_element_expressions(
    elem: &ElementRc,
    mut vis: impl FnMut(&mut Expression, Option<&str>, &dyn Fn() -> Type),
) {
    let repeated = std::mem::take(&mut elem.borrow_mut().repeated);
    if let Some(mut r) = repeated {
        let is_conditional_element = r.is_conditional_element;
        vis(&mut r.model, None, &|| if is_conditional_element { Type::Bool } else { Type::Model });
        elem.borrow_mut().repeated = Some(r)
    }
    let mut bindings = std::mem::take(&mut elem.borrow_mut().bindings);
    for (name, expr) in &mut bindings {
        vis(expr, Some(name.as_str()), &|| elem.borrow().lookup_property(name));
    }
    elem.borrow_mut().bindings = bindings;
    let mut states = std::mem::take(&mut elem.borrow_mut().states);
    for s in &mut states {
        if let Some(cond) = s.condition.as_mut() {
            vis(cond, None, &|| Type::Bool)
        }
        for (ne, e) in &mut s.property_changes {
            vis(e, Some(ne.name.as_ref()), &|| {
                ne.element.upgrade().unwrap().borrow().lookup_property(ne.name.as_ref())
            });
        }
    }
    elem.borrow_mut().states = states;

    let property_animations = std::mem::take(&mut elem.borrow_mut().property_animations);
    for anim_elem in property_animations.values() {
        let mut bindings = std::mem::take(&mut anim_elem.borrow_mut().bindings);
        for (name, expr) in &mut bindings {
            vis(expr, Some(name.as_str()), &|| anim_elem.borrow().lookup_property(name));
        }
        anim_elem.borrow_mut().bindings = bindings;
    }
    elem.borrow_mut().property_animations = property_animations;
}

pub fn visit_all_named_references(elem: &ElementRc, mut vis: impl FnMut(&mut NamedReference)) {
    fn recurse_expression(expr: &mut Expression, vis: &mut impl FnMut(&mut NamedReference)) {
        expr.visit_mut(|sub| recurse_expression(sub, vis));
        match expr {
            Expression::PropertyReference(r) | Expression::SignalReference(r) => vis(r),
            Expression::TwoWayBinding(r, _) => vis(r),
            // This is not really a named reference, but the result is the same, it need to be updated
            // FIXME: this should probably be lowered into a PropertyReference
            Expression::RepeaterModelReference { element }
            | Expression::RepeaterIndexReference { element } => {
                let mut nc = NamedReference { element: element.clone(), name: "$model".into() };
                vis(&mut nc);
                debug_assert!(nc.element.upgrade().unwrap().borrow().repeated.is_some());
                *element = nc.element;
            }
            _ => {}
        }
    }
    visit_element_expressions(elem, |expr, _, _| recurse_expression(expr, &mut vis));
    let mut states = std::mem::take(&mut elem.borrow_mut().states);
    for s in &mut states {
        for (r, _) in &mut s.property_changes {
            vis(r);
        }
    }
    elem.borrow_mut().states = states;
    let mut transitions = std::mem::take(&mut elem.borrow_mut().transitions);
    for t in &mut transitions {
        for (r, _) in &mut t.property_animations {
            vis(r)
        }
    }
    elem.borrow_mut().transitions = transitions;
    let mut repeated = std::mem::take(&mut elem.borrow_mut().repeated);
    if let Some(r) = &mut repeated {
        if let Some(lv) = &mut r.is_listview {
            vis(&mut lv.viewport_y);
            vis(&mut lv.viewport_height);
            vis(&mut lv.viewport_width);
            vis(&mut lv.listview_height);
            vis(&mut lv.listview_width);
        }
    }
    elem.borrow_mut().repeated = repeated;
}

#[derive(Debug, Clone)]
pub struct State {
    pub id: String,
    pub condition: Option<Expression>,
    pub property_changes: Vec<(NamedReference, Expression)>,
}

#[derive(Debug)]
pub struct Transition {
    /// false for 'to', true for 'out'
    pub is_out: bool,
    pub state_id: String,
    pub property_animations: Vec<(NamedReference, ElementRc)>,
}

#[derive(Debug, Clone)]
pub struct NamedExport {
    pub internal_name: String,
    pub exported_name: String,
}

#[derive(Default, Debug, derive_more::Deref)]
pub struct Exports(Vec<(String, Rc<Component>)>);

impl Exports {
    pub fn from_node(
        doc: &syntax_nodes::Document,
        inner_components: &Vec<Rc<Component>>,
        type_registry: &Rc<RefCell<TypeRegister>>,
        diag: &mut FileDiagnostics,
    ) -> Self {
        let mut exports = doc
            .ExportsList()
            .flat_map(|exports| exports.ExportSpecifier())
            .filter_map(|export_specifier| {
                let internal_name = match identifier_text(&export_specifier.ExportIdentifier()) {
                    Some(name) => name,
                    _ => {
                        diag.push_error(
                            "Missing internal name for export".to_owned(),
                            &export_specifier.ExportIdentifier(),
                        );
                        return None;
                    }
                };
                let exported_name = match export_specifier.ExportName() {
                    Some(ident) => match identifier_text(&ident) {
                        Some(name) => name,
                        None => {
                            diag.push_error("Missing external name for export".to_owned(), &ident);
                            return None;
                        }
                    },
                    None => internal_name.clone(),
                };
                Some(NamedExport { internal_name, exported_name })
            })
            .collect::<Vec<_>>();

        exports.extend(doc.ExportsList().flat_map(|exports| exports.Component()).filter_map(
            |component| {
                let name = match identifier_text(&component.DeclaredIdentifier()) {
                    Some(name) => name,
                    None => {
                        diag.push_error(
                            "Cannot export component without name".to_owned(),
                            &component,
                        );
                        return None;
                    }
                };
                Some(NamedExport { internal_name: name.clone(), exported_name: name })
            },
        ));

        if exports.is_empty() {
            let internal_name = inner_components.last().cloned().unwrap_or_default().id.clone();
            exports.push(NamedExport {
                internal_name: internal_name.clone(),
                exported_name: internal_name,
            })
        }

        let imported_names = doc
            .ImportSpecifier()
            .map(|import| crate::typeloader::ImportedName::extract_imported_names(&import))
            .flatten()
            .collect::<Vec<_>>();

        let resolve_export_to_inner_component_or_import = |export: &NamedExport| {
            if let Some(local_comp) = inner_components.iter().find(|c| c.id == export.internal_name)
            {
                local_comp.clone()
            } else {
                imported_names
                    .iter()
                    .find_map(|import| {
                        if import.internal_name == export.internal_name {
                            Some(
                                type_registry
                                    .borrow()
                                    .lookup_element(&import.internal_name)
                                    .unwrap()
                                    .as_component()
                                    .clone(),
                            )
                        } else {
                            None
                        }
                    })
                    .unwrap()
            }
        };

        Self(
            exports
                .iter()
                .map(|export| {
                    (
                        export.exported_name.clone(),
                        resolve_export_to_inner_component_or_import(export),
                    )
                })
                .collect(),
        )
    }
}
