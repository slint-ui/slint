// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use super::lower_to_item_tree::EmbeddingIndex;
use super::{EvaluationContext, Expression, ParentCtx};
use crate::langtype::{NativeClass, Type};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::num::NonZeroUsize;
use std::rc::Rc;

// Index in the `SubComponent::properties`
pub type PropertyIndex = usize;

#[derive(Debug, Clone, derive_more::Deref)]
pub struct MutExpression(RefCell<Expression>);

impl From<Expression> for MutExpression {
    fn from(e: Expression) -> Self {
        Self(e.into())
    }
}

impl MutExpression {
    pub fn visit_recursive(&self, visitor: &mut dyn FnMut(&Expression)) {
        self.0.borrow().visit_recursive(visitor)
    }
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
    /// the property is of type StateInfo and the `set_state_binding` ned to be used on the property
    pub is_state_info: bool,

    /// The amount of time this binding is used
    /// This property is only valid after the [`count_property_use`](super::optim_passes::count_property_use) pass
    pub use_count: Cell<usize>,
}

#[derive(Debug)]
pub struct GlobalComponent {
    pub name: String,
    pub properties: Vec<Property>,
    pub functions: Vec<Function>,
    /// One entry per property
    pub init_values: Vec<Option<BindingExpression>>,
    pub const_properties: Vec<bool>,
    pub public_properties: PublicProperties,
    pub private_properties: PrivateProperties,
    /// true if we should expose the global in the generated API
    pub exported: bool,
    /// The extra names under which this component should be accessible
    /// if it is exported several time.
    pub aliases: Vec<String>,
    /// True when this is a built-in global that does not need to be generated
    pub is_builtin: bool,

    /// Analysis for each properties
    pub prop_analysis: Vec<crate::object_tree::PropertyAnalysis>,
}

/// a Reference to a property, in the context of a SubComponent
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum PropertyReference {
    /// A property relative to this SubComponent
    Local { sub_component_path: Vec<usize>, property_index: PropertyIndex },
    /// A property in a Native item
    InNativeItem { sub_component_path: Vec<usize>, item_index: usize, prop_name: String },
    /// The properties is a property relative to a parent ItemTree (`level` level deep)
    InParent { level: NonZeroUsize, parent_reference: Box<PropertyReference> },
    /// The property within a GlobalComponent
    Global { global_index: usize, property_index: usize },

    /// A function in a sub component.
    Function { sub_component_path: Vec<usize>, function_index: usize },
    /// A function in a global.
    GlobalFunction { global_index: usize, function_index: usize },
}

#[derive(Debug, Default)]
pub struct Property {
    pub name: String,
    pub ty: Type,
    /// The amount of time this property is used of another property
    /// This property is only valid after the [`count_property_use`](super::optim_passes::count_property_use) pass
    pub use_count: Cell<usize>,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
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
    pub prop_width: PropertyReference,
    // In the repeated component context
    pub prop_height: PropertyReference,
}

#[derive(Debug)]
pub struct RepeatedElement {
    pub model: MutExpression,
    /// Within the sub_tree's root component
    pub index_prop: Option<PropertyIndex>,
    /// Within the sub_tree's root component
    pub data_prop: Option<PropertyIndex>,
    pub sub_tree: ItemTree,
    /// The index of the item node in the parent tree
    pub index_in_tree: usize,

    pub listview: Option<ListViewInfo>,
}

#[derive(Debug)]
pub struct EmbeddedElement {
    /// The index of the item node in the parent tree
    pub embed_item_index: EmbeddingIndex,
    /// The index to a dynamic tree node where the component is supposed to be embedded at
    pub embedding_placeholder: usize,
    /// The sub component index this embedding is a part of (usize::MAX if its in the root
    /// component)
    pub sub_component_path: Vec<usize>,
}

pub struct Item {
    pub ty: Rc<NativeClass>,
    pub name: String,
    /// Index in the item tree array
    pub index_in_tree: usize,
    /// When this is true, this item does not need to be created because it is
    /// already in the flickable.
    /// The Item::name is the same as the flickable, and ty is Rectangle
    pub is_flickable_viewport: bool,
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Item")
            .field("ty", &self.ty.class_name)
            .field("name", &self.name)
            .field("index_in_tree", &self.index_in_tree)
            .field("is_flickable_viewport", &self.is_flickable_viewport)
            .finish()
    }
}

#[derive(Debug)]
pub struct TreeNode {
    pub sub_component_path: Vec<usize>,
    /// Either an index in the items or repeater, depending on repeated
    pub item_index: usize,
    pub repeated: bool,
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

    /// Visit this, and the children. passes
    /// children_offset must be set to `1` for the root
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
    pub name: String,
    pub properties: Vec<Property>,
    pub functions: Vec<Function>,
    pub items: Vec<Item>,
    pub repeated: Vec<RepeatedElement>,
    pub embedded: Vec<EmbeddedElement>,
    pub popup_windows: Vec<ItemTree>,
    pub sub_components: Vec<SubComponentInstance>,
    /// The initial value or binding for properties.
    /// This is ordered in the order they must be set.
    pub property_init: Vec<(PropertyReference, BindingExpression)>,
    /// The animation for properties which are animated
    pub animations: HashMap<PropertyReference, Expression>,
    pub two_way_bindings: Vec<(PropertyReference, PropertyReference)>,
    pub const_properties: Vec<PropertyReference>,
    /// Code that is run in the sub component constructor, after property initializations
    pub init_code: Vec<MutExpression>,

    pub layout_info_h: MutExpression,
    pub layout_info_v: MutExpression,

    /// Maps (item_index, property) to an expression
    pub accessible_prop: BTreeMap<(usize, String), MutExpression>,

    pub prop_analysis: HashMap<PropertyReference, PropAnalysis>,
}

#[derive(Debug, Clone)]
pub struct PropAnalysis {
    /// Index in SubComponent::property_init for this property
    pub property_init: Option<usize>,
    pub analysis: crate::object_tree::PropertyAnalysis,
}

impl SubComponent {
    /// total count of repeater, including in sub components
    pub fn repeater_count(&self) -> usize {
        let mut count = self.repeated.len();
        for x in self.sub_components.iter() {
            count += x.ty.repeater_count();
        }
        count
    }

    /// total count of embeddings, including in sub components
    pub fn embedding_count(&self) -> usize {
        let mut count = self.embedded.len();
        for x in self.sub_components.iter() {
            count += x.ty.embedding_count();
        }
        count
    }

    /// total count of items, including in sub components
    pub fn child_item_count(&self) -> usize {
        let mut count = self.items.len();
        for x in self.sub_components.iter() {
            count += x.ty.child_item_count();
        }
        count
    }
}

pub struct SubComponentInstance {
    pub ty: Rc<SubComponent>,
    pub name: String,
    pub index_in_tree: usize,
    pub index_of_first_child_in_tree: usize,
    pub repeater_offset: usize,
}

impl std::fmt::Debug for SubComponentInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubComponentInstance")
            // only dump ty.name, not the whole structure
            .field("ty", &self.ty.name)
            .field("name", &self.name)
            .field("index_in_tree", &self.index_in_tree)
            .field("index_of_first_child_in_tree", &self.index_of_first_child_in_tree)
            .field("repeater_offset", &self.repeater_offset)
            .finish()
    }
}

#[derive(Debug)]
pub struct ItemTree {
    pub root: SubComponent,
    pub tree: TreeNode,
    /// This tree has a parent. e.g: it is a Repeater or a PopupMenu whose property can access
    /// the parent ItemTree.
    /// The String is the type of the parent ItemTree
    pub parent_context: Option<String>,
}

#[derive(Debug)]
pub struct PublicComponent {
    pub public_properties: PublicProperties,
    pub private_properties: PrivateProperties,
    pub item_tree: ItemTree,
    pub sub_components: Vec<Rc<SubComponent>>,
    pub globals: Vec<GlobalComponent>,
}

impl PublicComponent {
    pub fn for_each_sub_components<'a>(
        &'a self,
        visitor: &mut dyn FnMut(&'a SubComponent, &EvaluationContext<'_>),
    ) {
        fn visit_component<'a>(
            root: &'a PublicComponent,
            c: &'a SubComponent,
            visitor: &mut dyn FnMut(&'a SubComponent, &EvaluationContext<'_>),
            parent: Option<ParentCtx<'_>>,
        ) {
            let ctx = EvaluationContext::new_sub_component(root, c, (), parent);
            visitor(c, &ctx);
            for (idx, r) in c.repeated.iter().enumerate() {
                visit_component(
                    root,
                    &r.sub_tree.root,
                    visitor,
                    Some(ParentCtx::new(&ctx, Some(idx))),
                );
            }
            for x in &c.popup_windows {
                visit_component(root, &x.root, visitor, Some(ParentCtx::new(&ctx, None)));
            }
        }
        for c in &self.sub_components {
            visit_component(self, c, visitor, None);
        }
        visit_component(self, &self.item_tree.root, visitor, None);
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
            for (_, e) in &sc.accessible_prop {
                visitor(e, ctx);
            }
        });
        for g in &self.globals {
            let ctx = EvaluationContext::new_global(self, g, ());
            for e in g.init_values.iter().filter_map(|x| x.as_ref()) {
                visitor(&e.expression, &ctx)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PublicProperty {
    pub name: String,
    pub ty: Type,
    pub prop: PropertyReference,
    pub read_only: bool,
}
pub type PublicProperties = Vec<PublicProperty>;
pub type PrivateProperties = Vec<(String, Type)>;
