/*!
 This module contains the intermediate representation of the code in the form of an object tree
*/

use crate::diagnostics::{FileDiagnostics, Spanned, SpannedWithSourceFile};
use crate::expression_tree::{Expression, NamedReference};
use crate::parser::{syntax_nodes, SyntaxKind, SyntaxNode, SyntaxNodeWithSourceFile};
use crate::typeregister::{Type, TypeRegister};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// The full document (a complete file)
#[derive(Default, Debug)]
pub struct Document {
    //     node: SyntaxNode,
    pub inner_components: Vec<Rc<Component>>,
    pub root_component: Rc<Component>,
    pub local_registry: TypeRegister,
}

impl Document {
    pub fn from_node(
        node: SyntaxNode,
        diag: &mut FileDiagnostics,
        parent_registry: &Rc<RefCell<TypeRegister>>,
    ) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Document);
        let node: syntax_nodes::Document =
            SyntaxNodeWithSourceFile { node, source_file: diag.current_path.clone() }.into();

        let mut local_registry = TypeRegister::new(parent_registry);

        let inner_components = node
            .Component()
            .map(|n| {
                let compo = Component::from_node(n, diag, &local_registry);
                local_registry.add(compo.clone());
                compo
            })
            .collect::<Vec<_>>();

        Document {
            // FIXME: one should use the `component` hint instead of always returning the last
            root_component: inner_components.last().cloned().unwrap_or_default(),

            inner_components,

            local_registry,
        }
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

    /// Map of resources to embed in the generated binary, indexed by their absolute path on
    /// disk on the build system and valued by a unique integer id, that can be used by the
    /// generator for symbol generation.
    pub embedded_file_resources: RefCell<HashMap<String, usize>>,

    /// LayoutConstraints
    pub layout_constraints: RefCell<crate::layout::LayoutConstraints>,
}

impl Component {
    pub fn from_node(
        node: syntax_nodes::Component,
        diag: &mut FileDiagnostics,
        tr: &TypeRegister,
    ) -> Rc<Self> {
        let c = Rc::new(Component {
            id: node.child_text(SyntaxKind::Identifier).unwrap_or_default(),
            root_element: Element::from_node(
                node.Element(),
                "root".into(),
                Type::Invalid,
                diag,
                tr,
            ),
            ..Default::default()
        });
        let weak = Rc::downgrade(&c);
        recurse_elem(&c.root_element, &(), &mut |e, _| {
            e.borrow_mut().enclosing_component = weak.clone()
        });
        c
    }
}

#[derive(Clone, Debug, Default)]
pub struct PropertyDeclaration {
    pub property_type: Type,
    pub type_node: Option<SyntaxNodeWithSourceFile>,
    pub expose_in_public_api: bool,
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
    pub base_type: crate::typeregister::Type,
    /// Currently contains also the signals. FIXME: should that be changed?
    pub bindings: HashMap<String, Expression>,
    pub children: Vec<ElementRc>,
    /// The component which contains this element.
    pub enclosing_component: Weak<Component>,

    pub property_declarations: HashMap<String, PropertyDeclaration>,

    pub property_animations: HashMap<String, ElementRc>,

    /// Tis element is part of a `for <xxx> in <model>:
    pub repeated: Option<RepeatedElementInfo>,

    pub states: Vec<State>,
    pub transitions: Vec<Transition>,

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
        self.node.as_ref().map(|n| &n.0.source_file)
    }
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
}

pub type ElementRc = Rc<RefCell<Element>>;

impl Element {
    pub fn from_node(
        node: syntax_nodes::Element,
        id: String,
        parent_type: Type,
        diag: &mut FileDiagnostics,
        tr: &TypeRegister,
    ) -> ElementRc {
        let base = QualifiedTypeName::from_node(node.QualifiedName());
        let mut r = Element {
            id,
            base_type: match parent_type.lookup_type_for_child_element(&base.to_string(), tr) {
                Ok(ty) => ty,
                Err(err) => {
                    diag.push_error(err, &node.QualifiedName());
                    return ElementRc::default();
                }
            },
            node: Some(node.clone()),
            ..Default::default()
        };
        assert!(r.base_type.is_object_type());

        for prop_decl in node.PropertyDeclaration() {
            let qualified_type_node = prop_decl.QualifiedName();
            let qualified_type = QualifiedTypeName::from_node(qualified_type_node.clone());

            let prop_type = tr.lookup_qualified(&qualified_type.members);

            match prop_type {
                Type::Invalid => {
                    diag.push_error(
                        format!("Unknown property type '{}'", qualified_type.to_string()),
                        &qualified_type_node,
                    );
                }
                _ => (),
            };

            let prop_name_token =
                prop_decl.DeclaredIdentifier().child_token(SyntaxKind::Identifier).unwrap();

            let prop_name = prop_name_token.text().to_string();
            if !matches!(r.lookup_property(&prop_name), Type::Invalid) {
                diag.push_error(
                    format!("Cannot override property '{}'", prop_name),
                    &prop_name_token,
                )
            }

            r.property_declarations.insert(
                prop_name.clone(),
                PropertyDeclaration {
                    property_type: prop_type,
                    type_node: Some(qualified_type_node.into()),
                    ..Default::default()
                },
            );

            if let Some(csn) = prop_decl.BindingExpression() {
                if r.bindings.insert(prop_name, Expression::Uncompiled(csn.into())).is_some() {
                    diag.push_error("Duplicated property binding".into(), &prop_name_token);
                }
            }
        }

        r.parse_bindings(&base, node.Binding(), diag);

        for sig_decl in node.SignalDeclaration() {
            let name_token =
                sig_decl.DeclaredIdentifier().child_token(SyntaxKind::Identifier).unwrap();
            let name = name_token.text().to_string();
            r.property_declarations.insert(
                name,
                PropertyDeclaration {
                    property_type: Type::Signal,
                    type_node: Some(sig_decl.into()),
                    ..Default::default()
                },
            );
        }

        for con_node in node.SignalConnection() {
            let name_token = match con_node.child_token(SyntaxKind::Identifier) {
                Some(x) => x,
                None => continue,
            };
            let name = name_token.text().to_string();
            let prop_type = r.lookup_property(&name);
            if !matches!(prop_type, Type::Signal) {
                diag.push_error(format!("'{}' is not a signal in {}", name, base), &name_token);
            }
            if r.bindings
                .insert(name, Expression::Uncompiled(con_node.CodeBlock().into()))
                .is_some()
            {
                diag.push_error("Duplicated signal".into(), &name_token);
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

        for se in node.children() {
            if se.kind() == SyntaxKind::SubElement {
                let id = se.child_text(SyntaxKind::Identifier).unwrap_or_default();
                if let Some(element_node) = se.child_node(SyntaxKind::Element) {
                    r.children.push(Element::from_node(
                        element_node.into(),
                        id,
                        r.base_type.clone(),
                        diag,
                        tr,
                    ));
                } else {
                    assert!(diag.has_error());
                }
            } else if se.kind() == SyntaxKind::RepeatedElement {
                r.children.push(Element::from_repeated_node(
                    se.into(),
                    r.base_type.clone(),
                    diag,
                    tr,
                ));
            } else if se.kind() == SyntaxKind::ConditionalElement {
                r.children.push(Element::from_conditional_node(
                    se.into(),
                    r.base_type.clone(),
                    diag,
                    tr,
                ));
            }
        }

        let r = ElementRc::new(RefCell::new(r));

        for state in node.States().flat_map(|s| s.State()) {
            let s = State {
                id: state
                    .DeclaredIdentifier()
                    .child_text(SyntaxKind::Identifier)
                    .unwrap_or_default(),
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
                is_out: trs.child_text(SyntaxKind::Identifier).unwrap_or_default() == "out",
                state_id: trs
                    .DeclaredIdentifier()
                    .child_text(SyntaxKind::Identifier)
                    .unwrap_or_default(),
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
        parent_type: Type,
        diag: &mut FileDiagnostics,
        tr: &TypeRegister,
    ) -> ElementRc {
        let rei = RepeatedElementInfo {
            model: Expression::Uncompiled(node.Expression().into()),
            model_data_id: node
                .DeclaredIdentifier()
                .and_then(|n| n.child_text(SyntaxKind::Identifier))
                .unwrap_or_default(),
            index_id: node
                .RepeatedIndex()
                .and_then(|r| r.child_text(SyntaxKind::Identifier))
                .unwrap_or_default(),
            is_conditional_element: false,
        };
        let e = Element::from_node(node.Element(), String::new(), parent_type, diag, tr);
        e.borrow_mut().repeated = Some(rei);
        e
    }

    fn from_conditional_node(
        node: syntax_nodes::ConditionalElement,
        parent_type: Type,
        diag: &mut FileDiagnostics,
        tr: &TypeRegister,
    ) -> ElementRc {
        let rei = RepeatedElementInfo {
            model: Expression::Uncompiled(node.Expression().into()),
            model_data_id: String::new(),
            index_id: String::new(),
            is_conditional_element: true,
        };
        let e = Element::from_node(node.Element(), String::new(), parent_type, diag, tr);
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
        base: &QualifiedTypeName,
        bindings: impl Iterator<Item = syntax_nodes::Binding>,
        diag: &mut FileDiagnostics,
    ) {
        for b in bindings {
            let name_token = match b.child_token(SyntaxKind::Identifier) {
                Some(x) => x,
                None => continue,
            };
            let name = name_token.text().to_string();
            let prop_type = self.lookup_property(&name);
            if !prop_type.is_property_type() {
                diag.push_error(
                    match prop_type {
                        Type::Invalid => format!("Unknown property {} in {}", name, base),
                        Type::Signal => format!("'{}' is a signal. Use `=>` to connect", name),
                        _ => format!("Cannot assing to {} in {}", name, base),
                    },
                    &name_token,
                );
            }
            if self
                .bindings
                .insert(name, Expression::Uncompiled(b.BindingExpression().into()))
                .is_some()
            {
                diag.push_error("Duplicated property binding".into(), &name_token);
            }
        }
    }
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
        let base = QualifiedTypeName { members: vec![anim_type.as_builtin().class_name.clone()] };
        let mut anim_element =
            Element { id: "".into(), base_type: anim_type, node: None, ..Default::default() };
        anim_element.parse_bindings(&base, anim.Binding(), diag);
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
            .filter_map(|x| x.as_token().map(|x| x.text().to_string()))
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
            (NamedReference { element: Rc::downgrade(&r), name: String::default() }, ty)
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

/// This visit the binding attached to this element, but does not recurse in children elements
/// Also does not recurse within the expressions.
///
/// This code will temporarily move the bindings or states member so it can call the visitor without
/// maintaining a borrow on the RefCell.
pub fn visit_element_expressions(
    elem: &ElementRc,
    mut vis: impl FnMut(&mut Expression, &dyn Fn() -> Type),
) {
    let repeated = std::mem::take(&mut elem.borrow_mut().repeated);
    if let Some(mut r) = repeated {
        let is_conditional_element = r.is_conditional_element;
        vis(&mut r.model, &|| if is_conditional_element { Type::Bool } else { Type::Model });
        elem.borrow_mut().repeated = Some(r)
    }
    let mut bindings = std::mem::take(&mut elem.borrow_mut().bindings);
    for (name, expr) in &mut bindings {
        vis(expr, &|| elem.borrow().lookup_property(name));
    }
    elem.borrow_mut().bindings = bindings;
    let mut states = std::mem::take(&mut elem.borrow_mut().states);
    for s in &mut states {
        if let Some(cond) = s.condition.as_mut() {
            vis(cond, &|| Type::Bool)
        }
        for (ne, e) in &mut s.property_changes {
            vis(e, &|| ne.element.upgrade().unwrap().borrow().lookup_property(ne.name.as_ref()));
        }
    }
    elem.borrow_mut().states = states;

    let property_animations = std::mem::take(&mut elem.borrow_mut().property_animations);
    for anim_elem in property_animations.values() {
        let mut bindings = std::mem::take(&mut anim_elem.borrow_mut().bindings);
        for (name, expr) in &mut bindings {
            vis(expr, &|| anim_elem.borrow().lookup_property(name));
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
            _ => {}
        }
    }
    visit_element_expressions(elem, |expr, _| recurse_expression(expr, &mut vis));
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
