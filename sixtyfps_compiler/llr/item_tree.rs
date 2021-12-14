// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::rc::Rc;

use crate::langtype::Type;

use super::Expression;

// Index in the `SubComponent::properties`
pub type PropertyIndex = usize;

pub enum Animation {
    /// The expression is a Struct with the animation fields
    Static(Expression),
    Transition(Expression),
}
pub struct BindingExpression {
    pub expression: Expression,
    pub animation: Option<Animation>,
}

pub struct GlobalComponent {
    pub name: String,
    pub properties: Vec<Property>,
    pub init_values: Vec<Option<Expression>>,
}

/// a Reference to a property, in the context of a SubComponent
#[derive(Clone, Debug)]
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

pub struct Property {
    pub name: String,
    pub ty: Type,
    //pub binding: Option<BindingExpression>,
}

pub struct RepeatedElement {
    pub model: Expression,
    /// Within the sub_tree's root component
    pub index_prop: PropertyIndex,
    /// Within the sub_tree's root component
    pub data_prop: PropertyIndex,
    pub sub_tree: ItemTree,
}

pub struct ItemType {
    // cpp_name: String,
// rust_name: String,
// cpp_init_function: String,
// mouse_function: String,
// extra_data_type: String,
}

pub struct Item {
    pub ty: Rc<ItemType>,
}

pub struct TreeNode {
    pub sub_component_path: Vec<usize>,
    pub item_index: usize,
    pub children: Vec<TreeNode>,
}

pub struct SubComponent {
    pub name: String,
    pub properties: Vec<Property>,
    pub items: Vec<Item>,
    pub repeated: Vec<RepeatedElement>,
    pub sub_components: Vec<SubComponentInstance>,
    pub property_init: Vec<(PropertyReference, BindingExpression)>,
    pub two_way_bindings: Vec<(PropertyReference, PropertyReference)>,
}

pub struct SubComponentInstance {
    pub ty: Rc<SubComponent>,
    //pub property_values: Vec<(PropertyReference, BindingExpression)>,
}

pub struct ItemTree {
    pub root: SubComponentInstance,
    pub tree: TreeNode,
    /// This tree has a parent. e.g: it is a Repeater or a PopupMenu whose property can access
    /// the parent ItemTree.
    /// The String is the type of the parent ItemTree
    pub parent_context: Option<String>,
}

pub struct PublicComponent {
    pub item_tree: ItemTree,
    pub sub_components: HashMap<String, Rc<SubComponent>>,
    pub globals: Vec<GlobalComponent>,
}
