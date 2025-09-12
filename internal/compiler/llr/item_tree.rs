// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{EvaluationContext, Expression, ParentCtx};
use crate::langtype::{NativeClass, Type};
use smol_str::SmolStr;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::num::NonZeroUsize;
use std::rc::Rc;
use typed_index_collections::TiVec;

#[derive(
    Debug, Clone, Copy, derive_more::Into, derive_more::From, Hash, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct PropertyIdx(usize);
#[derive(Debug, Clone, Copy, derive_more::Into, derive_more::From, Hash, PartialEq, Eq)]
pub struct FunctionIdx(usize);
#[derive(Debug, Clone, Copy, derive_more::Into, derive_more::From)]
pub struct SubComponentIdx(usize);
#[derive(Debug, Clone, Copy, derive_more::Into, derive_more::From, Hash, PartialEq, Eq)]
pub struct GlobalIdx(usize);
#[derive(Debug, Clone, Copy, derive_more::Into, derive_more::From, Hash, PartialEq, Eq)]
pub struct SubComponentInstanceIdx(usize);
#[derive(Debug, Clone, Copy, derive_more::Into, derive_more::From, Hash, PartialEq, Eq)]
pub struct ItemInstanceIdx(usize);
#[derive(Debug, Clone, Copy, derive_more::Into, derive_more::From, Hash, PartialEq, Eq)]
pub struct RepeatedElementIdx(usize);

#[derive(Debug, Clone, derive_more::Deref)]
pub struct MutExpression(RefCell<Expression>);

impl From<Expression> for MutExpression {
    fn from(e: Expression) -> Self {
        Self(e.into())
    }
}

impl MutExpression {
    pub fn ty(&self, ctx: &dyn super::TypeResolutionContext) -> Type {
        self.0.borrow().ty(ctx)
    }
}

#[derive(Debug, Clone)]
pub enum Animation {
    /// The expression is a Struct with the animation fields
    Static(Expression),
    Transition(Expression),
}

#[derive(Debug, Clone)]
pub struct BindingExpression {
    pub expression: MutExpression,
    pub animation: Option<Animation>,
    /// When true, we can initialize the property with `set` otherwise, `set_binding` must be used
    pub is_constant: bool,
    /// When true, the expression is a "state binding".  Despite the type of the expression being a integer
    /// the property is of type StateInfo and the `set_state_binding` need to be used on the property
    pub is_state_info: bool,

    /// The amount of time this binding is used
    /// This property is only valid after the [`count_property_use`](super::optim_passes::count_property_use) pass
    pub use_count: Cell<usize>,
}

#[derive(Debug)]
pub struct GlobalComponent {
    pub name: SmolStr,
    pub properties: TiVec<PropertyIdx, Property>,
    pub functions: TiVec<FunctionIdx, Function>,
    /// One entry per property
    pub init_values: TiVec<PropertyIdx, Option<BindingExpression>>,
    // maps property to its changed callback
    pub change_callbacks: BTreeMap<PropertyIdx, MutExpression>,
    pub const_properties: TiVec<PropertyIdx, bool>,
    pub public_properties: PublicProperties,
    pub private_properties: PrivateProperties,
    /// true if we should expose the global in the generated API
    pub exported: bool,
    /// The extra names under which this component should be accessible
    /// if it is exported several time.
    pub aliases: Vec<SmolStr>,
    /// True when this is a built-in global that does not need to be generated
    pub is_builtin: bool,
    /// True if this component is imported from an external library
    pub from_library: bool,
    /// Analysis for each properties
    pub prop_analysis: TiVec<PropertyIdx, crate::object_tree::PropertyAnalysis>,
}

impl GlobalComponent {
    pub fn must_generate(&self) -> bool {
        !self.is_builtin
            && !self.from_library
            && (self.exported
                || !self.functions.is_empty()
                || self.properties.iter().any(|p| p.use_count.get() > 0))
    }
}

/// a Reference to a property, in the context of a SubComponent
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum PropertyReference {
    /// A property relative to this SubComponent
    Local { sub_component_path: Vec<SubComponentInstanceIdx>, property_index: PropertyIdx },
    /// A property in a Native item
    InNativeItem {
        sub_component_path: Vec<SubComponentInstanceIdx>,
        item_index: ItemInstanceIdx,
        prop_name: String,
    },
    /// The properties is a property relative to a parent ItemTree (`level` level deep)
    InParent { level: NonZeroUsize, parent_reference: Box<PropertyReference> },
    /// The property within a GlobalComponent
    Global { global_index: GlobalIdx, property_index: PropertyIdx },

    /// A function in a sub component.
    Function { sub_component_path: Vec<SubComponentInstanceIdx>, function_index: FunctionIdx },
    /// A function in a global.
    GlobalFunction { global_index: GlobalIdx, function_index: FunctionIdx },
}

#[derive(Debug, Default)]
pub struct Property {
    pub name: SmolStr,
    pub ty: Type,
    /// The amount of time this property is used of another property
    /// This property is only valid after the [`count_property_use`](super::optim_passes::count_property_use) pass
    pub use_count: Cell<usize>,
}

#[derive(Debug)]
pub struct Function {
    pub name: SmolStr,
    pub ret_ty: Type,
    pub args: Vec<Type>,
    pub code: Expression,
}

#[derive(Debug, Clone)]
/// The property references might be either in the parent context, or in the
/// repeated's component context
pub struct ListViewInfo {
    pub viewport_y: PropertyReference,
    pub viewport_height: PropertyReference,
    pub viewport_width: PropertyReference,
    /// The ListView's inner visible height (not counting eventual scrollbar)
    pub listview_height: PropertyReference,
    /// The ListView's inner visible width (not counting eventual scrollbar)
    pub listview_width: PropertyReference,

    // In the repeated component context
    pub prop_y: PropertyReference,
    // In the repeated component context
    pub prop_height: PropertyReference,
}

#[derive(Debug)]
pub struct RepeatedElement {
    pub model: MutExpression,
    /// Within the sub_tree's root component. None for `if`
    pub index_prop: Option<PropertyIdx>,
    /// Within the sub_tree's root component. None for `if`
    pub data_prop: Option<PropertyIdx>,
    pub sub_tree: ItemTree,
    /// The index of the item node in the parent tree
    pub index_in_tree: u32,

    pub listview: Option<ListViewInfo>,

    /// Access through this in case of the element being a `is_component_placeholder`
    pub container_item_index: Option<ItemInstanceIdx>,
}

#[derive(Debug)]
pub struct ComponentContainerElement {
    /// The index of the `ComponentContainer` in the enclosing components `item_tree` array
    pub component_container_item_tree_index: u32,
    /// The index of the `ComponentContainer` item in the enclosing components `items` array
    pub component_container_items_index: ItemInstanceIdx,
    /// The index to a dynamic tree node where the component is supposed to be embedded at
    pub component_placeholder_item_tree_index: u32,
}

pub struct Item {
    pub ty: Rc<NativeClass>,
    pub name: SmolStr,
    /// Index in the item tree array
    pub index_in_tree: u32,
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Item")
            .field("ty", &self.ty.class_name)
            .field("name", &self.name)
            .field("index_in_tree", &self.index_in_tree)
            .finish()
    }
}

#[derive(Debug)]
pub struct TreeNode {
    pub sub_component_path: Vec<SubComponentInstanceIdx>,
    /// Either an index in the items, or the local dynamic index for repeater or component container
    pub item_index: itertools::Either<ItemInstanceIdx, u32>,
    pub children: Vec<TreeNode>,
    pub is_accessible: bool,
}

impl TreeNode {
    fn children_count(&self) -> usize {
        let mut count = self.children.len();
        for c in &self.children {
            count += c.children_count();
        }
        count
    }

    /// Visit this, and the children.
    /// `children_offset` must be set to `1` for the root
    pub fn visit_in_array(
        &self,
        visitor: &mut dyn FnMut(
            &TreeNode,
            /*children_offset: */ usize,
            /*parent_index: */ usize,
        ),
    ) {
        visitor(self, 1, 0);
        visit_in_array_recursive(self, 1, 0, visitor);

        fn visit_in_array_recursive(
            node: &TreeNode,
            children_offset: usize,
            current_index: usize,
            visitor: &mut dyn FnMut(&TreeNode, usize, usize),
        ) {
            let mut offset = children_offset + node.children.len();
            for c in &node.children {
                visitor(c, offset, current_index);
                offset += c.children_count();
            }

            let mut offset = children_offset + node.children.len();
            for (i, c) in node.children.iter().enumerate() {
                visit_in_array_recursive(c, offset, children_offset + i, visitor);
                offset += c.children_count();
            }
        }
    }
}

#[derive(Debug)]
pub struct SubComponent {
    pub name: SmolStr,
    pub properties: TiVec<PropertyIdx, Property>,
    pub functions: TiVec<FunctionIdx, Function>,
    pub items: TiVec<ItemInstanceIdx, Item>,
    pub repeated: TiVec<RepeatedElementIdx, RepeatedElement>,
    pub component_containers: Vec<ComponentContainerElement>,
    pub popup_windows: Vec<PopupWindow>,
    /// The MenuItem trees. The index is stored in a Expression::NumberLiteral in the arguments of BuiltinFunction::ShowPopupMenu and BuiltinFunction::SetupNativeMenuBar
    pub menu_item_trees: Vec<ItemTree>,
    pub timers: Vec<Timer>,
    pub sub_components: TiVec<SubComponentInstanceIdx, SubComponentInstance>,
    /// The initial value or binding for properties.
    /// This is ordered in the order they must be set.
    pub property_init: Vec<(PropertyReference, BindingExpression)>,
    pub change_callbacks: Vec<(PropertyReference, MutExpression)>,
    /// The animation for properties which are animated
    pub animations: HashMap<PropertyReference, Expression>,
    pub two_way_bindings: Vec<(PropertyReference, PropertyReference)>,
    pub const_properties: Vec<PropertyReference>,
    /// Code that is run in the sub component constructor, after property initializations
    pub init_code: Vec<MutExpression>,

    /// For each node, an expression that returns a `{x: length, y: length, width: length, height: length}`
    pub geometries: Vec<Option<MutExpression>>,

    pub layout_info_h: MutExpression,
    pub layout_info_v: MutExpression,

    /// Maps (item_index, property) to an expression
    pub accessible_prop: BTreeMap<(u32, String), MutExpression>,

    /// Maps item index to a list of encoded element infos of the element  (type name, qualified ids).
    pub element_infos: BTreeMap<u32, String>,

    pub prop_analysis: HashMap<PropertyReference, PropAnalysis>,
}

#[derive(Debug)]
pub struct PopupWindow {
    pub item_tree: ItemTree,
    pub position: MutExpression,
}

#[derive(Debug)]
pub struct PopupMenu {
    pub item_tree: ItemTree,
    pub sub_menu: PropertyReference,
    pub activated: PropertyReference,
    pub close: PropertyReference,
    pub entries: PropertyReference,
}

#[derive(Debug)]
pub struct Timer {
    pub interval: MutExpression,
    pub running: MutExpression,
    pub triggered: MutExpression,
}

#[derive(Debug, Clone)]
pub struct PropAnalysis {
    /// Index in SubComponent::property_init for this property
    pub property_init: Option<usize>,
    pub analysis: crate::object_tree::PropertyAnalysis,
}

impl SubComponent {
    /// total count of repeater, including in sub components
    pub fn repeater_count(&self, cu: &CompilationUnit) -> u32 {
        let mut count = (self.repeated.len() + self.component_containers.len()) as u32;
        for x in self.sub_components.iter() {
            count += cu.sub_components[x.ty].repeater_count(cu);
        }
        count
    }

    /// total count of items, including in sub components
    pub fn child_item_count(&self, cu: &CompilationUnit) -> u32 {
        let mut count = self.items.len() as u32;
        for x in self.sub_components.iter() {
            count += cu.sub_components[x.ty].child_item_count(cu);
        }
        count
    }

    /// Return if a local property is used. (unused property shouldn't be generated)
    pub fn prop_used(&self, prop: &PropertyReference, cu: &CompilationUnit) -> bool {
        if let PropertyReference::Local { property_index, sub_component_path } = prop {
            let mut sc = self;
            for i in sub_component_path {
                sc = &cu.sub_components[sc.sub_components[*i].ty];
            }
            if sc.properties[*property_index].use_count.get() == 0 {
                return false;
            }
        }
        true
    }
}

#[derive(Debug)]
pub struct SubComponentInstance {
    pub ty: SubComponentIdx,
    pub name: SmolStr,
    pub index_in_tree: u32,
    pub index_of_first_child_in_tree: u32,
    pub repeater_offset: u32,
}

#[derive(Debug)]
pub struct ItemTree {
    pub root: SubComponentIdx,
    pub tree: TreeNode,
    /// This tree has a parent. e.g: it is a Repeater or a PopupWindow whose property can access
    /// the parent ItemTree.
    /// The String is the type of the parent ItemTree
    pub parent_context: Option<SmolStr>,
}

#[derive(Debug)]
pub struct PublicComponent {
    pub public_properties: PublicProperties,
    pub private_properties: PrivateProperties,
    pub item_tree: ItemTree,
    pub name: SmolStr,
}

#[derive(Debug)]
pub struct CompilationUnit {
    pub public_components: Vec<PublicComponent>,
    /// Storage for all sub-components
    pub sub_components: TiVec<SubComponentIdx, SubComponent>,
    /// The sub-components that are not item-tree root
    pub used_sub_components: Vec<SubComponentIdx>,
    pub globals: TiVec<GlobalIdx, GlobalComponent>,
    pub popup_menu: Option<PopupMenu>,
    pub has_debug_info: bool,
    #[cfg(feature = "bundle-translations")]
    pub translations: Option<crate::translations::Translations>,
}

impl CompilationUnit {
    pub fn for_each_sub_components<'a>(
        &'a self,
        visitor: &mut dyn FnMut(&'a SubComponent, &EvaluationContext<'_>),
    ) {
        fn visit_component<'a>(
            root: &'a CompilationUnit,
            c: SubComponentIdx,
            visitor: &mut dyn FnMut(&'a SubComponent, &EvaluationContext<'_>),
            parent: Option<ParentCtx<'_>>,
        ) {
            let ctx = EvaluationContext::new_sub_component(root, c, (), parent);
            let sc = &root.sub_components[c];
            visitor(sc, &ctx);
            for (idx, r) in sc.repeated.iter_enumerated() {
                visit_component(
                    root,
                    r.sub_tree.root,
                    visitor,
                    Some(ParentCtx::new(&ctx, Some(idx))),
                );
            }
            for popup in &sc.popup_windows {
                visit_component(
                    root,
                    popup.item_tree.root,
                    visitor,
                    Some(ParentCtx::new(&ctx, None)),
                );
            }
            for menu_tree in &sc.menu_item_trees {
                visit_component(root, menu_tree.root, visitor, Some(ParentCtx::new(&ctx, None)));
            }
        }
        for c in &self.used_sub_components {
            visit_component(self, *c, visitor, None);
        }
        for p in &self.public_components {
            visit_component(self, p.item_tree.root, visitor, None);
        }
        if let Some(p) = &self.popup_menu {
            visit_component(self, p.item_tree.root, visitor, None);
        }
    }

    pub fn for_each_expression<'a>(
        &'a self,
        visitor: &mut dyn FnMut(&'a super::MutExpression, &EvaluationContext<'_>),
    ) {
        self.for_each_sub_components(&mut |sc, ctx| {
            for e in &sc.init_code {
                visitor(e, ctx);
            }
            for (_, e) in &sc.property_init {
                visitor(&e.expression, ctx);
            }
            visitor(&sc.layout_info_h, ctx);
            visitor(&sc.layout_info_v, ctx);
            for e in sc.accessible_prop.values() {
                visitor(e, ctx);
            }
            for i in sc.geometries.iter().flatten() {
                visitor(i, ctx);
            }
            for (_, e) in sc.change_callbacks.iter() {
                visitor(e, ctx);
            }
        });
        for (idx, g) in self.globals.iter_enumerated() {
            let ctx = EvaluationContext::new_global(self, idx, ());
            for e in g.init_values.iter().filter_map(|x| x.as_ref()) {
                visitor(&e.expression, &ctx)
            }
            for e in g.change_callbacks.values() {
                visitor(e, &ctx)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PublicProperty {
    pub name: SmolStr,
    pub ty: Type,
    pub prop: PropertyReference,
    pub read_only: bool,
}
pub type PublicProperties = Vec<PublicProperty>;
pub type PrivateProperties = Vec<(SmolStr, Type)>;
