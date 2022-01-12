// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use super::Expression;
use crate::langtype::{NativeClass, Type};
use std::collections::{BTreeMap, HashMap};
use std::num::NonZeroUsize;
use std::rc::Rc;

// Index in the `SubComponent::properties`
pub type PropertyIndex = usize;

#[derive(Debug, Clone)]
pub enum Animation {
    /// The expression is a Struct with the animation fields
    Static(Expression),
    Transition(Expression),
}

#[derive(Debug, Clone)]
pub struct BindingExpression {
    pub expression: Expression,
    pub animation: Option<Animation>,
    /// When true, we can initialize the property with `set` otherwise, `set_binding` must be used
    pub is_constant: bool,
}

#[derive(Debug)]
pub struct GlobalComponent {
    pub name: String,
    pub properties: Vec<Property>,
    /// One entry per property
    pub init_values: Vec<Option<BindingExpression>>,
    pub const_properties: Vec<bool>,
    pub public_properties: PublicProperties,
    /// true if we should expose the global in the generated API
    pub exported: bool,
    /// The extra names under which this component should be accessible
    /// if it is exported several time.
    pub aliases: Vec<String>,
    /// True when this is a built-in global that does not need to be generated
    pub is_builtin: bool,
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
}

#[derive(Debug)]
pub struct Property {
    pub name: String,
    pub ty: Type,
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
    pub model: Expression,
    /// Within the sub_tree's root component
    pub index_prop: Option<PropertyIndex>,
    /// Within the sub_tree's root component
    pub data_prop: Option<PropertyIndex>,
    pub sub_tree: ItemTree,
    /// The index of the item node in the parent tree
    pub index_in_tree: usize,

    pub listview: Option<ListViewInfo>,
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
    pub items: Vec<Item>,
    pub repeated: Vec<RepeatedElement>,
    pub popup_windows: Vec<ItemTree>,
    pub sub_components: Vec<SubComponentInstance>,
    /// The initial value or binding for properties.
    /// This is ordered in the order they must be set.
    pub property_init: Vec<(PropertyReference, BindingExpression)>,
    /// The animation for properties which are animated
    pub animations: HashMap<PropertyReference, Expression>,
    pub two_way_bindings: Vec<(PropertyReference, PropertyReference)>,
    pub const_properties: Vec<PropertyReference>,
    // Code that is run in the sub component constructor, after property initializations
    pub init_code: Vec<Expression>,

    pub layout_info_h: Expression,
    pub layout_info_v: Expression,
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
    pub item_tree: ItemTree,
    pub sub_components: Vec<Rc<SubComponent>>,
    pub globals: Vec<GlobalComponent>,
}

pub type PublicProperties = BTreeMap<String, (Type, PropertyReference)>;
