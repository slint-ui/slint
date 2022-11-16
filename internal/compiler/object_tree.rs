// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
 This module contains the intermediate representation of the code in the form of an object tree
*/

// cSpell: ignore qualname

use itertools::Either;

use crate::diagnostics::{BuildDiagnostics, SourceLocation, Spanned};
use crate::expression_tree::{self, BindingExpression, Expression, Unit};
use crate::langtype::{BuiltinElement, NativeClass, Type};
use crate::langtype::{ElementType, PropertyLookupResult};
use crate::layout::{LayoutConstraints, Orientation};
use crate::namedreference::NamedReference;
use crate::parser;
use crate::parser::{syntax_nodes, SyntaxKind, SyntaxNode};
use crate::typeloader::ImportedTypes;
use crate::typeregister::TypeRegister;
use std::cell::{Cell, RefCell};
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;
use std::rc::{Rc, Weak};

macro_rules! unwrap_or_continue {
    ($e:expr ; $diag:expr) => {
        match $e {
            Some(x) => x,
            None => {
                debug_assert!($diag.has_error()); // error should have been reported at parsing time
                continue;
            }
        }
    };
}

/// The full document (a complete file)
#[derive(Default, Debug)]
pub struct Document {
    pub node: Option<syntax_nodes::Document>,
    pub inner_components: Vec<Rc<Component>>,
    pub inner_structs: Vec<Type>,
    pub root_component: Rc<Component>,
    pub local_registry: TypeRegister,
    /// A list of paths to .ttf/.ttc files that are supposed to be registered on
    /// startup for custom font use.
    pub custom_fonts: Vec<(String, crate::parser::SyntaxToken)>,
    pub exports: Exports,
}

impl Document {
    pub fn from_node(
        node: syntax_nodes::Document,
        foreign_imports: Vec<ImportedTypes>,
        diag: &mut BuildDiagnostics,
        parent_registry: &Rc<RefCell<TypeRegister>>,
    ) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Document);

        let mut local_registry = TypeRegister::new(parent_registry);
        let mut inner_components = vec![];
        let mut inner_structs = vec![];

        let mut process_component =
            |n: syntax_nodes::Component,
             diag: &mut BuildDiagnostics,
             local_registry: &mut TypeRegister| {
                let compo = Component::from_node(n, diag, local_registry);
                local_registry.add(compo.clone());
                inner_components.push(compo);
            };
        let mut process_struct =
            |n: syntax_nodes::StructDeclaration,
             diag: &mut BuildDiagnostics,
             local_registry: &mut TypeRegister| {
                let mut ty = type_struct_from_node(n.ObjectType(), diag, local_registry);
                if let Type::Struct { name, .. } = &mut ty {
                    *name = parser::identifier_text(&n.DeclaredIdentifier());
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
                        crate::typeloader::ImportedName::extract_imported_names(&import)
                            .and_then(|it| it.last())
                    })
                    .and_then(|import| local_registry.lookup_element(&import.internal_name).ok())
                    .and_then(|c| match c {
                        ElementType::Component(c) => Some(c),
                        _ => None,
                    })
            })
            .unwrap_or_default();

        let custom_fonts = foreign_imports
            .into_iter()
            .filter_map(|import| {
                if import.file.ends_with(".ttc")
                    || import.file.ends_with(".ttf")
                    || import.file.ends_with(".otf")
                {
                    // Assume remote urls are valid, we need to load them at run-time (which we currently don't). For
                    // local paths we should try to verify the existence and let the developer know ASAP.
                    if import.file.starts_with("http://")
                        || import.file.starts_with("https://")
                        || crate::fileaccess::load_file(std::path::Path::new(&import.file))
                            .is_some()
                    {
                        Some((import.file, import.import_token))
                    } else {
                        diag.push_error(
                            format!("File \"{}\" not found", import.file),
                            &import.import_token,
                        );
                        None
                    }
                } else {
                    diag.push_error(
                        format!("Unsupported foreign import \"{}\"", import.file),
                        &import.import_token,
                    );
                    None
                }
            })
            .collect();

        Document {
            node: Some(node),
            root_component,
            inner_components,
            inner_structs,
            local_registry,
            custom_fonts,
            exports,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PopupWindow {
    pub component: Rc<Component>,
    pub x: NamedReference,
    pub y: NamedReference,
    pub parent_element: ElementRc,
}

type ChildrenInsertionPoint = (ElementRc, syntax_nodes::ChildrenPlaceholder);

/// Used sub types for a root component
#[derive(Debug, Default)]
pub struct UsedSubTypes {
    /// All the globals used by the component and its children.
    pub globals: Vec<Rc<Component>>,
    /// All the structs used by the component and its children.
    pub structs: Vec<Type>,
    /// All the sub components use by this components and its children,
    /// and the amount of time it is used
    pub sub_components: Vec<Rc<Component>>,
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
    /// disk on the build system
    pub embedded_file_resources:
        RefCell<HashMap<String, crate::embedded_resources::EmbeddedResources>>,

    /// The layout constraints of the root item
    pub root_constraints: RefCell<LayoutConstraints>,

    /// When creating this component and inserting "children", append them to the children of
    /// the element pointer to by this field.
    pub child_insertion_point: RefCell<Option<ChildrenInsertionPoint>>,

    /// Code to be inserted into the constructor
    pub setup_code: RefCell<Vec<Expression>>,

    /// The list of used extra types used (recursively) by this root component.
    /// (This only make sense on the root component)
    pub used_types: RefCell<UsedSubTypes>,
    pub popup_windows: RefCell<Vec<PopupWindow>>,

    /// The names under which this component should be accessible
    /// if it is a global singleton and exported.
    pub exported_global_names: RefCell<Vec<ExportedName>>,

    /// This is the main entry point for the code generators. Such a component
    /// should have the full API, etc.
    pub is_root_component: Cell<bool>,
}

impl Component {
    pub fn from_node(
        node: syntax_nodes::Component,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) -> Rc<Self> {
        let mut child_insertion_point = None;
        let is_legacy_syntax = node.child_token(SyntaxKind::ColonEqual).is_some();
        let c = Component {
            id: parser::identifier_text(&node.DeclaredIdentifier()).unwrap_or_default(),
            root_element: Element::from_node(
                node.Element(),
                "root".into(),
                if node.child_text(SyntaxKind::Identifier).map_or(false, |t| t == "global") {
                    ElementType::Global
                } else {
                    ElementType::Error
                },
                &mut child_insertion_point,
                is_legacy_syntax,
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
            ElementType::Global => true,
            ElementType::Builtin(c) => c.is_global,
            _ => false,
        }
    }

    pub fn visible_in_public_api(&self) -> bool {
        if self.is_global() {
            !self.exported_global_names.borrow().is_empty()
        } else {
            self.parent_element.upgrade().is_none() && self.is_root_component.get()
        }
    }

    /// Returns the names of aliases to global singletons, exactly as
    /// specified in the .slint markup (not normalized).
    pub fn global_aliases(&self) -> Vec<String> {
        self.exported_global_names
            .borrow()
            .iter()
            .filter(|name| name.as_str() != self.root_element.borrow().id)
            .map(|name| name.original_name())
            .collect()
    }

    pub fn is_sub_component(&self) -> bool {
        !self.is_root_component.get()
            && self.parent_element.upgrade().is_none()
            && !self.is_global()
    }

    // Number of repeaters in this component, including sub-components
    pub fn repeater_count(&self) -> u32 {
        let mut count = 0;
        recurse_elem(&self.root_element, &(), &mut |element, _| {
            let element = element.borrow();
            if let Some(sub_component) = element.sub_component() {
                count += sub_component.repeater_count();
            } else if element.repeated.is_some() {
                count += 1;
            }
        });
        count
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PropertyVisibility {
    Private,
    Input,
    Output,
    InOut,
}

impl Default for PropertyVisibility {
    fn default() -> Self {
        Self::Private
    }
}

impl Display for PropertyVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropertyVisibility::Private => f.write_str("private"),
            PropertyVisibility::Input => f.write_str("input"),
            PropertyVisibility::Output => f.write_str("output"),
            PropertyVisibility::InOut => f.write_str("input output"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PropertyDeclaration {
    pub property_type: Type,
    pub node: Option<Either<syntax_nodes::PropertyDeclaration, syntax_nodes::CallbackDeclaration>>,
    /// Tells if getter and setter will be added to expose in the native language API
    pub expose_in_public_api: bool,
    /// Public API property exposed as an alias: it shouldn't be generated but instead forward to the alias.
    pub is_alias: Option<NamedReference>,
    pub visibility: PropertyVisibility,
}

impl PropertyDeclaration {
    // For diagnostics: return a node pointing to the type
    pub fn type_node(&self) -> Option<SyntaxNode> {
        self.node.as_ref().map(|x| -> crate::parser::SyntaxNode {
            x.as_ref().either(
                |x| x.Type().map_or_else(|| x.clone().into(), |x| x.into()),
                |x| x.clone().into(),
            )
        })
    }
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
    /// The content of the `animation` object
    pub animation: ElementRc,
}

impl TransitionPropertyAnimation {
    /// Return an expression which returns a boolean which is true if the transition is active.
    /// The state argument is an expression referencing the state property of type StateInfo
    pub fn condition(&self, state: Expression) -> Expression {
        Expression::BinaryExpression {
            lhs: Box::new(Expression::StructFieldAccess {
                base: Box::new(state),
                name: (if self.is_out { "previous-state" } else { "current-state" }).into(),
            }),
            rhs: Box::new(Expression::NumberLiteral(self.state_id as _, Unit::None)),
            op: '=',
        }
    }
}

#[derive(Debug)]
pub enum PropertyAnimation {
    Static(ElementRc),
    Transition { state_ref: Expression, animations: Vec<TransitionPropertyAnimation> },
}

impl Clone for PropertyAnimation {
    fn clone(&self) -> Self {
        fn deep_clone(e: &ElementRc) -> ElementRc {
            let e = e.borrow();
            debug_assert!(e.children.is_empty());
            debug_assert!(e.property_declarations.is_empty());
            debug_assert!(e.states.is_empty() && e.transitions.is_empty());
            Rc::new(RefCell::new(Element {
                id: e.id.clone(),
                base_type: e.base_type.clone(),
                bindings: e.bindings.clone(),
                property_analysis: e.property_analysis.clone(),
                enclosing_component: e.enclosing_component.clone(),
                repeated: None,
                node: e.node.clone(),
                ..Default::default()
            }))
        }
        match self {
            PropertyAnimation::Static(e) => PropertyAnimation::Static(deep_clone(e)),
            PropertyAnimation::Transition { state_ref, animations } => {
                PropertyAnimation::Transition {
                    state_ref: state_ref.clone(),
                    animations: animations
                        .iter()
                        .map(|t| TransitionPropertyAnimation {
                            state_id: t.state_id,
                            is_out: t.is_out,
                            animation: deep_clone(&t.animation),
                        })
                        .collect(),
                }
            }
        }
    }
}

/// Map the accessibility property (eg "accessible-role", "accessible-label") to its named reference
#[derive(Default, Clone)]
pub struct AccessibilityProps(pub BTreeMap<String, NamedReference>);

pub type BindingsMap = BTreeMap<String, RefCell<BindingExpression>>;

/// An Element is an instantiation of a Component
#[derive(Default)]
pub struct Element {
    /// The id as named in the original .slint file.
    ///
    /// Note that it can only be used for lookup before inlining.
    /// After inlining there can be duplicated id in the component.
    /// The id are then re-assigned unique id in the assign_id pass
    pub id: String,
    //pub base: QualifiedTypeName,
    pub base_type: ElementType,
    /// Currently contains also the callbacks. FIXME: should that be changed?
    pub bindings: BindingsMap,
    pub property_analysis: RefCell<HashMap<String, PropertyAnalysis>>,

    pub children: Vec<ElementRc>,
    /// The component which contains this element.
    pub enclosing_component: Weak<Component>,

    pub property_declarations: BTreeMap<String, PropertyDeclaration>,

    /// Main owner for a reference to a property.
    pub named_references: crate::namedreference::NamedReferenceContainer,

    /// This element is part of a `for <xxx> in <model>`:
    pub repeated: Option<RepeatedElementInfo>,

    pub states: Vec<State>,
    pub transitions: Vec<Transition>,

    /// true when this item's geometry is handled by a layout
    pub child_of_layout: bool,
    /// The property pointing to the layout info. `(horizontal, vertical)`
    pub layout_info_prop: Option<(NamedReference, NamedReference)>,

    pub accessibility_props: AccessibilityProps,

    /// true if this Element is the fake Flickable viewport
    pub is_flickable_viewport: bool,

    /// true if this Element may have a popup as child meaning it cannot be optimized
    /// because the popup references it.
    pub has_popup_child: bool,

    /// This is the component-local index of this item in the item tree array.
    /// It is generated after the last pass and before the generators run.
    pub item_index: once_cell::unsync::OnceCell<usize>,
    /// the index of the first children in the tree, set with item_index
    pub item_index_of_first_children: once_cell::unsync::OnceCell<usize>,

    /// True when this element is in a component was declared with the `:=` symbol instead of the `component` keyword
    pub is_legacy_syntax: bool,

    /// How many times the element was inlined
    pub inline_depth: i32,

    /// The AST node, if available
    pub node: Option<syntax_nodes::Element>,
}

impl Spanned for Element {
    fn span(&self) -> crate::diagnostics::Span {
        self.node.as_ref().map(|n| n.span()).unwrap_or_default()
    }

    fn source_file(&self) -> Option<&crate::diagnostics::SourceFile> {
        self.node.as_ref().map(|n| &n.source_file)
    }
}

impl core::fmt::Debug for Element {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        pretty_print(f, self, 0)
    }
}

pub fn pretty_print(
    f: &mut impl std::fmt::Write,
    e: &Element,
    indentation: usize,
) -> std::fmt::Result {
    if let Some(repeated) = &e.repeated {
        write!(f, "for {}[{}] in ", repeated.model_data_id, repeated.index_id)?;
        expression_tree::pretty_print(f, &repeated.model)?;
        write!(f, ":")?;
        if let ElementType::Component(base) = &e.base_type {
            if base.parent_element.upgrade().is_some() {
                pretty_print(f, &base.root_element.borrow(), indentation)?;
                return Ok(());
            }
        }
    }
    writeln!(f, "{} := {} {{", e.id, e.base_type)?;
    let mut indentation = indentation + 1;
    macro_rules! indent {
        () => {
            for _ in 0..indentation {
                write!(f, "   ")?
            }
        };
    }
    for (name, ty) in &e.property_declarations {
        indent!();
        if let Some(alias) = &ty.is_alias {
            writeln!(f, "alias<{}> {} <=> {:?};", ty.property_type, name, alias)?
        } else {
            writeln!(f, "property<{}> {};", ty.property_type, name)?
        }
    }
    for (name, expr) in &e.bindings {
        let expr = expr.borrow();
        indent!();
        write!(f, "{}: ", name)?;
        expression_tree::pretty_print(f, &expr.expression)?;
        if expr.analysis.as_ref().map_or(false, |a| a.is_const) {
            write!(f, "/*const*/")?;
        }
        writeln!(f, ";")?;
        //writeln!(f, "; /*{}*/", expr.priority)?;
        if let Some(anim) = &expr.animation {
            indent!();
            writeln!(f, "animate {} {:?}", name, anim)?;
        }
        for nr in &expr.two_way_bindings {
            indent!();
            writeln!(f, "{} <=> {:?};", name, nr)?;
        }
    }
    if !e.states.is_empty() {
        indent!();
        writeln!(f, "states {:?}", e.states)?;
    }
    if !e.transitions.is_empty() {
        indent!();
        writeln!(f, "transitions {:?} ", e.transitions)?;
    }
    for c in &e.children {
        indent!();
        pretty_print(f, &c.borrow(), indentation)?
    }

    /*if let Type::Component(base) = &e.base_type {
        pretty_print(f, &c.borrow(), indentation)?
    }*/
    indentation -= 1;
    indent!();
    writeln!(f, "}}")
}

#[derive(Clone, Default, Debug)]
pub struct PropertyAnalysis {
    /// true if somewhere in the code, there is an expression that changes this property with an assignment
    pub is_set: bool,

    /// True if this property might be set from a different component.
    pub is_set_externally: bool,

    /// true if somewhere in the code, an expression is reading this property
    /// Note: currently this is only set in the binding analysis pass
    pub is_read: bool,

    /// true if this property is read from another component
    pub is_read_externally: bool,

    /// True if the property is linked to another property that is read only. That property becomes read-only
    pub is_linked_to_read_only: bool,
}

impl PropertyAnalysis {
    /// Merge analysis from base element for inlining
    ///
    /// Contrary to `merge`, we don't keep the external uses because
    /// they should come from us
    pub fn merge_with_base(&mut self, other: &PropertyAnalysis) {
        self.is_set |= other.is_set;
        self.is_read |= other.is_read;
    }

    /// Merge the analysis
    pub fn merge(&mut self, other: &PropertyAnalysis) {
        self.is_set |= other.is_set;
        self.is_read |= other.is_read;
        self.is_read_externally |= other.is_read_externally;
        self.is_set_externally |= other.is_set_externally;
    }

    /// Return true if it is read or set or used in any way
    pub fn is_used(&self) -> bool {
        self.is_read || self.is_read_externally || self.is_set || self.is_set_externally
    }
}

#[derive(Debug, Clone)]
pub struct ListViewInfo {
    pub viewport_y: NamedReference,
    pub viewport_height: NamedReference,
    pub viewport_width: NamedReference,
    /// The ListView's inner visible height (not counting eventual scrollbar)
    pub listview_height: NamedReference,
    /// The ListView's inner visible width (not counting eventual scrollbar)
    pub listview_width: NamedReference,
}

#[derive(Debug, Clone)]
/// If the parent element is a repeated element, this has information about the models
pub struct RepeatedElementInfo {
    pub model: Expression,
    pub model_data_id: String,
    pub index_id: String,
    /// A conditional element is just a for whose model is a boolean expression
    ///
    /// When this is true, the model is of type boolean instead of Model
    pub is_conditional_element: bool,
    /// When the for is the delegate of a ListView
    pub is_listview: Option<ListViewInfo>,
}

pub type ElementRc = Rc<RefCell<Element>>;

impl Element {
    pub fn from_node(
        node: syntax_nodes::Element,
        id: String,
        parent_type: ElementType,
        component_child_insertion_point: &mut Option<ChildrenInsertionPoint>,
        is_legacy_syntax: bool,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) -> ElementRc {
        let base_type = if let Some(base_node) = node.QualifiedName() {
            let base = QualifiedTypeName::from_node(base_node.clone());
            let base_string = base.to_string();
            match parent_type.lookup_type_for_child_element(&base_string, tr) {
                Ok(ElementType::Component(c)) if c.is_global() => {
                    diag.push_error(
                        "Cannot create an instance of a global component".into(),
                        &base_node,
                    );
                    ElementType::Error
                }
                Ok(ty) => ty,
                Err(err) => {
                    diag.push_error(err, &base_node);
                    ElementType::Error
                }
            }
        } else if parent_type == ElementType::Global {
            // This must be a global component it can only have properties and callback
            let mut error_on = |node: &dyn Spanned, what: &str| {
                diag.push_error(format!("A global component cannot have {}", what), node);
            };
            node.SubElement().for_each(|n| error_on(&n, "sub elements"));
            node.RepeatedElement().for_each(|n| error_on(&n, "sub elements"));
            if let Some(n) = node.ChildrenPlaceholder() {
                error_on(&n, "sub elements");
            }
            node.PropertyAnimation().for_each(|n| error_on(&n, "animations"));
            node.States().for_each(|n| error_on(&n, "states"));
            node.Transitions().for_each(|n| error_on(&n, "transitions"));

            ElementType::Global
        } else if parent_type != ElementType::Error {
            // This should normally never happen because the parser does not allow for this
            assert!(diag.has_error());
            return ElementRc::default();
        } else {
            tr.empty_type()
        };
        let mut r = Element {
            id,
            base_type,
            node: Some(node.clone()),
            is_legacy_syntax,
            ..Default::default()
        };

        for prop_decl in node.PropertyDeclaration() {
            let prop_type = prop_decl
                .Type()
                .map(|type_node| type_from_node(type_node, diag, tr))
                // Type::Void is used for two way bindings without type specified
                .unwrap_or(Type::InferredProperty);

            let unresolved_prop_name =
                unwrap_or_continue!(parser::identifier_text(&prop_decl.DeclaredIdentifier()); diag);
            let PropertyLookupResult {
                resolved_name: prop_name,
                property_type: maybe_existing_prop_type,
                ..
            } = r.lookup_property(&unresolved_prop_name);
            match maybe_existing_prop_type {
                Type::Callback { .. } => {
                    diag.push_error(
                        format!("Cannot declare property '{}' when a callback with the same name exists", prop_name),
                        &prop_decl.DeclaredIdentifier().child_token(SyntaxKind::Identifier).unwrap(),
                    );
                    continue;
                }
                Type::Invalid => {} // Ok to proceed with a new declaration
                _ => {
                    diag.push_error(
                        format!("Cannot override property '{}'", prop_name),
                        &prop_decl
                            .DeclaredIdentifier()
                            .child_token(SyntaxKind::Identifier)
                            .unwrap(),
                    );
                    continue;
                }
            }

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
            let visibility = visibility.unwrap_or_else(|| {
                if is_legacy_syntax {
                    PropertyVisibility::InOut
                } else {
                    PropertyVisibility::Private
                }
            });

            r.property_declarations.insert(
                prop_name.to_string(),
                PropertyDeclaration {
                    property_type: prop_type,
                    node: Some(Either::Left(prop_decl.clone())),
                    visibility,
                    ..Default::default()
                },
            );

            if let Some(csn) = prop_decl.BindingExpression() {
                if r.bindings
                    .insert(
                        prop_name.to_string(),
                        BindingExpression::new_uncompiled(csn.into()).into(),
                    )
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
                    .insert(prop_name.into(), BindingExpression::new_uncompiled(csn.into()).into())
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
            node.Binding().filter_map(|b| {
                Some((b.child_token(SyntaxKind::Identifier)?, b.BindingExpression().into()))
            }),
            is_legacy_syntax,
            diag,
        );
        r.parse_bindings(
            node.TwoWayBinding()
                .filter_map(|b| Some((b.child_token(SyntaxKind::Identifier)?, b.into()))),
            is_legacy_syntax,
            diag,
        );

        apply_default_type_properties(&mut r);

        for sig_decl in node.CallbackDeclaration() {
            let name =
                unwrap_or_continue!(parser::identifier_text(&sig_decl.DeclaredIdentifier()); diag);

            if let Some(csn) = sig_decl.TwoWayBinding() {
                r.bindings
                    .insert(name.clone(), BindingExpression::new_uncompiled(csn.into()).into());
                r.property_declarations.insert(
                    name,
                    PropertyDeclaration {
                        property_type: Type::InferredCallback,
                        node: Some(Either::Right(sig_decl)),
                        visibility: PropertyVisibility::InOut,
                        ..Default::default()
                    },
                );
                continue;
            }

            let PropertyLookupResult {
                resolved_name: existing_name,
                property_type: maybe_existing_prop_type,
                ..
            } = r.lookup_property(&name);
            if !matches!(maybe_existing_prop_type, Type::Invalid) {
                if matches!(maybe_existing_prop_type, Type::Callback { .. }) {
                    if r.property_declarations.contains_key(&name) {
                        diag.push_error(
                            "Duplicated callback declaration".into(),
                            &sig_decl.DeclaredIdentifier(),
                        );
                    } else {
                        diag.push_error(
                            format!("Cannot override callback '{}'", existing_name),
                            &sig_decl.DeclaredIdentifier(),
                        )
                    }
                } else {
                    diag.push_error(
                        format!("Cannot declare callback '{}' when a property with the same name exists", existing_name),
                        &sig_decl.DeclaredIdentifier(),
                    );
                }
                continue;
            }

            let args = sig_decl.Type().map(|node_ty| type_from_node(node_ty, diag, tr)).collect();
            let return_type = sig_decl
                .ReturnType()
                .map(|ret_ty| Box::new(type_from_node(ret_ty.Type(), diag, tr)));
            r.property_declarations.insert(
                name,
                PropertyDeclaration {
                    property_type: Type::Callback { return_type, args },
                    node: Some(Either::Right(sig_decl)),
                    visibility: PropertyVisibility::InOut,
                    ..Default::default()
                },
            );
        }

        for con_node in node.CallbackConnection() {
            let unresolved_name = unwrap_or_continue!(parser::identifier_text(&con_node); diag);
            let PropertyLookupResult { resolved_name, property_type, .. } =
                r.lookup_property(&unresolved_name);
            if let Type::Callback { args, .. } = &property_type {
                let num_arg = con_node.DeclaredIdentifier().count();
                if num_arg > args.len() {
                    diag.push_error(
                        format!(
                            "'{}' only has {} arguments, but {} were provided",
                            unresolved_name,
                            args.len(),
                            num_arg
                        ),
                        &con_node.child_token(SyntaxKind::Identifier).unwrap(),
                    );
                }
            } else if property_type == Type::InferredCallback {
                // argument matching will happen later
            } else {
                diag.push_error(
                    format!("'{}' is not a callback in {}", unresolved_name, r.base_type),
                    &con_node.child_token(SyntaxKind::Identifier).unwrap(),
                );
                continue;
            }
            if r.bindings
                .insert(
                    resolved_name.into_owned(),
                    BindingExpression::new_uncompiled(con_node.clone().into()).into(),
                )
                .is_some()
            {
                diag.push_error(
                    "Duplicated callback".into(),
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
                    [unresolved_prop_name] => {
                        let lookup_result = r.lookup_property(unresolved_prop_name);
                        let valid_assign = lookup_result.is_valid_for_assignment();
                        if let Some(anim_element) = animation_element_from_node(
                            &anim,
                            &prop_name_token,
                            lookup_result.property_type,
                            diag,
                            tr,
                        ) {
                            if !valid_assign {
                                diag.push_error(
                                    format!(
                                        "Cannot animate {} property '{}'",
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
                            }

                            let expr_binding = r
                                .bindings
                                .entry(lookup_result.resolved_name.to_string())
                                .or_insert_with(|| {
                                    let mut r = BindingExpression::from(Expression::Invalid);
                                    r.priority = 1;
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

        let mut children_placeholder = None;
        let r = ElementRc::new(RefCell::new(r));

        for se in node.children() {
            if se.kind() == SyntaxKind::SubElement {
                let parent_type = r.borrow().base_type.clone();
                r.borrow_mut().children.push(Element::from_sub_element_node(
                    se.into(),
                    parent_type,
                    component_child_insertion_point,
                    is_legacy_syntax,
                    diag,
                    tr,
                ));
            } else if se.kind() == SyntaxKind::RepeatedElement {
                let mut sub_child_insertion_point = None;
                let rep = Element::from_repeated_node(
                    se.into(),
                    &r,
                    &mut sub_child_insertion_point,
                    is_legacy_syntax,
                    diag,
                    tr,
                );
                if let Some((_, se)) = sub_child_insertion_point {
                    diag.push_error(
                        "The @children placeholder cannot appear in a repeated element".into(),
                        &se,
                    )
                }
                r.borrow_mut().children.push(rep);
            } else if se.kind() == SyntaxKind::ConditionalElement {
                let mut sub_child_insertion_point = None;
                let rep = Element::from_conditional_node(
                    se.into(),
                    r.borrow().base_type.clone(),
                    &mut sub_child_insertion_point,
                    is_legacy_syntax,
                    diag,
                    tr,
                );
                if let Some((_, se)) = sub_child_insertion_point {
                    diag.push_error(
                        "The @children placeholder cannot appear in a conditional element".into(),
                        &se,
                    )
                }
                r.borrow_mut().children.push(rep);
            } else if se.kind() == SyntaxKind::ChildrenPlaceholder {
                if children_placeholder.is_some() {
                    diag.push_error(
                        "The @children placeholder can only appear once in an element".into(),
                        &se,
                    )
                } else {
                    children_placeholder = Some(se.clone().into());
                }
            }
        }

        if let Some(children_placeholder) = children_placeholder {
            if component_child_insertion_point.is_some() {
                diag.push_error(
                    "The @children placeholder can only appear once in an element hierarchy".into(),
                    &children_placeholder,
                )
            } else {
                *component_child_insertion_point = Some((r.clone(), children_placeholder));
            }
        }

        for state in node.States().flat_map(|s| s.State()) {
            let s = State {
                id: parser::identifier_text(&state.DeclaredIdentifier()).unwrap_or_default(),
                condition: state.Expression().map(|e| Expression::Uncompiled(e.into())),
                property_changes: state
                    .StatePropertyChange()
                    .filter_map(|s| {
                        lookup_property_from_qualified_name_for_state(s.QualifiedName(), &r, diag)
                            .map(|(ne, _)| {
                                (ne, Expression::Uncompiled(s.BindingExpression().into()), s)
                            })
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
                is_out: parser::identifier_text(&trs).unwrap_or_default() == "out",
                state_id: parser::identifier_text(&trs.DeclaredIdentifier()).unwrap_or_default(),
                property_animations: trs
                    .PropertyAnimation()
                    .flat_map(|pa| pa.QualifiedName().map(move |qn| (pa.clone(), qn)))
                    .filter_map(|(pa, qn)| {
                        lookup_property_from_qualified_name_for_state(qn.clone(), &r, diag)
                            .and_then(|(ne, prop_type)| {
                                animation_element_from_node(&pa, &qn, prop_type, diag, tr)
                                    .map(|anim_element| (ne, qn.to_source_location(), anim_element))
                            })
                    })
                    .collect(),
                node: trs.DeclaredIdentifier().into(),
            };
            r.borrow_mut().transitions.push(trans);
        }

        r
    }

    fn from_sub_element_node(
        node: syntax_nodes::SubElement,
        parent_type: ElementType,
        component_child_insertion_point: &mut Option<ChildrenInsertionPoint>,
        is_in_legacy_component: bool,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) -> ElementRc {
        let id = parser::identifier_text(&node).unwrap_or_default();
        if matches!(id.as_ref(), "parent" | "self" | "root") {
            diag.push_error(
                format!("'{}' is a reserved id", id),
                &node.child_token(SyntaxKind::Identifier).unwrap(),
            )
        }
        Element::from_node(
            node.Element(),
            id,
            parent_type,
            component_child_insertion_point,
            is_in_legacy_component,
            diag,
            tr,
        )
    }

    fn from_repeated_node(
        node: syntax_nodes::RepeatedElement,
        parent: &ElementRc,
        component_child_insertion_point: &mut Option<ChildrenInsertionPoint>,
        is_in_legacy_component: bool,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) -> ElementRc {
        let is_listview = if parent.borrow().base_type.to_string() == "ListView" {
            Some(ListViewInfo {
                viewport_y: NamedReference::new(parent, "viewport-y"),
                viewport_height: NamedReference::new(parent, "viewport-height"),
                viewport_width: NamedReference::new(parent, "viewport-width"),
                listview_height: NamedReference::new(parent, "visible-height"),
                listview_width: NamedReference::new(parent, "visible-width"),
            })
        } else {
            None
        };
        let rei = RepeatedElementInfo {
            model: Expression::Uncompiled(node.Expression().into()),
            model_data_id: node
                .DeclaredIdentifier()
                .and_then(|n| parser::identifier_text(&n))
                .unwrap_or_default(),
            index_id: node
                .RepeatedIndex()
                .and_then(|r| parser::identifier_text(&r))
                .unwrap_or_default(),
            is_conditional_element: false,
            is_listview,
        };
        let e = Element::from_sub_element_node(
            node.SubElement(),
            parent.borrow().base_type.clone(),
            component_child_insertion_point,
            is_in_legacy_component,
            diag,
            tr,
        );
        e.borrow_mut().repeated = Some(rei);
        e
    }

    fn from_conditional_node(
        node: syntax_nodes::ConditionalElement,
        parent_type: ElementType,
        component_child_insertion_point: &mut Option<ChildrenInsertionPoint>,
        is_in_legacy_component: bool,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) -> ElementRc {
        let rei = RepeatedElementInfo {
            model: Expression::Uncompiled(node.Expression().into()),
            model_data_id: String::new(),
            index_id: String::new(),
            is_conditional_element: true,
            is_listview: None,
        };
        let e = Element::from_sub_element_node(
            node.SubElement(),
            parent_type,
            component_child_insertion_point,
            is_in_legacy_component,
            diag,
            tr,
        );
        e.borrow_mut().repeated = Some(rei);
        e
    }

    /// Return the type of a property in this element or its base, along with the final name, in case
    /// the provided name points towards a property alias. Type::Invalid is returned if the property does
    /// not exist.
    pub fn lookup_property<'a>(&self, name: &'a str) -> PropertyLookupResult<'a> {
        self.property_declarations.get(name).map_or_else(
            || {
                let mut r = self.base_type.lookup_property(name);
                r.is_local_to_component = false;
                r
            },
            |p| PropertyLookupResult {
                resolved_name: name.into(),
                property_type: p.property_type.clone(),
                property_visibility: p.visibility,
                is_local_to_component: true,
            },
        )
    }

    /// Return the Span of this element in the AST for error reporting
    pub fn span(&self) -> crate::diagnostics::Span {
        self.node.as_ref().map(|n| n.span()).unwrap_or_default()
    }

    fn parse_bindings(
        &mut self,
        bindings: impl Iterator<Item = (crate::parser::SyntaxToken, SyntaxNode)>,
        is_in_legacy_component: bool,
        diag: &mut BuildDiagnostics,
    ) {
        for (name_token, b) in bindings {
            let unresolved_name = crate::parser::normalize_identifier(name_token.text());
            let lookup_result = self.lookup_property(&unresolved_name);
            if !lookup_result.property_type.is_property_type() {
                match lookup_result.property_type {
                        Type::Invalid => {
                            if self.base_type != ElementType::Error {
                                diag.push_error(if self.base_type.to_string() == "Empty" {
                                    format!( "Unknown property {unresolved_name}")
                                } else {
                                    format!( "Unknown property {unresolved_name} in {}", self.base_type)
                                },
                                &name_token);
                            }
                        }
                        Type::Callback { .. } => {
                            diag.push_error(format!("'{}' is a callback. Use `=>` to connect", unresolved_name),
                            &name_token)
                        }
                        _ => diag.push_error(format!(
                            "Cannot assign to {} in {} because it does not have a valid property type",
                            unresolved_name, self.base_type,
                        ),
                        &name_token),
                    }
            } else if !lookup_result.is_local_to_component
                && (lookup_result.property_visibility == PropertyVisibility::Private
                    || lookup_result.property_visibility == PropertyVisibility::Output)
            {
                if is_in_legacy_component
                    && lookup_result.property_visibility == PropertyVisibility::Output
                {
                    diag.push_warning(
                        format!("Assigning to output property '{unresolved_name}' is deprecated"),
                        &name_token,
                    );
                } else {
                    diag.push_error(
                        format!(
                            "Cannot assign to {} property '{}'",
                            lookup_result.property_visibility, unresolved_name
                        ),
                        &name_token,
                    );
                }
            }

            if lookup_result.resolved_name != unresolved_name {
                diag.push_property_deprecation_warning(
                    &unresolved_name,
                    &lookup_result.resolved_name,
                    &name_token,
                );
            }

            if self
                .bindings
                .insert(
                    lookup_result.resolved_name.to_string(),
                    BindingExpression::new_uncompiled(b).into(),
                )
                .is_some()
            {
                diag.push_error("Duplicated property binding".into(), &name_token);
            }
        }
    }

    pub fn native_class(&self) -> Option<Rc<NativeClass>> {
        let mut base_type = self.base_type.clone();
        loop {
            match &base_type {
                ElementType::Component(component) => {
                    base_type = component.root_element.clone().borrow().base_type.clone();
                }
                ElementType::Builtin(builtin) => break Some(builtin.native_class.clone()),
                ElementType::Native(native) => break Some(native.clone()),
                _ => break None,
            }
        }
    }

    pub fn builtin_type(&self) -> Option<Rc<BuiltinElement>> {
        let mut base_type = self.base_type.clone();
        loop {
            match &base_type {
                ElementType::Component(component) => {
                    base_type = component.root_element.clone().borrow().base_type.clone();
                }
                ElementType::Builtin(builtin) => break Some(builtin.clone()),
                _ => break None,
            }
        }
    }

    pub fn layout_info_prop(&self, orientation: Orientation) -> Option<&NamedReference> {
        self.layout_info_prop.as_ref().map(|prop| match orientation {
            Orientation::Horizontal => &prop.0,
            Orientation::Vertical => &prop.1,
        })
    }

    /// Returns the element's name as specified in the markup, not normalized.
    pub fn original_name(&self) -> String {
        self.node
            .as_ref()
            .and_then(|n| n.child_token(parser::SyntaxKind::Identifier))
            .map(|n| n.to_string())
            .unwrap_or_else(|| self.id.clone())
    }

    /// Return true if the binding is set, either on this element or in a base
    ///
    /// If `need_explicit` is true, then only consider binding set in the code, not the ones set
    /// by the compiler later.
    pub fn is_binding_set(self: &Element, property_name: &str, need_explicit: bool) -> bool {
        if self.bindings.get(property_name).map_or(false, |b| {
            b.borrow().has_binding() && (!need_explicit || b.borrow().priority > 0)
        }) {
            true
        } else if let ElementType::Component(base) = &self.base_type {
            base.root_element.borrow().is_binding_set(property_name, need_explicit)
        } else {
            false
        }
    }

    /// Set the property `property_name` of this Element only if it was not set.
    /// the `expression_fn` will only be called if it isn't set
    ///
    /// returns true if the binding was changed
    pub fn set_binding_if_not_set(
        &mut self,
        property_name: String,
        expression_fn: impl FnOnce() -> Expression,
    ) -> bool {
        if self.is_binding_set(&property_name, false) {
            return false;
        }

        match self.bindings.entry(property_name) {
            Entry::Vacant(vacant_entry) => {
                let mut binding: BindingExpression = expression_fn().into();
                binding.priority = i32::MAX;
                vacant_entry.insert(binding.into());
            }
            Entry::Occupied(mut existing_entry) => {
                let mut binding: BindingExpression = expression_fn().into();
                binding.priority = i32::MAX;
                existing_entry.get_mut().get_mut().merge_with(&binding);
            }
        };
        true
    }

    pub fn sub_component(&self) -> Option<&Rc<Component>> {
        if self.repeated.is_some() {
            None
        } else if let ElementType::Component(sub_component) = &self.base_type {
            Some(sub_component)
        } else {
            None
        }
    }
}

/// Apply default property values defined in `builtins.slint` to the element.
fn apply_default_type_properties(element: &mut Element) {
    // Apply default property values on top:
    if let ElementType::Builtin(builtin_base) = &element.base_type {
        for (prop, info) in &builtin_base.properties {
            if let Some(expr) = &info.default_value {
                element.bindings.entry(prop.clone()).or_insert_with(|| {
                    let mut binding = BindingExpression::from(expr.clone());
                    binding.priority = i32::MAX;
                    RefCell::new(binding)
                });
            }
        }
    }
}

/// Create a Type for this node
pub fn type_from_node(
    node: syntax_nodes::Type,
    diag: &mut BuildDiagnostics,
    tr: &TypeRegister,
) -> Type {
    if let Some(qualified_type_node) = node.QualifiedName() {
        let qualified_type = QualifiedTypeName::from_node(qualified_type_node.clone());

        let prop_type = tr.lookup_qualified(&qualified_type.members);

        if prop_type == Type::Invalid && tr.lookup_element(&qualified_type.to_string()).is_err() {
            diag.push_error(format!("Unknown type '{}'", qualified_type), &qualified_type_node);
        } else if !prop_type.is_property_type() {
            diag.push_error(
                format!("'{}' is not a valid type", qualified_type),
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
    diag: &mut BuildDiagnostics,
    tr: &TypeRegister,
) -> Type {
    let fields = object_node
        .ObjectTypeMember()
        .map(|member| {
            (
                parser::identifier_text(&member).unwrap_or_default(),
                type_from_node(member.Type(), diag, tr),
            )
        })
        .collect();
    Type::Struct { fields, name: None, node: Some(object_node) }
}

fn animation_element_from_node(
    anim: &syntax_nodes::PropertyAnimation,
    prop_name: &syntax_nodes::QualifiedName,
    prop_type: Type,
    diag: &mut BuildDiagnostics,
    tr: &TypeRegister,
) -> Option<ElementRc> {
    let anim_type = tr.property_animation_type_for_property(prop_type);
    if !matches!(anim_type, ElementType::Builtin(..)) {
        diag.push_error(
            format!(
                "'{}' is not a property that can be animated",
                prop_name.text().to_string().trim()
            ),
            prop_name,
        );
        None
    } else {
        let mut anim_element =
            Element { id: "".into(), base_type: anim_type, node: None, ..Default::default() };
        anim_element.parse_bindings(
            anim.Binding().filter_map(|b| {
                Some((b.child_token(SyntaxKind::Identifier)?, b.BindingExpression().into()))
            }),
            false,
            diag,
        );

        apply_default_type_properties(&mut anim_element);

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

impl Display for QualifiedTypeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.members.join("."))
    }
}

/// Return a NamedReference for a qualified name used in a state (or transition),
/// if the reference is invalid, there will be a diagnostic
fn lookup_property_from_qualified_name_for_state(
    node: syntax_nodes::QualifiedName,
    r: &Rc<RefCell<Element>>,
    diag: &mut BuildDiagnostics,
) -> Option<(NamedReference, Type)> {
    let qualname = QualifiedTypeName::from_node(node.clone());
    match qualname.members.as_slice() {
        [unresolved_prop_name] => {
            let lookup_result = r.borrow().lookup_property(unresolved_prop_name.as_ref());
            if !lookup_result.property_type.is_property_type() {
                diag.push_error(format!("'{}' is not a valid property", qualname), &node);
            } else if !lookup_result.is_valid_for_assignment() {
                diag.push_error(
                    format!(
                        "'{}' cannot be set in a state because it is {}",
                        qualname, lookup_result.property_visibility
                    ),
                    &node,
                );
            }
            Some((
                NamedReference::new(r, &lookup_result.resolved_name),
                lookup_result.property_type,
            ))
        }
        [elem_id, unresolved_prop_name] => {
            if let Some(element) = find_element_by_id(r, elem_id.as_ref()) {
                let lookup_result = element.borrow().lookup_property(unresolved_prop_name.as_ref());
                if !lookup_result.is_valid() {
                    diag.push_error(
                        format!("'{}' not found in '{}'", unresolved_prop_name, elem_id),
                        &node,
                    );
                } else if !lookup_result.is_valid_for_assignment() {
                    diag.push_error(
                        format!(
                            "'{}' cannot be set in a state because it is {}",
                            qualname, lookup_result.property_visibility
                        ),
                        &node,
                    );
                }
                Some((
                    NamedReference::new(&element, &lookup_result.resolved_name),
                    lookup_result.property_type,
                ))
            } else {
                diag.push_error(format!("'{}' is not a valid element id", elem_id), &node);
                None
            }
        }
        _ => {
            diag.push_error(format!("'{}' is not a valid property", qualname), &node);
            None
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

/// Find the parent element to a given element.
/// (since there is no parent mapping we need to fo an exhaustive search)
pub fn find_parent_element(e: &ElementRc) -> Option<ElementRc> {
    fn recurse(base: &ElementRc, e: &ElementRc) -> Option<ElementRc> {
        for child in &base.borrow().children {
            if Rc::ptr_eq(child, e) {
                return Some(base.clone());
            }
            if let Some(x) = recurse(child, e) {
                return Some(x);
            }
        }
        None
    }

    let root = e.borrow().enclosing_component.upgrade().unwrap().root_element.clone();
    if Rc::ptr_eq(&root, e) {
        return None;
    }
    recurse(&root, e)
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
        debug_assert!(std::ptr::eq(
            component as *const Component,
            (&*elem.borrow().enclosing_component.upgrade().unwrap()) as *const Component
        ));
        if elem.borrow().repeated.is_some() {
            if let ElementType::Component(base) = &elem.borrow().base_type {
                if base.parent_element.upgrade().is_some() {
                    recurse_elem_including_sub_components(base, state, vis);
                }
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
        let base = if elem.borrow().repeated.is_some() {
            if let ElementType::Component(base) = &elem.borrow().base_type {
                Some(base.clone())
            } else {
                None
            }
        } else {
            None
        };
        if let Some(base) = base {
            recurse_elem_including_sub_components_no_borrow(&base, state, vis);
        }
        vis(elem, state)
    });
    component
        .popup_windows
        .borrow()
        .iter()
        .for_each(|p| recurse_elem_including_sub_components_no_borrow(&p.component, state, vis));
    component
        .used_types
        .borrow()
        .globals
        .iter()
        .for_each(|p| recurse_elem_including_sub_components_no_borrow(p, state, vis));
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
        for (name, expr) in &elem.borrow().bindings {
            vis(&mut *expr.borrow_mut(), Some(name.as_str()), &|| {
                elem.borrow().lookup_property(name).property_type
            });

            match &mut expr.borrow_mut().animation {
                Some(PropertyAnimation::Static(e)) => visit_element_expressions_simple(e, vis),
                Some(PropertyAnimation::Transition { animations, state_ref }) => {
                    vis(state_ref, None, &|| Type::Int32);
                    for a in animations {
                        visit_element_expressions_simple(&a.animation, vis)
                    }
                }
                None => (),
            }
        }
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
        for (ne, e, _) in &mut s.property_changes {
            vis(e, Some(ne.name()), &|| {
                ne.element().borrow().lookup_property(ne.name()).property_type
            });
        }
    }
    elem.borrow_mut().states = states;

    let mut transitions = std::mem::take(&mut elem.borrow_mut().transitions);
    for t in &mut transitions {
        for (_, _, a) in &mut t.property_animations {
            visit_element_expressions_simple(a, &mut vis);
        }
    }
    elem.borrow_mut().transitions = transitions;
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
            Expression::LayoutCacheAccess { layout_cache_prop, .. } => vis(layout_cache_prop),
            Expression::SolveLayout(l, _) => l.visit_named_references(vis),
            Expression::ComputeLayoutInfo(l, _) => l.visit_named_references(vis),
            // This is not really a named reference, but the result is the same, it need to be updated
            // FIXME: this should probably be lowered into a PropertyReference
            Expression::RepeaterModelReference { element }
            | Expression::RepeaterIndexReference { element } => {
                // FIXME: this is questionable
                let mut nc = NamedReference::new(&element.upgrade().unwrap(), "$model");
                vis(&mut nc);
                debug_assert!(nc.element().borrow().repeated.is_some());
                *element = Rc::downgrade(&nc.element());
            }
            _ => {}
        }
    }
    visit_element_expressions(elem, |expr, _, _| recurse_expression(expr, &mut vis));
    let mut states = std::mem::take(&mut elem.borrow_mut().states);
    for s in &mut states {
        for (r, _, _) in &mut s.property_changes {
            vis(r);
        }
    }
    elem.borrow_mut().states = states;
    let mut transitions = std::mem::take(&mut elem.borrow_mut().transitions);
    for t in &mut transitions {
        for (r, _, _) in &mut t.property_animations {
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
    let mut layout_info_prop = std::mem::take(&mut elem.borrow_mut().layout_info_prop);
    layout_info_prop.as_mut().map(|(h, b)| (vis(h), vis(b)));
    elem.borrow_mut().layout_info_prop = layout_info_prop;

    let mut accessibility_props = std::mem::take(&mut elem.borrow_mut().accessibility_props);
    accessibility_props.0.iter_mut().for_each(|(_, x)| vis(x));
    elem.borrow_mut().accessibility_props = accessibility_props;

    // visit two way bindings
    for expr in elem.borrow().bindings.values() {
        for nr in &mut expr.borrow_mut().two_way_bindings {
            vis(nr);
        }
    }

    let mut property_declarations = std::mem::take(&mut elem.borrow_mut().property_declarations);
    for pd in property_declarations.values_mut() {
        pd.is_alias.as_mut().map(&mut vis);
    }
    elem.borrow_mut().property_declarations = property_declarations;
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
                compo.root_constraints.borrow_mut().visit_named_references(vis);
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
    pub property_changes: Vec<(NamedReference, Expression, syntax_nodes::StatePropertyChange)>,
}

#[derive(Debug, Clone)]
pub struct Transition {
    /// false for 'to', true for 'out'
    pub is_out: bool,
    pub state_id: String,
    pub property_animations: Vec<(NamedReference, SourceLocation, ElementRc)>,
    /// Node pointing to the state name
    pub node: SyntaxNode,
}

#[derive(Clone, Debug, derive_more::Deref)]
pub struct ExportedName {
    #[deref]
    pub name: String, // normalized
    pub name_ident: SyntaxNode,
}

impl ExportedName {
    pub fn original_name(&self) -> String {
        self.name_ident
            .child_token(parser::SyntaxKind::Identifier)
            .map(|n| n.to_string())
            .unwrap_or_else(|| self.name.clone())
    }
}

#[derive(Default, Debug, derive_more::Deref)]
pub struct Exports(pub Vec<(ExportedName, Either<Rc<Component>, Type>)>);

impl Exports {
    pub fn from_node(
        doc: &syntax_nodes::Document,
        inner_components: &[Rc<Component>],
        type_registry: &TypeRegister,
        diag: &mut BuildDiagnostics,
    ) -> Self {
        #[derive(Debug, Clone)]
        struct NamedExport {
            internal_name_ident: SyntaxNode,
            internal_name: String,
            external_name_ident: SyntaxNode,
            exported_name: String,
        }

        let exports_it = doc.ExportsList().flat_map(|exports| exports.ExportSpecifier()).map(
            |export_specifier| {
                let internal_name = parser::identifier_text(&export_specifier.ExportIdentifier())
                    .unwrap_or_else(|| {
                        debug_assert!(diag.has_error());
                        String::new()
                    });

                let (exported_name, name_location): (String, SyntaxNode) = export_specifier
                    .ExportName()
                    .and_then(|ident| {
                        parser::identifier_text(&ident).map(|text| (text, ident.clone().into()))
                    })
                    .unwrap_or_else(|| {
                        (internal_name.clone(), export_specifier.ExportIdentifier().into())
                    });

                NamedExport {
                    internal_name_ident: export_specifier.ExportIdentifier().into(),
                    internal_name,
                    external_name_ident: name_location,
                    exported_name,
                }
            },
        );

        let exports_it = exports_it.chain(
            doc.ExportsList().filter_map(|exports| exports.Component()).map(|component| {
                let name_location: SyntaxNode = component.DeclaredIdentifier().into();
                let name =
                    parser::identifier_text(&component.DeclaredIdentifier()).unwrap_or_else(|| {
                        debug_assert!(diag.has_error());
                        String::new()
                    });
                NamedExport {
                    internal_name_ident: name_location.clone(),
                    internal_name: name.clone(),
                    external_name_ident: name_location,
                    exported_name: name,
                }
            }),
        );
        let exports_it = exports_it.chain(
            doc.ExportsList().flat_map(|exports| exports.StructDeclaration()).map(|st| {
                let name_location: SyntaxNode = st.DeclaredIdentifier().into();
                let name = parser::identifier_text(&st.DeclaredIdentifier()).unwrap_or_else(|| {
                    debug_assert!(diag.has_error());
                    String::new()
                });
                NamedExport {
                    internal_name_ident: name_location.clone(),
                    internal_name: name.clone(),
                    external_name_ident: name_location,
                    exported_name: name,
                }
            }),
        );

        struct SeenExport {
            name_location: SyntaxNode,
            warned: bool,
        }
        let mut seen_exports: HashMap<String, SeenExport> = HashMap::new();
        let mut export_diagnostics = Vec::new();

        let mut exports: Vec<_> = exports_it
            .filter(|export| {
                if let Some(other_loc) = seen_exports.get_mut(&export.exported_name) {
                    let message = format!("Duplicated export '{}'", export.exported_name);
                    if !other_loc.warned {
                        export_diagnostics.push((message.clone(), other_loc.name_location.clone()));
                        other_loc.warned = true;
                    }
                    export_diagnostics.push((message, export.external_name_ident.clone()));
                    false
                } else {
                    seen_exports.insert(
                        export.exported_name.clone(),
                        SeenExport {
                            name_location: export.external_name_ident.clone(),
                            warned: false,
                        },
                    );

                    true
                }
            })
            .collect();

        for (message, location) in export_diagnostics {
            diag.push_error(message, &location);
        }

        if exports.is_empty() {
            if let Some(internal_name) = inner_components.last().as_ref().map(|x| x.id.clone()) {
                exports.push(NamedExport {
                    internal_name_ident: doc.clone().into(),
                    internal_name: internal_name.clone(),
                    external_name_ident: doc.clone().into(),
                    exported_name: internal_name,
                })
            }
        }

        let mut resolve_export_to_inner_component_or_import = |export: &NamedExport| {
            if let Ok(ElementType::Component(c)) =
                type_registry.lookup_element(export.internal_name.as_str())
            {
                Some(Either::Left(c))
            } else if let ty @ Type::Struct { .. } =
                type_registry.lookup(export.internal_name.as_str())
            {
                Some(Either::Right(ty))
            } else if type_registry.lookup_element(export.internal_name.as_str()).is_ok()
                || type_registry.lookup(export.internal_name.as_str()) != Type::Invalid
            {
                diag.push_error(
                    format!(
                        "Cannot export '{}' because it is not a component",
                        export.internal_name,
                    ),
                    &export.internal_name_ident,
                );
                None
            } else {
                diag.push_error(
                    format!("'{}' not found", export.internal_name,),
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
                        ExportedName {
                            name: export.exported_name.clone(),
                            name_ident: export.external_name_ident.clone(),
                        },
                        resolve_export_to_inner_component_or_import(export)?,
                    ))
                })
                .collect(),
        )
    }
}

/// This function replace the root element of a repeated element. the previous root becomes the only
/// child of the new root element.
/// Note that no reference to the base component must exist outside of repeated_element.base_type
pub fn inject_element_as_repeated_element(repeated_element: &ElementRc, new_root: ElementRc) {
    let component = repeated_element.borrow().base_type.as_component().clone();
    // Since we're going to replace the repeated element's component, we need to assert that
    // outside this function no strong reference exists to it. Then we can unwrap and
    // replace the root element.
    debug_assert_eq!(Rc::strong_count(&component), 2);
    let old_root = &component.root_element;

    adjust_geometry_for_injected_parent(&new_root, old_root);

    // Any elements with a weak reference to the repeater's component will need fixing later.
    let mut elements_with_enclosing_component_reference = Vec::new();
    recurse_elem(old_root, &(), &mut |element: &ElementRc, _| {
        if let Some(enclosing_component) = element.borrow().enclosing_component.upgrade() {
            if Rc::ptr_eq(&enclosing_component, &component) {
                elements_with_enclosing_component_reference.push(element.clone());
            }
        }
    });
    elements_with_enclosing_component_reference
        .extend_from_slice(component.optimized_elements.borrow().as_slice());
    elements_with_enclosing_component_reference.push(new_root.clone());

    new_root.borrow_mut().child_of_layout =
        std::mem::replace(&mut old_root.borrow_mut().child_of_layout, false);
    new_root.borrow_mut().layout_info_prop = old_root.borrow().layout_info_prop.clone();

    // Replace the repeated component's element with our shadow element. That requires a bit of reference counting
    // surgery and relies on nobody having a strong reference left to the component, which we take out of the Rc.
    drop(std::mem::take(&mut repeated_element.borrow_mut().base_type));

    debug_assert_eq!(Rc::strong_count(&component), 1);

    let mut component = Rc::try_unwrap(component).expect("internal compiler error: more than one strong reference left to repeated component when lowering shadow properties");

    let old_root = std::mem::replace(&mut component.root_element, new_root.clone());
    new_root.borrow_mut().children.push(old_root);

    let component = Rc::new(component);
    repeated_element.borrow_mut().base_type = ElementType::Component(component.clone());

    for elem in elements_with_enclosing_component_reference {
        elem.borrow_mut().enclosing_component = Rc::downgrade(&component);
    }
}

/// Make the geometry of the `injected_parent` that of the old_elem. And the old_elem
/// will cover the `injected_parent`
pub fn adjust_geometry_for_injected_parent(injected_parent: &ElementRc, old_elem: &ElementRc) {
    // The values for properties that affect the geometry may be supplied in two different ways:
    //
    //   * When coming from the outside, for example by the repeater being inside a layout, we need
    //     the values to apply to the new root element and the old root just needs to follow.
    //   * When coming from the inside, for example when the repeater just creates rectangles that
    //     calculate their own position, we need to move those bindings as well to the new root.
    injected_parent.borrow_mut().bindings.extend(Iterator::chain(
        ["x", "y", "z"].iter().filter_map(|x| old_elem.borrow_mut().bindings.remove_entry(*x)),
        ["width", "height"].iter().map(|x| {
            (
                x.to_string(),
                BindingExpression::from(Expression::PropertyReference(NamedReference::new(
                    old_elem, x,
                )))
                .into(),
            )
        }),
    ));
    injected_parent.borrow().property_analysis.borrow_mut().extend(
        ["x", "y", "z"].into_iter().filter_map(|x| {
            old_elem
                .borrow()
                .property_analysis
                .borrow()
                .get_key_value(x)
                .map(|(k, v)| (k.clone(), v.clone()))
        }),
    );
}
