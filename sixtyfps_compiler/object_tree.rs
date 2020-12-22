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
use crate::expression_tree::Unit;
use crate::expression_tree::{Expression, ExpressionSpanned, NamedReference};
use crate::langtype::{NativeClass, Type};
use crate::parser::{identifier_text, syntax_nodes, SyntaxKind, SyntaxNodeWithSourceFile};
use crate::typeregister::TypeRegister;
use std::cell::RefCell;
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
        let exports = Exports::from_node(&node, &inner_components, &local_registry, diag);

        let root_component = inner_components
            .last()
            .cloned()
            .or_else(|| {
                node.ImportSpecifier()
                    .last()
                    .and_then(|import| {
                        crate::typeloader::ImportedName::extract_imported_names(&import).last()
                    })
                    .and_then(|import| match local_registry.lookup(&import.internal_name) {
                        Type::Component(c) => Some(c),
                        _ => None,
                    })
            })
            .unwrap_or_default();
        Document { root_component, inner_components, inner_structs, local_registry, exports }
    }

    pub fn exports(&self) -> &Vec<(String, Type)> {
        &self.exports.0
    }
}

#[derive(Debug)]
pub struct PopupWindow {
    pub component: Rc<Component>,
    pub x: NamedReference,
    pub y: NamedReference,
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

    /// Map of resources that should be embedded in the generated code, indexed by their absolute path on
    /// disk on the build system and valued by a unique integer id, that can be used by the
    /// generator for symbol generation.
    pub embedded_file_resources: RefCell<HashMap<String, usize>>,

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
    pub used_structs: RefCell<Vec<Type>>,

    pub popup_windows: RefCell<Vec<PopupWindow>>,
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
        match &self.root_element.borrow().base_type {
            Type::Void => true,
            Type::Builtin(c) => c.is_global,
            _ => false,
        }
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

#[derive(Debug, Clone)]
pub struct TransitionPropertyAnimation {
    /// The state id as computed in lower_state
    pub state_id: i32,
    /// false for 'to', true for 'out'
    pub is_out: bool,
    /// The content of the `anumation` object
    pub animation: ElementRc,
}

impl TransitionPropertyAnimation {
    /// Return an expression which returns a boolean which is true if the transition is active.
    /// The state argument is an expresison referencing the state property of type StateInfo
    pub fn condition(&self, state: Expression) -> Expression {
        Expression::BinaryExpression {
            lhs: Box::new(Expression::ObjectAccess {
                base: Box::new(state),
                name: (if self.is_out { "previous_state" } else { "current_state" }).into(),
            }),
            rhs: Box::new(Expression::NumberLiteral(self.state_id as _, Unit::None)),
            op: '=',
        }
    }
}

#[derive(Debug, Clone)]
pub enum PropertyAnimation {
    Static(ElementRc),
    Transition { state_ref: Expression, animations: Vec<TransitionPropertyAnimation> },
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
    /// Currently contains also the callbacks. FIXME: should that be changed?
    pub bindings: HashMap<String, ExpressionSpanned>,
    pub children: Vec<ElementRc>,
    /// The component which contains this element.
    pub enclosing_component: Weak<Component>,

    pub property_declarations: HashMap<String, PropertyDeclaration>,

    pub property_animations: HashMap<String, PropertyAnimation>,

    /// Tis element is part of a `for <xxx> in <model>:
    pub repeated: Option<RepeatedElementInfo>,

    pub states: Vec<State>,
    pub transitions: Vec<Transition>,

    /// true when this item's geometry is handled by a layout
    pub child_of_layout: bool,

    /// This is the component-local index of this item in the item tree array.
    /// It is generated after the last pass and before the generators run.
    pub item_index: once_cell::unsync::OnceCell<usize>,

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

            // This must be a global component it can only have properties and callback
            let mut error_on = |node: &dyn Spanned, what: &str| {
                diag.push_error(format!("A global component cannot have {}", what), node);
            };
            node.SubElement().for_each(|n| error_on(&n, "sub elements"));
            node.RepeatedElement().for_each(|n| error_on(&n, "sub elements"));
            node.ChildrenPlaceholder().map(|n| error_on(&n, "sub elements"));
            node.CallbackConnection().for_each(|n| error_on(&n, "callback connections"));
            node.PropertyAnimation().for_each(|n| error_on(&n, "animations"));
            node.States().for_each(|n| error_on(&n, "states"));
            node.Transitions().for_each(|n| error_on(&n, "transitions"));
            (Type::Void, String::new())
        };
        let mut r = Element { id, base_type, node: Some(node.clone()), ..Default::default() };

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

            if prop_type != Type::Invalid && !prop_type.is_property_type() {
                diag.push_error(
                    format!("'{}' is not a valid property type", prop_type),
                    &type_node,
                );
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

        if let Type::Builtin(builtin_base) = &r.base_type {
            for (prop, expr) in &builtin_base.default_bindings {
                r.bindings.entry(prop.clone()).or_insert_with(|| expr.clone().into());
            }
        }

        for sig_decl in node.CallbackDeclaration() {
            let name = identifier_text(&sig_decl.DeclaredIdentifier()).unwrap();
            let args = sig_decl.Type().map(|node_ty| type_from_node(node_ty, diag, tr)).collect();
            let return_type = sig_decl
                .ReturnType()
                .map(|ret_ty| Box::new(type_from_node(ret_ty.Type(), diag, tr)));
            r.property_declarations.insert(
                name,
                PropertyDeclaration {
                    property_type: Type::Callback { return_type, args },
                    type_node: Some(sig_decl.into()),
                    ..Default::default()
                },
            );
        }

        for con_node in node.CallbackConnection() {
            let name = match identifier_text(&con_node) {
                Some(x) => x,
                None => continue,
            };
            let prop_type = r.lookup_property(&name);
            if let Type::Callback { args, .. } = prop_type {
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
                        "Duplicated callback".into(),
                        &con_node.child_token(SyntaxKind::Identifier).unwrap(),
                    );
                }
            } else {
                diag.push_error(
                    format!("'{}' is not a callback{}", name, name_for_looup_errors),
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
                                .insert(prop_name.clone(), PropertyAnimation::Static(anim_element))
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
                node: trs.DeclaredIdentifier().into(),
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
                        Type::Callback { .. } => {
                            format!("'{}' is a callback. Use `=>` to connect", name)
                        }
                        _ => format!(
                            "Cannot assign to {}{} because it does not have a valid property type.",
                            name, name_for_lookup_error
                        ),
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

/// Create a Type for this node
pub fn type_from_node(
    node: syntax_nodes::Type,
    diag: &mut FileDiagnostics,
    tr: &TypeRegister,
) -> Type {
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

/// Create a Type::Object from a syntax_nodes::ObjectType
pub fn type_struct_from_node(
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
    pub members: Vec<String>,
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
    component: &Component,
    state: &State,
    vis: &mut impl FnMut(&ElementRc, &State) -> State,
) {
    recurse_elem(&component.root_element, state, &mut |elem, state| {
        if elem.borrow().repeated.is_some() {
            if let Type::Component(base) = &elem.borrow().base_type {
                recurse_elem_including_sub_components(base, state, vis);
            }
        }
        vis(elem, state)
    });
    component
        .popup_windows
        .borrow()
        .iter()
        .for_each(|p| recurse_elem_including_sub_components(&p.component, state, vis))
}

/// Same as recurse_elem, but will take the children from the element as to not keep the element borrow
pub fn recurse_elem_no_borrow<State>(
    elem: &ElementRc,
    state: &State,
    vis: &mut impl FnMut(&ElementRc, &State) -> State,
) {
    let state = vis(elem, state);
    let children = elem.borrow().children.clone();
    for sub in &children {
        recurse_elem_no_borrow(sub, &state, vis);
    }
}

/// Same as [`recurse_elem`] but include the elements form sub_components
pub fn recurse_elem_including_sub_components_no_borrow<State>(
    component: &Component,
    state: &State,
    vis: &mut impl FnMut(&ElementRc, &State) -> State,
) {
    recurse_elem_no_borrow(&component.root_element, state, &mut |elem, state| {
        if elem.borrow().repeated.is_some() {
            if let Type::Component(base) = &elem.borrow().base_type {
                recurse_elem_including_sub_components_no_borrow(base, state, vis);
            }
        }
        vis(elem, state)
    });
    component
        .popup_windows
        .borrow()
        .iter()
        .for_each(|p| recurse_elem_including_sub_components(&p.component, state, vis))
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
    fn visit_element_expressions_simple(
        elem: &ElementRc,
        vis: &mut impl FnMut(&mut Expression, Option<&str>, &dyn Fn() -> Type),
    ) {
        let mut bindings = std::mem::take(&mut elem.borrow_mut().bindings);
        for (name, expr) in &mut bindings {
            vis(expr, Some(name.as_str()), &|| elem.borrow().lookup_property(name));
        }
        elem.borrow_mut().bindings = bindings;
    }

    let repeated = std::mem::take(&mut elem.borrow_mut().repeated);
    if let Some(mut r) = repeated {
        let is_conditional_element = r.is_conditional_element;
        vis(&mut r.model, None, &|| if is_conditional_element { Type::Bool } else { Type::Model });
        elem.borrow_mut().repeated = Some(r)
    }
    visit_element_expressions_simple(elem, &mut vis);
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

    let mut transitions = std::mem::take(&mut elem.borrow_mut().transitions);
    for t in &mut transitions {
        for (_, a) in &mut t.property_animations {
            visit_element_expressions_simple(a, &mut vis);
        }
    }
    elem.borrow_mut().transitions = transitions;

    let mut property_animations = std::mem::take(&mut elem.borrow_mut().property_animations);
    for anim_elem in property_animations.values_mut() {
        match anim_elem {
            PropertyAnimation::Static(e) => visit_element_expressions_simple(e, &mut vis),
            PropertyAnimation::Transition { animations, state_ref } => {
                vis(state_ref, None, &|| Type::Int32);
                for a in animations {
                    visit_element_expressions_simple(&a.animation, &mut vis)
                }
            }
        }
    }
    elem.borrow_mut().property_animations = property_animations;
}

/// Visit all the named reference in an element
/// But does not recurse in sub-elements. (unlike [`visit_all_named_references`] which recurse)
pub fn visit_all_named_references_in_element(
    elem: &ElementRc,
    mut vis: impl FnMut(&mut NamedReference),
) {
    fn recurse_expression(expr: &mut Expression, vis: &mut impl FnMut(&mut NamedReference)) {
        expr.visit_mut(|sub| recurse_expression(sub, vis));
        match expr {
            Expression::PropertyReference(r) | Expression::CallbackReference(r) => vis(r),
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

/// Visit all named reference in this component and sub component
pub fn visit_all_named_references(
    component: &Component,
    vis: &mut impl FnMut(&mut NamedReference),
) {
    recurse_elem_including_sub_components_no_borrow(
        component,
        &Weak::new(),
        &mut |elem, parent_compo| {
            visit_all_named_references_in_element(elem, |nr| vis(nr));
            let compo = elem.borrow().enclosing_component.clone();
            if !Weak::ptr_eq(parent_compo, &compo) {
                let compo = compo.upgrade().unwrap();
                compo.layouts.borrow_mut().iter_mut().for_each(|l| l.visit_named_references(vis));
                compo.popup_windows.borrow_mut().iter_mut().for_each(|p| {
                    vis(&mut p.x);
                    vis(&mut p.y);
                });
            }
            compo
        },
    );
}

/// Visit all expression in this component and sub components
///
/// Does not recurse in the expression itself
pub fn visit_all_expressions(
    component: &Component,
    mut vis: impl FnMut(&mut Expression, &dyn Fn() -> Type),
) {
    recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
        visit_element_expressions(elem, |expr, _, ty| vis(expr, ty));
    })
}

#[derive(Debug, Clone)]
pub struct State {
    pub id: String,
    pub condition: Option<Expression>,
    pub property_changes: Vec<(NamedReference, Expression)>,
}

#[derive(Debug, Clone)]
pub struct Transition {
    /// false for 'to', true for 'out'
    pub is_out: bool,
    pub state_id: String,
    pub property_animations: Vec<(NamedReference, ElementRc)>,
    /// Node pointing to the state name
    pub node: SyntaxNodeWithSourceFile,
}

#[derive(Default, Debug, derive_more::Deref)]
pub struct Exports(Vec<(String, Type)>);

impl Exports {
    pub fn from_node(
        doc: &syntax_nodes::Document,
        inner_components: &[Rc<Component>],
        type_registry: &TypeRegister,
        diag: &mut FileDiagnostics,
    ) -> Self {
        #[derive(Debug, Clone)]
        struct NamedExport {
            internal_name_ident: SyntaxNodeWithSourceFile,
            internal_name: String,
            exported_name: String,
        }

        let mut exports = doc
            .ExportsList()
            .flat_map(|exports| exports.ExportSpecifier())
            .map(|export_specifier| {
                let internal_name = identifier_text(&export_specifier.ExportIdentifier())
                    .expect("internal error: missing internal name for export");
                let exported_name = match export_specifier.ExportName() {
                    Some(ident) => identifier_text(&ident)
                        .expect("internal error: missing external name for export"),
                    None => internal_name.clone(),
                };
                NamedExport {
                    internal_name_ident: export_specifier.ExportIdentifier().into(),
                    internal_name,
                    exported_name,
                }
            })
            .collect::<Vec<_>>();

        exports.extend(doc.ExportsList().flat_map(|exports| exports.Component()).map(
            |component| {
                let name = identifier_text(&component.DeclaredIdentifier())
                    .expect("internal error: cannot export component without name");
                NamedExport {
                    internal_name_ident: component.DeclaredIdentifier().into(),
                    internal_name: name.clone(),
                    exported_name: name,
                }
            },
        ));
        exports.extend(doc.ExportsList().flat_map(|exports| exports.StructDeclaration()).map(
            |st| {
                let name = identifier_text(&st.DeclaredIdentifier())
                    .expect("internal error: cannot export struct without name");
                NamedExport {
                    internal_name_ident: st.DeclaredIdentifier().into(),
                    internal_name: name.clone(),
                    exported_name: name,
                }
            },
        ));

        if exports.is_empty() {
            if let Some(internal_name) = inner_components.last().as_ref().map(|x| x.id.clone()) {
                exports.push(NamedExport {
                    internal_name_ident: doc.clone().into(),
                    internal_name: internal_name.clone(),
                    exported_name: internal_name,
                })
            }
        }

        let mut resolve_export_to_inner_component_or_import =
            |export: &NamedExport| match type_registry.lookup(export.internal_name.as_str()) {
                ty @ Type::Component(_) | ty @ Type::Object { .. } => Some(ty),
                Type::Invalid => {
                    diag.push_error(
                        format!("'{}' not found", export.internal_name),
                        &export.internal_name_ident,
                    );
                    None
                }
                _ => {
                    diag.push_error(
                        format!(
                            "Cannot export '{}' because it is not a component",
                            export.internal_name,
                        ),
                        &export.internal_name_ident,
                    );
                    None
                }
            };

        Self(
            exports
                .iter()
                .filter_map(|export| {
                    Some((
                        export.exported_name.clone(),
                        resolve_export_to_inner_component_or_import(export)?,
                    ))
                })
                .collect(),
        )
    }
}
