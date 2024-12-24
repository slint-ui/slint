// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
 This module contains the intermediate representation of the code in the form of an object tree
*/

// cSpell: ignore qualname

use crate::diagnostics::{BuildDiagnostics, SourceLocation, Spanned};
use crate::expression_tree::{self, BindingExpression, Expression, Unit};
use crate::langtype::{
    BuiltinElement, BuiltinPropertyDefault, Enumeration, EnumerationValue, Function, NativeClass,
    Struct, Type,
};
use crate::langtype::{ElementType, PropertyLookupResult};
use crate::layout::{LayoutConstraints, Orientation};
use crate::namedreference::NamedReference;
use crate::parser;
use crate::parser::{syntax_nodes, SyntaxKind, SyntaxNode};
use crate::typeloader::{ImportKind, ImportedTypes};
use crate::typeregister::TypeRegister;
use itertools::Either;
use once_cell::unsync::OnceCell;
use smol_str::{format_smolstr, SmolStr, ToSmolStr};
use std::cell::{Cell, RefCell};
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Display;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

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

/// The full document (a complete file)
#[derive(Default, Debug)]
pub struct Document {
    pub node: Option<syntax_nodes::Document>,
    pub inner_components: Vec<Rc<Component>>,
    pub inner_types: Vec<Type>,
    pub local_registry: TypeRegister,
    /// A list of paths to .ttf/.ttc files that are supposed to be registered on
    /// startup for custom font use.
    pub custom_fonts: Vec<(SmolStr, crate::parser::SyntaxToken)>,
    pub exports: Exports,
    pub imports: Vec<ImportedTypes>,

    /// Map of resources that should be embedded in the generated code, indexed by their absolute path on
    /// disk on the build system
    pub embedded_file_resources:
        RefCell<HashMap<SmolStr, crate::embedded_resources::EmbeddedResources>>,

    /// The list of used extra types used recursively.
    pub used_types: RefCell<UsedSubTypes>,

    /// The popup_menu_impl
    pub popup_menu_impl: Option<Rc<Component>>,
}

impl Document {
    pub fn from_node(
        node: syntax_nodes::Document,
        imports: Vec<ImportedTypes>,
        reexports: Exports,
        diag: &mut BuildDiagnostics,
        parent_registry: &Rc<RefCell<TypeRegister>>,
    ) -> Self {
        debug_assert_eq!(node.kind(), SyntaxKind::Document);

        let mut local_registry = TypeRegister::new(parent_registry);
        let mut inner_components = vec![];
        let mut inner_types = vec![];

        let mut process_component =
            |n: syntax_nodes::Component,
             diag: &mut BuildDiagnostics,
             local_registry: &mut TypeRegister| {
                let compo = Component::from_node(n, diag, local_registry);
                local_registry.add(compo.clone());
                inner_components.push(compo);
            };
        let process_struct = |n: syntax_nodes::StructDeclaration,
                              diag: &mut BuildDiagnostics,
                              local_registry: &mut TypeRegister,
                              inner_types: &mut Vec<Type>| {
            let rust_attributes = n.AtRustAttr().map(|child| vec![child.text().to_smolstr()]);
            let ty = type_struct_from_node(
                n.ObjectType(),
                diag,
                local_registry,
                rust_attributes,
                parser::identifier_text(&n.DeclaredIdentifier()),
            );
            assert!(matches!(ty, Type::Struct(_)));
            local_registry.insert_type(ty.clone());
            inner_types.push(ty);
        };
        let process_enum = |n: syntax_nodes::EnumDeclaration,
                            diag: &mut BuildDiagnostics,
                            local_registry: &mut TypeRegister,
                            inner_types: &mut Vec<Type>| {
            let Some(name) = parser::identifier_text(&n.DeclaredIdentifier()) else {
                assert!(diag.has_errors());
                return;
            };
            let mut existing_names = HashSet::new();
            let values = n
                .EnumValue()
                .filter_map(|v| {
                    let value = parser::identifier_text(&v)?;
                    if value == name {
                        diag.push_error(
                            format!("Enum '{value}' can't have a value with the same name"),
                            &v,
                        );
                        None
                    } else if !existing_names.insert(crate::generator::to_pascal_case(&value)) {
                        diag.push_error(format!("Duplicated enum value '{value}'"), &v);
                        None
                    } else {
                        Some(value)
                    }
                })
                .collect();
            let en = Enumeration { name: name.clone(), values, default_value: 0, node: Some(n) };
            let ty = Type::Enumeration(Rc::new(en));
            local_registry.insert_type_with_name(ty.clone(), name);
            inner_types.push(ty);
        };

        for n in node.children() {
            match n.kind() {
                SyntaxKind::Component => process_component(n.into(), diag, &mut local_registry),
                SyntaxKind::StructDeclaration => {
                    process_struct(n.into(), diag, &mut local_registry, &mut inner_types)
                }
                SyntaxKind::EnumDeclaration => {
                    process_enum(n.into(), diag, &mut local_registry, &mut inner_types)
                }
                SyntaxKind::ExportsList => {
                    for n in n.children() {
                        match n.kind() {
                            SyntaxKind::Component => {
                                process_component(n.into(), diag, &mut local_registry)
                            }
                            SyntaxKind::StructDeclaration => process_struct(
                                n.into(),
                                diag,
                                &mut local_registry,
                                &mut inner_types,
                            ),
                            SyntaxKind::EnumDeclaration => {
                                process_enum(n.into(), diag, &mut local_registry, &mut inner_types)
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            };
        }
        let mut exports = Exports::from_node(&node, &inner_components, &local_registry, diag);
        exports.add_reexports(reexports, diag);

        let custom_fonts = imports
            .iter()
            .filter(|import| matches!(import.import_kind, ImportKind::FileImport))
            .filter_map(|import| {
                if import.file.ends_with(".ttc")
                    || import.file.ends_with(".ttf")
                    || import.file.ends_with(".otf")
                {
                    let token_path = import.import_uri_token.source_file.path();
                    let import_file_path = PathBuf::from(import.file.clone());
                    let import_file_path = crate::pathutils::join(token_path, &import_file_path)
                        .unwrap_or(import_file_path);

                    // Assume remote urls are valid, we need to load them at run-time (which we currently don't). For
                    // local paths we should try to verify the existence and let the developer know ASAP.
                    if crate::pathutils::is_url(&import_file_path)
                        || crate::fileaccess::load_file(std::path::Path::new(&import_file_path))
                            .is_some()
                    {
                        Some((import_file_path.to_string_lossy().into(), import.import_uri_token.clone()))
                    } else {
                        diag.push_error(
                            format!("File \"{}\" not found", import.file),
                            &import.import_uri_token,
                        );
                        None
                    }
                } else if import.file.ends_with(".slint") {
                    diag.push_error("Import names are missing. Please specify which types you would like to import".into(), &import.import_uri_token.parent());
                    None
                } else {
                    diag.push_error(
                        format!("Unsupported foreign import \"{}\"", import.file),
                        &import.import_uri_token,
                    );
                    None
                }
            })
            .collect();

        for local_compo in &inner_components {
            if exports
                .components_or_types
                .iter()
                .filter_map(|(_, exported_compo_or_type)| exported_compo_or_type.as_ref().left())
                .any(|exported_compo| Rc::ptr_eq(exported_compo, local_compo))
            {
                continue;
            }
            // Don't warn about these for now - detecting their use can only be done after the resolve_expressions
            // pass.
            if local_compo.is_global() {
                continue;
            }
            // First ref count is in the type registry, the second one in inner_components. Any use of the element
            // would have resulted in another strong reference.
            if Rc::strong_count(local_compo) == 2 {
                diag.push_warning(
                    "Component is neither used nor exported".into(),
                    &local_compo.node,
                )
            }
        }

        Document {
            node: Some(node),
            inner_components,
            inner_types,
            local_registry,
            custom_fonts,
            imports,
            exports,
            embedded_file_resources: Default::default(),
            used_types: Default::default(),
            popup_menu_impl: None,
        }
    }

    pub fn exported_roots(&self) -> impl DoubleEndedIterator<Item = Rc<Component>> + '_ {
        self.exports.iter().filter_map(|e| e.1.as_ref().left()).filter(|c| !c.is_global()).cloned()
    }

    /// This is the component that is going to be instantiated by the interpreter
    pub fn last_exported_component(&self) -> Option<Rc<Component>> {
        self.exports
            .iter()
            .filter_map(|e| Some((&e.0.name_ident, e.1.as_ref().left()?)))
            .filter(|(_, c)| !c.is_global())
            .max_by_key(|(n, _)| n.text_range().end())
            .map(|(_, c)| c.clone())
    }

    /// visit all root and used component (including globals)
    pub fn visit_all_used_components(&self, mut v: impl FnMut(&Rc<Component>)) {
        let used_types = self.used_types.borrow();
        for c in &used_types.sub_components {
            v(c);
        }
        for c in self.exported_roots() {
            v(&c);
        }
        for c in &used_types.globals {
            v(c);
        }
        if let Some(c) = &self.popup_menu_impl {
            v(c);
        }
    }
}

#[derive(Debug, Clone)]
pub struct PopupWindow {
    pub component: Rc<Component>,
    pub x: NamedReference,
    pub y: NamedReference,
    pub close_policy: EnumerationValue,
    pub parent_element: ElementRc,
}

#[derive(Debug, Clone)]
pub struct Timer {
    pub interval: NamedReference,
    pub triggered: NamedReference,
    pub running: NamedReference,
}

type ChildrenInsertionPoint = (ElementRc, usize, syntax_nodes::ChildrenPlaceholder);

/// Used sub types for a root component
#[derive(Debug, Default)]
pub struct UsedSubTypes {
    /// All the globals used by the component and its children.
    pub globals: Vec<Rc<Component>>,
    /// All the structs and enums used by the component and its children.
    pub structs_and_enums: Vec<Type>,
    /// All the sub components use by this components and its children,
    /// and the amount of time it is used
    pub sub_components: Vec<Rc<Component>>,
}

#[derive(Debug, Default, Clone)]
pub struct InitCode {
    // Code from init callbacks collected from elements
    pub constructor_code: Vec<Expression>,
    /// Code to set the initial focus via forward-focus on the Window
    pub focus_setting_code: Vec<Expression>,
    /// Code to register embedded fonts.
    pub font_registration_code: Vec<Expression>,

    /// Code inserted from inlined components, ordered by offset of the place where it was inlined from. This way
    /// we can preserve the order across multiple inlining passes.
    pub inlined_init_code: BTreeMap<usize, Expression>,
}

impl InitCode {
    pub fn iter(&self) -> impl Iterator<Item = &Expression> {
        self.font_registration_code
            .iter()
            .chain(self.focus_setting_code.iter())
            .chain(self.constructor_code.iter())
            .chain(self.inlined_init_code.values())
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Expression> {
        self.font_registration_code
            .iter_mut()
            .chain(self.focus_setting_code.iter_mut())
            .chain(self.constructor_code.iter_mut())
            .chain(self.inlined_init_code.values_mut())
    }
}

/// A component is a type in the language which can be instantiated,
/// Or is materialized for repeated expression.
#[derive(Default, Debug)]
pub struct Component {
    pub node: Option<SyntaxNode>,
    pub id: SmolStr,
    pub root_element: ElementRc,

    /// The parent element within the parent component if this component represents a repeated element
    pub parent_element: Weak<RefCell<Element>>,

    /// List of elements that are not attached to the root anymore because they have been
    /// optimized away, but their properties may still be in use
    pub optimized_elements: RefCell<Vec<ElementRc>>,

    /// The layout constraints of the root item
    pub root_constraints: RefCell<LayoutConstraints>,

    /// When creating this component and inserting "children", append them to the children of
    /// the element pointer to by this field.
    pub child_insertion_point: RefCell<Option<ChildrenInsertionPoint>>,

    pub init_code: RefCell<InitCode>,

    pub popup_windows: RefCell<Vec<PopupWindow>>,
    pub timers: RefCell<Vec<Timer>>,

    /// This component actually inherits PopupWindow (although that has been changed to a Window by the lower_popups pass)
    pub inherits_popup_window: Cell<bool>,

    /// The names under which this component should be accessible
    /// if it is a global singleton and exported.
    pub exported_global_names: RefCell<Vec<ExportedName>>,

    /// The list of properties (name and type) declared as private in the component.
    /// This is used to issue better error in the generated code if the property is used.
    pub private_properties: RefCell<Vec<(SmolStr, Type)>>,
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
            node: Some(node.clone().into()),
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
            e.borrow_mut().enclosing_component = weak.clone();
            if let Some(qualified_id) =
                e.borrow_mut().debug.first_mut().and_then(|x| x.qualified_id.as_mut())
            {
                *qualified_id = format_smolstr!("{}::{}", c.id, qualified_id);
            }
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

    /// Returns the names of aliases to global singletons, exactly as
    /// specified in the .slint markup (not normalized).
    pub fn global_aliases(&self) -> Vec<SmolStr> {
        self.exported_global_names
            .borrow()
            .iter()
            .filter(|name| name.as_str() != self.root_element.borrow().id)
            .map(|name| name.original_name())
            .collect()
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum PropertyVisibility {
    #[default]
    Private,
    Input,
    Output,
    InOut,
    /// for builtin properties that must be known at compile time and cannot be changed at runtime
    Constexpr,
    /// For builtin properties that are meant to just be bindings but cannot be read or written
    /// (eg, Path's `commands`)
    Fake,
    /// For functions, not properties
    Public,
    Protected,
}

impl Display for PropertyVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropertyVisibility::Private => f.write_str("private"),
            PropertyVisibility::Input => f.write_str("input"),
            PropertyVisibility::Output => f.write_str("output"),
            PropertyVisibility::InOut => f.write_str("input output"),
            PropertyVisibility::Constexpr => f.write_str("constexpr"),
            PropertyVisibility::Public => f.write_str("public"),
            PropertyVisibility::Protected => f.write_str("protected"),
            PropertyVisibility::Fake => f.write_str("fake"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PropertyDeclaration {
    pub property_type: Type,
    pub node: Option<SyntaxNode>,
    /// Tells if getter and setter will be added to expose in the native language API
    pub expose_in_public_api: bool,
    /// Public API property exposed as an alias: it shouldn't be generated but instead forward to the alias.
    pub is_alias: Option<NamedReference>,
    pub visibility: PropertyVisibility,
    /// For function or callback: whether it is declared as `pure` (None for private function for which this has to be deduced)
    pub pure: Option<bool>,
}

impl PropertyDeclaration {
    // For diagnostics: return a node pointing to the type
    pub fn type_node(&self) -> Option<SyntaxNode> {
        let node = self.node.as_ref()?;
        if let Some(x) = syntax_nodes::PropertyDeclaration::new(node.clone()) {
            Some(x.Type().map_or_else(|| x.into(), |x| x.into()))
        } else {
            node.clone().into()
        }
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
                debug: e.debug.clone(),
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

#[derive(Clone, Debug)]
pub struct GeometryProps {
    pub x: NamedReference,
    pub y: NamedReference,
    pub width: NamedReference,
    pub height: NamedReference,
}

impl GeometryProps {
    pub fn new(element: &ElementRc) -> Self {
        Self {
            x: NamedReference::new(element, SmolStr::new_static("x")),
            y: NamedReference::new(element, SmolStr::new_static("y")),
            width: NamedReference::new(element, SmolStr::new_static("width")),
            height: NamedReference::new(element, SmolStr::new_static("height")),
        }
    }
}

pub type BindingsMap = BTreeMap<SmolStr, RefCell<BindingExpression>>;

#[derive(Clone)]
pub struct ElementDebugInfo {
    // The id qualified with the enclosing component name. Given `foo := Bar {}` this is `EnclosingComponent::foo`
    pub qualified_id: Option<SmolStr>,
    pub type_name: String,
    pub node: syntax_nodes::Element,
    // Field to indicate wether this element was a layout that had
    // been lowered into a rectangle in the lower_layouts pass.
    pub layout: Option<crate::layout::Layout>,
    /// Set to true if the ElementDebugInfo following this one in the debug vector
    /// in Element::debug is the last one and the next entry belongs to an other element.
    /// This can happen as a result of rectangle optimization, for example.
    pub element_boundary: bool,
}

impl ElementDebugInfo {
    // Returns a comma separate string that encodes the element type name (`Rectangle`, `MyButton`, etc.)
    // and the qualified id (`SurroundingComponent::my-id`).
    fn encoded_element_info(&self) -> String {
        let mut info = self.type_name.clone();
        info.push(',');
        if let Some(id) = self.qualified_id.as_ref() {
            info.push_str(id);
        }
        info
    }
}

/// An Element is an instantiation of a Component
#[derive(Default)]
pub struct Element {
    /// The id as named in the original .slint file.
    ///
    /// Note that it can only be used for lookup before inlining.
    /// After inlining there can be duplicated id in the component.
    /// The id are then re-assigned unique id in the assign_id pass
    pub id: SmolStr,
    //pub base: QualifiedTypeName,
    pub base_type: ElementType,
    /// Currently contains also the callbacks. FIXME: should that be changed?
    pub bindings: BindingsMap,
    pub change_callbacks: BTreeMap<SmolStr, RefCell<Vec<Expression>>>,
    pub property_analysis: RefCell<HashMap<SmolStr, PropertyAnalysis>>,

    pub children: Vec<ElementRc>,
    /// The component which contains this element.
    pub enclosing_component: Weak<Component>,

    pub property_declarations: BTreeMap<SmolStr, PropertyDeclaration>,

    /// Main owner for a reference to a property.
    pub named_references: crate::namedreference::NamedReferenceContainer,

    /// This element is part of a `for <xxx> in <model>`:
    pub repeated: Option<RepeatedElementInfo>,
    /// This element is a placeholder to embed an Component at
    pub is_component_placeholder: bool,

    pub states: Vec<State>,
    pub transitions: Vec<Transition>,

    /// true when this item's geometry is handled by a layout
    pub child_of_layout: bool,
    /// The property pointing to the layout info. `(horizontal, vertical)`
    pub layout_info_prop: Option<(NamedReference, NamedReference)>,
    /// Whether we have `preferred-{width,height}: 100%`
    pub default_fill_parent: (bool, bool),

    pub accessibility_props: AccessibilityProps,

    /// Reference to the property.
    /// This is always initialized from the element constructor, but is Option because it references itself
    pub geometry_props: Option<GeometryProps>,

    /// true if this Element is the fake Flickable viewport
    pub is_flickable_viewport: bool,

    /// true if this Element may have a popup as child meaning it cannot be optimized
    /// because the popup references it.
    pub has_popup_child: bool,

    /// This is the component-local index of this item in the item tree array.
    /// It is generated after the last pass and before the generators run.
    pub item_index: OnceCell<u32>,
    /// the index of the first children in the tree, set with item_index
    pub item_index_of_first_children: OnceCell<u32>,

    /// True when this element is in a component was declared with the `:=` symbol instead of the `component` keyword
    pub is_legacy_syntax: bool,

    /// How many times the element was inlined
    pub inline_depth: i32,

    /// Debug information about this element.
    ///
    /// There can be several in case of inlining or optimization (child merged into their parent).
    ///
    /// The order in the list is first the parent, and then the removed children.
    pub debug: Vec<ElementDebugInfo>,
}

impl Spanned for Element {
    fn span(&self) -> crate::diagnostics::Span {
        self.debug.first().map(|n| n.node.span()).unwrap_or_default()
    }

    fn source_file(&self) -> Option<&crate::diagnostics::SourceFile> {
        self.debug.first().map(|n| &n.node.source_file)
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
            write!(f, "(base) ")?;
            if base.parent_element.upgrade().is_some() {
                pretty_print(f, &base.root_element.borrow(), indentation)?;
                return Ok(());
            }
        }
    }
    if e.is_component_placeholder {
        write!(f, "/* Component Placeholder */ ")?;
    }
    writeln!(f, "{} := {} {{  /* {} */", e.id, e.base_type, e.element_infos())?;
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
    for (name, ch) in &e.change_callbacks {
        for ex in &*ch.borrow() {
            indent!();
            write!(f, "changed {name} => ")?;
            expression_tree::pretty_print(f, ex)?;
            writeln!(f)?;
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
    if let Some(g) = &e.geometry_props {
        indent!();
        writeln!(f, "geometry {:?} ", g)?;
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

    /// True if this property is linked to another property
    pub is_linked: bool,
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
    pub model_data_id: SmolStr,
    pub index_id: SmolStr,
    /// A conditional element is just a for whose model is a boolean expression
    ///
    /// When this is true, the model is of type boolean instead of Model
    pub is_conditional_element: bool,
    /// When the for is the delegate of a ListView
    pub is_listview: Option<ListViewInfo>,
}

pub type ElementRc = Rc<RefCell<Element>>;
pub type ElementWeak = Weak<RefCell<Element>>;

impl Element {
    pub fn make_rc(self) -> ElementRc {
        let r = ElementRc::new(RefCell::new(self));
        let g = GeometryProps::new(&r);
        r.borrow_mut().geometry_props = Some(g);
        r
    }

    pub fn from_node(
        node: syntax_nodes::Element,
        id: SmolStr,
        parent_type: ElementType,
        component_child_insertion_point: &mut Option<ChildrenInsertionPoint>,
        is_legacy_syntax: bool,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) -> ElementRc {
        let base_type = if let Some(base_node) = node.QualifiedName() {
            let base = QualifiedTypeName::from_node(base_node.clone());
            let base_string = base.to_smolstr();
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
            node.CallbackDeclaration().for_each(|cb| {
                if parser::identifier_text(&cb.DeclaredIdentifier()).map_or(false, |s| s == "init")
                {
                    error_on(&cb, "an 'init' callback")
                }
            });
            node.CallbackConnection().for_each(|cb| {
                if parser::identifier_text(&cb).map_or(false, |s| s == "init") {
                    error_on(&cb, "an 'init' callback")
                }
            });

            ElementType::Global
        } else if parent_type != ElementType::Error {
            // This should normally never happen because the parser does not allow for this
            assert!(diag.has_errors());
            return ElementRc::default();
        } else {
            tr.empty_type()
        };
        // This isn't truly qualified yet, the enclosing component is added at the end of Component::from_node
        let qualified_id = (!id.is_empty()).then(|| id.clone());
        let type_name = base_type
            .type_name()
            .filter(|_| base_type != tr.empty_type())
            .unwrap_or_default()
            .to_string();
        let mut r = Element {
            id,
            base_type,
            debug: vec![ElementDebugInfo {
                qualified_id,
                type_name,
                node: node.clone(),
                layout: None,
                element_boundary: false,
            }],
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
                Type::Function { .. } => {
                    diag.push_error(
                        format!("Cannot declare property '{}' when a function with the same name exists", prop_name),
                        &prop_decl.DeclaredIdentifier().child_token(SyntaxKind::Identifier).unwrap(),
                    );
                    continue;
                }
                Type::Invalid => {} // Ok to proceed with a new declaration
                _ => {
                    diag.push_error(
                        format!("Cannot override property '{}'", unresolved_prop_name),
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
            let visibility = visibility.unwrap_or({
                if is_legacy_syntax {
                    PropertyVisibility::InOut
                } else {
                    PropertyVisibility::Private
                }
            });

            r.property_declarations.insert(
                prop_name.clone().into(),
                PropertyDeclaration {
                    property_type: prop_type,
                    node: Some(prop_decl.clone().into()),
                    visibility,
                    ..Default::default()
                },
            );

            if let Some(csn) = prop_decl.BindingExpression() {
                match r.bindings.entry(prop_name.clone().into()) {
                    Entry::Vacant(e) => {
                        e.insert(BindingExpression::new_uncompiled(csn.into()).into());
                    }
                    Entry::Occupied(_) => {
                        diag.push_error(
                            "Duplicated property binding".into(),
                            &prop_decl.DeclaredIdentifier(),
                        );
                    }
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

            let pure = Some(
                sig_decl.child_token(SyntaxKind::Identifier).map_or(false, |t| t.text() == "pure"),
            );

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
                        format!(
                            "Cannot declare callback '{existing_name}' when a {} with the same name exists",
                            if matches!(maybe_existing_prop_type, Type::Function { .. }) { "function" } else { "property" }
                        ),
                        &sig_decl.DeclaredIdentifier(),
                    );
                }
                continue;
            }

            if let Some(csn) = sig_decl.TwoWayBinding() {
                r.bindings
                    .insert(name.clone(), BindingExpression::new_uncompiled(csn.into()).into());
                r.property_declarations.insert(
                    name,
                    PropertyDeclaration {
                        property_type: Type::InferredCallback,
                        node: Some(sig_decl.into()),
                        visibility: PropertyVisibility::InOut,
                        pure,
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
                    property_type: Type::Callback(Rc::new(Function {
                        return_type,
                        args,
                        arg_names,
                    })),
                    node: Some(sig_decl.into()),
                    visibility: PropertyVisibility::InOut,
                    pure,
                    ..Default::default()
                },
            );
        }

        for func in node.Function() {
            let name =
                unwrap_or_continue!(parser::identifier_text(&func.DeclaredIdentifier()); diag);

            let PropertyLookupResult {
                resolved_name: existing_name,
                property_type: maybe_existing_prop_type,
                ..
            } = r.lookup_property(&name);
            if !matches!(maybe_existing_prop_type, Type::Invalid) {
                if matches!(maybe_existing_prop_type, Type::Callback { .. } | Type::Function { .. })
                {
                    diag.push_error(
                        format!("Cannot override '{}'", existing_name),
                        &func.DeclaredIdentifier(),
                    )
                } else {
                    diag.push_error(
                        format!("Cannot declare function '{}' when a property with the same name exists", existing_name),
                        &func.DeclaredIdentifier(),
                    );
                }
                continue;
            }

            let mut args = vec![];
            let mut arg_names = vec![];
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
            if r.bindings
                .insert(name.clone(), BindingExpression::new_uncompiled(func.clone().into()).into())
                .is_some()
            {
                assert!(diag.has_errors());
            }

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

            r.property_declarations.insert(
                name,
                PropertyDeclaration {
                    property_type: Type::Function(Rc::new(Function {
                        return_type,
                        args,
                        arg_names,
                    })),
                    node: Some(func.into()),
                    visibility,
                    pure,
                    ..Default::default()
                },
            );
        }

        for con_node in node.CallbackConnection() {
            let unresolved_name = unwrap_or_continue!(parser::identifier_text(&con_node); diag);
            let PropertyLookupResult { resolved_name, property_type, .. } =
                r.lookup_property(&unresolved_name);
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
            match r.bindings.entry(resolved_name.into()) {
                Entry::Vacant(e) => {
                    e.insert(BindingExpression::new_uncompiled(con_node.clone().into()).into());
                }
                Entry::Occupied(_) => diag.push_error(
                    "Duplicated callback".into(),
                    &con_node.child_token(SyntaxKind::Identifier).unwrap(),
                ),
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
                        if r.base_type == ElementType::Error {
                            continue;
                        };
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
                                .entry(lookup_result.resolved_name.into())
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

        for ch in node.PropertyChangedCallback() {
            let Some(prop) = parser::identifier_text(&ch.DeclaredIdentifier()) else { continue };
            let lookup_result = r.lookup_property(&prop);
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
            match r.change_callbacks.entry(prop) {
                Entry::Vacant(e) => {
                    e.insert(vec![handler].into());
                }
                Entry::Occupied(mut e) => {
                    diag.push_error(
                        format!("Duplicated change callback on '{}'", e.key()),
                        &ch.DeclaredIdentifier(),
                    );
                    e.get_mut().get_mut().push(handler);
                }
            }
        }

        let mut children_placeholder = None;
        let r = r.make_rc();

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
                if let Some((_, _, se)) = sub_child_insertion_point {
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
                if let Some((_, _, se)) = sub_child_insertion_point {
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
                    children_placeholder = Some((se.clone().into(), r.borrow().children.len()));
                }
            }
        }

        if let Some((children_placeholder, index)) = children_placeholder {
            if component_child_insertion_point.is_some() {
                diag.push_error(
                    "The @children placeholder can only appear once in an element hierarchy".into(),
                    &children_placeholder,
                )
            } else {
                *component_child_insertion_point = Some((r.clone(), index, children_placeholder));
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
                            .map(|(ne, ty)| {
                                if !ty.is_property_type() && !matches!(ty, Type::Invalid) {
                                    diag.push_error(
                                        format!("'{}' is not a property", **s.QualifiedName()),
                                        &s,
                                    );
                                }
                                (ne, Expression::Uncompiled(s.BindingExpression().into()), s)
                            })
                    })
                    .collect(),
            };
            for trs in state.Transition() {
                let mut t = Transition::from_node(trs, &r, tr, diag);
                t.state_id.clone_from(&s.id);
                r.borrow_mut().transitions.push(t);
            }
            r.borrow_mut().states.push(s);
        }

        for ts in node.Transitions() {
            if !is_legacy_syntax {
                diag.push_error("'transitions' block are no longer supported. Use 'in {...}' and 'out {...}' directly in the state definition".into(), &ts);
            }
            for trs in ts.Transition() {
                let trans = Transition::from_node(trs, &r, tr, diag);
                r.borrow_mut().transitions.push(trans);
            }
        }

        if r.borrow().base_type.to_smolstr() == "ListView" {
            let mut seen_for = false;
            for se in node.children() {
                if se.kind() == SyntaxKind::RepeatedElement && !seen_for {
                    seen_for = true;
                } else if matches!(
                    se.kind(),
                    SyntaxKind::SubElement
                        | SyntaxKind::ConditionalElement
                        | SyntaxKind::RepeatedElement
                        | SyntaxKind::ChildrenPlaceholder
                ) {
                    diag.push_error("A ListView can just have a single 'for' as children. Anything else is not supported".into(), &se)
                }
            }
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
        let mut id = parser::identifier_text(&node).unwrap_or_default();
        if matches!(id.as_ref(), "parent" | "self" | "root") {
            diag.push_error(
                format!("'{}' is a reserved id", id),
                &node.child_token(SyntaxKind::Identifier).unwrap(),
            );
            id = SmolStr::default();
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
                viewport_y: NamedReference::new(parent, SmolStr::new_static("viewport-y")),
                viewport_height: NamedReference::new(
                    parent,
                    SmolStr::new_static("viewport-height"),
                ),
                viewport_width: NamedReference::new(parent, SmolStr::new_static("viewport-width")),
                listview_height: NamedReference::new(parent, SmolStr::new_static("visible-height")),
                listview_width: NamedReference::new(parent, SmolStr::new_static("visible-width")),
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
            model_data_id: SmolStr::default(),
            index_id: SmolStr::default(),
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
                r.is_in_direct_base = r.is_local_to_component;
                r.is_local_to_component = false;
                r
            },
            |p| PropertyLookupResult {
                resolved_name: name.into(),
                property_type: p.property_type.clone(),
                property_visibility: p.visibility,
                declared_pure: p.pure,
                is_local_to_component: true,
                is_in_direct_base: false,
            },
        )
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
                                diag.push_error(if self.base_type.to_smolstr() == "Empty" {
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

            if *lookup_result.resolved_name != *unresolved_name {
                diag.push_property_deprecation_warning(
                    &unresolved_name,
                    &lookup_result.resolved_name,
                    &name_token,
                );
            }

            match self.bindings.entry(lookup_result.resolved_name.into()) {
                Entry::Occupied(_) => {
                    diag.push_error("Duplicated property binding".into(), &name_token);
                }
                Entry::Vacant(entry) => {
                    entry.insert(BindingExpression::new_uncompiled(b).into());
                }
            };
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
    pub fn original_name(&self) -> SmolStr {
        self.debug
            .first()
            .and_then(|n| n.node.child_token(parser::SyntaxKind::Identifier))
            .map(|n| n.to_smolstr())
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
        property_name: SmolStr,
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
        if self.repeated.is_some() || self.is_component_placeholder {
            None
        } else if let ElementType::Component(sub_component) = &self.base_type {
            Some(sub_component)
        } else {
            None
        }
    }

    pub fn element_infos(&self) -> String {
        let mut debug_infos = self.debug.clone();
        let mut base = self.base_type.clone();
        while let ElementType::Component(b) = base {
            let elem = b.root_element.borrow();
            base = elem.base_type.clone();
            debug_infos.extend(elem.debug.iter().cloned());
        }

        let (infos, _, _) = debug_infos.into_iter().fold(
            (String::new(), false, true),
            |(mut infos, elem_boundary, first), debug_info| {
                if elem_boundary {
                    infos.push('/');
                } else if !first {
                    infos.push(';');
                }

                infos.push_str(&debug_info.encoded_element_info());
                (infos, debug_info.element_boundary, false)
            },
        );
        infos
    }
}

/// Apply default property values defined in `builtins.slint` to the element.
fn apply_default_type_properties(element: &mut Element) {
    // Apply default property values on top:
    if let ElementType::Builtin(builtin_base) = &element.base_type {
        for (prop, info) in &builtin_base.properties {
            if let BuiltinPropertyDefault::Expr(expr) = &info.default_value {
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

        if prop_type == Type::Invalid && tr.lookup_element(&qualified_type.to_smolstr()).is_err() {
            diag.push_error(format!("Unknown type '{}'", qualified_type), &qualified_type_node);
        } else if !prop_type.is_property_type() {
            diag.push_error(
                format!("'{}' is not a valid type", qualified_type),
                &qualified_type_node,
            );
        }
        prop_type
    } else if let Some(object_node) = node.ObjectType() {
        type_struct_from_node(object_node, diag, tr, None, None)
    } else if let Some(array_node) = node.ArrayType() {
        Type::Array(Rc::new(type_from_node(array_node.Type(), diag, tr)))
    } else {
        assert!(diag.has_errors());
        Type::Invalid
    }
}

/// Create a [`Type::Struct`] from a [`syntax_nodes::ObjectType`]
pub fn type_struct_from_node(
    object_node: syntax_nodes::ObjectType,
    diag: &mut BuildDiagnostics,
    tr: &TypeRegister,
    rust_attributes: Option<Vec<SmolStr>>,
    name: Option<SmolStr>,
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
    Type::Struct(Rc::new(Struct { fields, name, node: Some(object_node), rust_attributes }))
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
            Element { id: "".into(), base_type: anim_type, ..Default::default() };
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
    pub members: Vec<SmolStr>,
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
    r: &ElementRc,
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
                NamedReference::new(r, lookup_result.resolved_name.to_smolstr()),
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
                    NamedReference::new(&element, lookup_result.resolved_name.to_smolstr()),
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

/// Same as [`recurse_elem`] but include the elements from sub_components
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
            vis(&mut expr.borrow_mut(), Some(name.as_str()), &|| {
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

    let repeated = elem
        .borrow_mut()
        .repeated
        .as_mut()
        .map(|r| (std::mem::take(&mut r.model), r.is_conditional_element));
    if let Some((mut model, is_cond)) = repeated {
        vis(&mut model, None, &|| if is_cond { Type::Bool } else { Type::Model });
        elem.borrow_mut().repeated.as_mut().unwrap().model = model;
    }
    visit_element_expressions_simple(elem, &mut vis);

    for expr in elem.borrow().change_callbacks.values() {
        for expr in expr.borrow_mut().iter_mut() {
            vis(expr, Some("$change callback$"), &|| Type::Void);
        }
    }

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

    let component = elem.borrow().enclosing_component.upgrade().unwrap();
    if Rc::ptr_eq(&component.root_element, elem) {
        for e in component.init_code.borrow_mut().iter_mut() {
            vis(e, None, &|| Type::Void);
        }
    }
}

pub fn visit_named_references_in_expression(
    expr: &mut Expression,
    vis: &mut impl FnMut(&mut NamedReference),
) {
    expr.visit_mut(|sub| visit_named_references_in_expression(sub, vis));
    match expr {
        Expression::PropertyReference(r)
        | Expression::CallbackReference(r, _)
        | Expression::FunctionReference(r, _) => vis(r),
        Expression::LayoutCacheAccess { layout_cache_prop, .. } => vis(layout_cache_prop),
        Expression::SolveLayout(l, _) => l.visit_named_references(vis),
        Expression::ComputeLayoutInfo(l, _) => l.visit_named_references(vis),
        // This is not really a named reference, but the result is the same, it need to be updated
        // FIXME: this should probably be lowered into a PropertyReference
        Expression::RepeaterModelReference { element }
        | Expression::RepeaterIndexReference { element } => {
            // FIXME: this is questionable
            let mut nc =
                NamedReference::new(&element.upgrade().unwrap(), SmolStr::new_static("$model"));
            vis(&mut nc);
            debug_assert!(nc.element().borrow().repeated.is_some());
            *element = Rc::downgrade(&nc.element());
        }
        _ => {}
    }
}

/// Visit all the named reference in an element
/// But does not recurse in sub-elements. (unlike [`visit_all_named_references`] which recurse)
pub fn visit_all_named_references_in_element(
    elem: &ElementRc,
    mut vis: impl FnMut(&mut NamedReference),
) {
    visit_element_expressions(elem, |expr, _, _| {
        visit_named_references_in_expression(expr, &mut vis)
    });
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
    let mut debug = std::mem::take(&mut elem.borrow_mut().debug);
    for d in debug.iter_mut() {
        if let Some(l) = d.layout.as_mut() {
            l.visit_named_references(&mut vis)
        }
    }
    elem.borrow_mut().debug = debug;

    let mut accessibility_props = std::mem::take(&mut elem.borrow_mut().accessibility_props);
    accessibility_props.0.iter_mut().for_each(|(_, x)| vis(x));
    elem.borrow_mut().accessibility_props = accessibility_props;

    let geometry_props = elem.borrow_mut().geometry_props.take();
    if let Some(mut geometry_props) = geometry_props {
        vis(&mut geometry_props.x);
        vis(&mut geometry_props.y);
        vis(&mut geometry_props.width);
        vis(&mut geometry_props.height);
        elem.borrow_mut().geometry_props = Some(geometry_props);
    }

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
                compo.timers.borrow_mut().iter_mut().for_each(|t| {
                    vis(&mut t.interval);
                    vis(&mut t.triggered);
                    vis(&mut t.running);
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
    pub id: SmolStr,
    pub condition: Option<Expression>,
    pub property_changes: Vec<(NamedReference, Expression, syntax_nodes::StatePropertyChange)>,
}

#[derive(Debug, Clone)]
pub struct Transition {
    /// false for 'to', true for 'out'
    pub is_out: bool,
    pub state_id: SmolStr,
    pub property_animations: Vec<(NamedReference, SourceLocation, ElementRc)>,
    pub node: syntax_nodes::Transition,
}

impl Transition {
    fn from_node(
        trs: syntax_nodes::Transition,
        r: &ElementRc,
        tr: &TypeRegister,
        diag: &mut BuildDiagnostics,
    ) -> Transition {
        if let Some(star) = trs.child_token(SyntaxKind::Star) {
            diag.push_error("catch-all not yet implemented".into(), &star);
        };
        Transition {
            is_out: parser::identifier_text(&trs).unwrap_or_default() == "out",
            state_id: trs
                .DeclaredIdentifier()
                .and_then(|x| parser::identifier_text(&x))
                .unwrap_or_default(),
            property_animations: trs
                .PropertyAnimation()
                .flat_map(|pa| pa.QualifiedName().map(move |qn| (pa.clone(), qn)))
                .filter_map(|(pa, qn)| {
                    lookup_property_from_qualified_name_for_state(qn.clone(), r, diag).and_then(
                        |(ne, prop_type)| {
                            animation_element_from_node(&pa, &qn, prop_type, diag, tr)
                                .map(|anim_element| (ne, qn.to_source_location(), anim_element))
                        },
                    )
                })
                .collect(),
            node: trs.clone(),
        }
    }
}

#[derive(Clone, Debug, derive_more::Deref)]
pub struct ExportedName {
    #[deref]
    pub name: SmolStr, // normalized
    pub name_ident: SyntaxNode,
}

impl ExportedName {
    pub fn original_name(&self) -> SmolStr {
        self.name_ident
            .child_token(parser::SyntaxKind::Identifier)
            .map(|n| n.to_smolstr())
            .unwrap_or_else(|| self.name.clone())
    }

    pub fn from_export_specifier(
        export_specifier: &syntax_nodes::ExportSpecifier,
    ) -> (SmolStr, ExportedName) {
        let internal_name =
            parser::identifier_text(&export_specifier.ExportIdentifier()).unwrap_or_default();

        let (name, name_ident): (SmolStr, SyntaxNode) = export_specifier
            .ExportName()
            .and_then(|ident| {
                parser::identifier_text(&ident).map(|text| (text, ident.clone().into()))
            })
            .unwrap_or_else(|| (internal_name.clone(), export_specifier.ExportIdentifier().into()));
        (internal_name, ExportedName { name, name_ident })
    }
}

#[derive(Default, Debug, derive_more::Deref)]
pub struct Exports {
    #[deref]
    components_or_types: Vec<(ExportedName, Either<Rc<Component>, Type>)>,
}

impl Exports {
    pub fn from_node(
        doc: &syntax_nodes::Document,
        inner_components: &[Rc<Component>],
        type_registry: &TypeRegister,
        diag: &mut BuildDiagnostics,
    ) -> Self {
        let resolve_export_to_inner_component_or_import =
            |internal_name: &str, internal_name_node: &dyn Spanned, diag: &mut BuildDiagnostics| {
                if let Ok(ElementType::Component(c)) = type_registry.lookup_element(internal_name) {
                    Some(Either::Left(c))
                } else if let ty @ Type::Struct { .. } | ty @ Type::Enumeration(_) =
                    type_registry.lookup(internal_name)
                {
                    Some(Either::Right(ty))
                } else if type_registry.lookup_element(internal_name).is_ok()
                    || type_registry.lookup(internal_name) != Type::Invalid
                {
                    diag.push_error(
                        format!("Cannot export '{}' because it is not a component", internal_name,),
                        internal_name_node,
                    );
                    None
                } else {
                    diag.push_error(format!("'{}' not found", internal_name,), internal_name_node);
                    None
                }
            };

        let mut sorted_exports_with_duplicates: Vec<(ExportedName, _)> = Vec::new();

        let mut extend_exports =
            |it: &mut dyn Iterator<Item = (ExportedName, Either<Rc<Component>, Type>)>| {
                for (name, compo_or_type) in it {
                    let pos = sorted_exports_with_duplicates
                        .partition_point(|(existing_name, _)| existing_name.name <= name.name);
                    sorted_exports_with_duplicates.insert(pos, (name, compo_or_type));
                }
            };

        extend_exports(
            &mut doc
                .ExportsList()
                // re-export are handled in the TypeLoader::load_dependencies_recursively_impl
                .filter(|exports| exports.ExportModule().is_none())
                .flat_map(|exports| exports.ExportSpecifier())
                .filter_map(|export_specifier| {
                    let (internal_name, exported_name) =
                        ExportedName::from_export_specifier(&export_specifier);
                    Some((
                        exported_name,
                        resolve_export_to_inner_component_or_import(
                            &internal_name,
                            &export_specifier.ExportIdentifier(),
                            diag,
                        )?,
                    ))
                }),
        );

        extend_exports(&mut doc.ExportsList().flat_map(|exports| exports.Component()).filter_map(
            |component| {
                let name_ident: SyntaxNode = component.DeclaredIdentifier().into();
                let name =
                    parser::identifier_text(&component.DeclaredIdentifier()).unwrap_or_else(|| {
                        debug_assert!(diag.has_errors());
                        SmolStr::default()
                    });

                let compo_or_type =
                    resolve_export_to_inner_component_or_import(&name, &name_ident, diag)?;

                Some((ExportedName { name, name_ident }, compo_or_type))
            },
        ));

        extend_exports(
            &mut doc
                .ExportsList()
                .flat_map(|exports| {
                    exports
                        .StructDeclaration()
                        .map(|st| st.DeclaredIdentifier())
                        .chain(exports.EnumDeclaration().map(|en| en.DeclaredIdentifier()))
                })
                .filter_map(|name_ident| {
                    let name = parser::identifier_text(&name_ident).unwrap_or_else(|| {
                        debug_assert!(diag.has_errors());
                        SmolStr::default()
                    });

                    let name_ident = name_ident.into();

                    let compo_or_type =
                        resolve_export_to_inner_component_or_import(&name, &name_ident, diag)?;

                    Some((ExportedName { name, name_ident }, compo_or_type))
                }),
        );

        let mut sorted_deduped_exports = Vec::with_capacity(sorted_exports_with_duplicates.len());
        let mut it = sorted_exports_with_duplicates.into_iter().peekable();
        while let Some((exported_name, compo_or_type)) = it.next() {
            let mut warning_issued_on_first_occurrence = false;

            // Skip over duplicates and issue warnings
            while it.peek().map(|(name, _)| &name.name) == Some(&exported_name.name) {
                let message = format!("Duplicated export '{}'", exported_name.name);

                if !warning_issued_on_first_occurrence {
                    diag.push_error(message.clone(), &exported_name.name_ident);
                    warning_issued_on_first_occurrence = true;
                }

                let duplicate_loc = it.next().unwrap().0.name_ident;
                diag.push_error(message.clone(), &duplicate_loc);
            }

            sorted_deduped_exports.push((exported_name, compo_or_type));
        }

        if let Some(last_compo) = inner_components.last() {
            let name = last_compo.id.clone();
            if last_compo.is_global() {
                if sorted_deduped_exports.is_empty() {
                    diag.push_warning("Global singleton is implicitly marked for export. This is deprecated and it should be explicitly exported".into(), &last_compo.node);
                    sorted_deduped_exports.push((
                        ExportedName { name, name_ident: doc.clone().into() },
                        Either::Left(last_compo.clone()),
                    ))
                }
            } else if !sorted_deduped_exports
                .iter()
                .any(|e| e.1.as_ref().left().is_some_and(|c| !c.is_global()))
            {
                diag.push_warning("Component is implicitly marked for export. This is deprecated and it should be explicitly exported".into(), &last_compo.node);
                let insert_pos = sorted_deduped_exports
                    .partition_point(|(existing_export, _)| existing_export.name <= name);
                sorted_deduped_exports.insert(
                    insert_pos,
                    (
                        ExportedName { name, name_ident: doc.clone().into() },
                        Either::Left(last_compo.clone()),
                    ),
                )
            }
        }
        Self { components_or_types: sorted_deduped_exports }
    }

    pub fn add_reexports(
        &mut self,
        other_exports: impl IntoIterator<Item = (ExportedName, Either<Rc<Component>, Type>)>,
        diag: &mut BuildDiagnostics,
    ) {
        for export in other_exports {
            match self.components_or_types.binary_search_by(|entry| entry.0.cmp(&export.0)) {
                Ok(_) => {
                    diag.push_warning(
                        format!(
                            "'{}' is already exported in this file; it will not be re-exported",
                            &*export.0
                        ),
                        &export.0.name_ident,
                    );
                }
                Err(insert_pos) => {
                    self.components_or_types.insert(insert_pos, export);
                }
            }
        }
    }

    pub fn find(&self, name: &str) -> Option<Either<Rc<Component>, Type>> {
        self.components_or_types
            .binary_search_by(|(exported_name, _)| exported_name.as_str().cmp(name))
            .ok()
            .map(|index| self.components_or_types[index].1.clone())
    }

    pub fn retain(
        &mut self,
        func: impl FnMut(&mut (ExportedName, Either<Rc<Component>, Type>)) -> bool,
    ) {
        self.components_or_types.retain_mut(func)
    }

    pub(crate) fn snapshot(&self, snapshotter: &mut crate::typeloader::Snapshotter) -> Self {
        let components_or_types = self
            .components_or_types
            .iter()
            .map(|(en, either)| {
                let en = en.clone();
                let either = match either {
                    itertools::Either::Left(l) => itertools::Either::Left({
                        Weak::upgrade(&snapshotter.use_component(l))
                            .expect("Component should cleanly upgrade here")
                    }),
                    itertools::Either::Right(r) => itertools::Either::Right(r.clone()),
                };
                (en, either)
            })
            .collect();

        Self { components_or_types }
    }
}

impl std::iter::IntoIterator for Exports {
    type Item = (ExportedName, Either<Rc<Component>, Type>);

    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.components_or_types.into_iter()
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
    let layout_info_prop = old_root.borrow().layout_info_prop.clone().or_else(|| {
        // generate the layout_info_prop that forward to the implicit layout for that item
        let li_v = crate::layout::create_new_prop(
            &new_root,
            SmolStr::new_static("layoutinfo-v"),
            crate::typeregister::layout_info_type(),
        );
        let li_h = crate::layout::create_new_prop(
            &new_root,
            SmolStr::new_static("layoutinfo-h"),
            crate::typeregister::layout_info_type(),
        );
        let expr_h = crate::layout::implicit_layout_info_call(old_root, Orientation::Horizontal);
        let expr_v = crate::layout::implicit_layout_info_call(old_root, Orientation::Vertical);
        let expr_v =
            BindingExpression::new_with_span(expr_v, old_root.borrow().to_source_location());
        li_v.element().borrow_mut().bindings.insert(li_v.name().clone(), expr_v.into());
        let expr_h =
            BindingExpression::new_with_span(expr_h, old_root.borrow().to_source_location());
        li_h.element().borrow_mut().bindings.insert(li_h.name().clone(), expr_h.into());
        Some((li_h.clone(), li_v.clone()))
    });
    new_root.borrow_mut().layout_info_prop = layout_info_prop;

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
    let mut injected_parent_mut = injected_parent.borrow_mut();
    injected_parent_mut.bindings.insert(
        "z".into(),
        RefCell::new(BindingExpression::new_two_way(NamedReference::new(
            old_elem,
            SmolStr::new_static("z"),
        ))),
    );
    // (should be removed by const propagation in the llr)
    injected_parent_mut.property_declarations.insert(
        "dummy".into(),
        PropertyDeclaration { property_type: Type::LogicalLength, ..Default::default() },
    );
    let mut old_elem_mut = old_elem.borrow_mut();
    injected_parent_mut.default_fill_parent = std::mem::take(&mut old_elem_mut.default_fill_parent);
    injected_parent_mut.geometry_props.clone_from(&old_elem_mut.geometry_props);
    drop(injected_parent_mut);
    old_elem_mut.geometry_props.as_mut().unwrap().x =
        NamedReference::new(injected_parent, SmolStr::new_static("dummy"));
    old_elem_mut.geometry_props.as_mut().unwrap().y =
        NamedReference::new(injected_parent, SmolStr::new_static("dummy"));
}
