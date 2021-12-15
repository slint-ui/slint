// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use by_address::ByAddress;

use crate::langtype::Type;
use crate::llr::item_tree::*;
use crate::namedreference::NamedReference;
use crate::object_tree::{Component, ElementRc};
use std::collections::HashMap;
use std::rc::Rc;

use super::lower_expression::ExpressionContext;

pub fn lower_to_item_tree(component: &Rc<Component>) -> PublicComponent {
    let mut state = LoweringState::default();

    let mut globals = Vec::new();
    for g in &component.used_types.borrow().globals {
        let count = globals.len();
        globals.push(lower_global(g, count, &mut state));
    }
    for c in &component.used_types.borrow().sub_components {
        let sc = lower_sub_component(c, &state, None);
        state.sub_components.insert(ByAddress(c.clone()), sc);
    }

    let sc = lower_sub_component(component, &state, None);
    let public_properties = component
        .root_element
        .borrow()
        .property_declarations
        .iter()
        .filter(|(_, c)| c.expose_in_public_api)
        .map(|(p, c)| {
            let property_reference = sc
                .mapping
                .map_property_reference(&NamedReference::new(&component.root_element, p), &state)
                .unwrap();
            (p.clone(), (c.property_type.clone(), property_reference))
        })
        .collect();
    let item_tree = ItemTree {
        tree: make_tree(&state, &component.root_element, &sc, &[]),
        root: Rc::try_unwrap(sc.sub_component).unwrap(),
        parent_context: None,
    };
    PublicComponent {
        item_tree,
        globals,
        sub_components: state
            .sub_components
            .into_iter()
            .map(|(c, sc)| (c.id.clone(), sc.sub_component))
            .collect(),
        public_properties,
    }
}

#[derive(Default)]
pub struct LoweringState {
    global_properties: HashMap<NamedReference, PropertyReference>,
    sub_components: HashMap<ByAddress<Rc<Component>>, LoweredSubComponent>,
}

#[derive(Debug, Clone)]
pub enum LoweredElement {
    SubComponent { sub_component_index: usize },
    NativeItem { item_index: usize },
    Repeated { repeated_index: usize },
}

#[derive(Default, Debug, Clone)]
pub struct LoweredSubComponentMapping {
    pub element_mapping: HashMap<ByAddress<ElementRc>, LoweredElement>,
    pub property_mapping: HashMap<NamedReference, PropertyReference>,
}

impl LoweredSubComponentMapping {
    pub fn map_property_reference(
        &self,
        from: &NamedReference,
        state: &LoweringState,
    ) -> Option<PropertyReference> {
        if let Some(x) = self.property_mapping.get(&from) {
            return Some(x.clone());
        }
        if let Some(x) = state.global_properties.get(&from) {
            return Some(x.clone());
        }
        let element = from.element();
        if let Some(alias) = element
            .borrow()
            .property_declarations
            .get(from.name())
            .and_then(|x| x.is_alias.as_ref())
        {
            return state.map_property_reference(alias);
        }
        match self.element_mapping.get(&element.clone().into())? {
            LoweredElement::SubComponent { sub_component_index } => {
                if let Type::Component(base) = &element.borrow().base_type {
                    return Some(property_reference_within_sub_component(
                        state.map_property_reference(&NamedReference::new(
                            &base.root_element,
                            from.name(),
                        ))?,
                        *sub_component_index,
                    ));
                }
                unreachable!()
            }
            LoweredElement::NativeItem { item_index } => {
                return Some(PropertyReference::InNativeItem {
                    sub_component_path: vec![],
                    item_index: *item_index,
                    prop_name: from.name().into(),
                });
            }
            LoweredElement::Repeated { .. } => unreachable!(),
        }
    }
}

pub struct LoweredSubComponent {
    sub_component: Rc<SubComponent>,
    mapping: LoweredSubComponentMapping,
}

impl LoweringState {
    pub fn map_property_reference(&self, from: &NamedReference) -> Option<PropertyReference> {
        if let Some(x) = self.global_properties.get(&from) {
            return Some(x.clone());
        }

        let element = from.element();
        let enclosing = self
            .sub_components
            .get(&element.borrow().enclosing_component.upgrade().unwrap().into())?;

        enclosing.mapping.map_property_reference(from, self)
    }
}

// Map a PropertyReference within a `sub_component` to a PropertyReference to the component containing it
fn property_reference_within_sub_component(
    mut prop_ref: PropertyReference,
    sub_component: usize,
) -> PropertyReference {
    match &mut prop_ref {
        PropertyReference::Local { sub_component_path, .. }
        | PropertyReference::InNativeItem { sub_component_path, .. } => {
            sub_component_path.insert(0, sub_component);
        }
        PropertyReference::InParent { .. } => panic!("the sub-component had no parents"),
        PropertyReference::Global { .. } => (),
    }
    prop_ref
}

impl LoweringState {
    fn sub_component(&self, component: &Rc<Component>) -> &LoweredSubComponent {
        &self.sub_components[&ByAddress(component.clone())]
    }
}

fn component_id(component: &Rc<Component>) -> String {
    if component.is_global() {
        component.root_element.borrow().id.clone()
    } else if component.id.is_empty() {
        format!("Component_{}", component.root_element.borrow().id)
    } else if component.is_sub_component() {
        format!("{}_{}", component.id, component.root_element.borrow().id)
    } else {
        component.id.clone()
    }
}

fn lower_sub_component(
    component: &Rc<Component>,
    state: &LoweringState,
    parent_context: Option<&ExpressionContext>,
) -> LoweredSubComponent {
    let mut sub_component = SubComponent {
        name: component_id(component),
        properties: Default::default(),
        items: Default::default(),
        repeated: Default::default(),
        popup_windows: Default::default(),
        sub_components: Default::default(),
        property_init: Default::default(),
        two_way_bindings: Default::default(),
        const_properties: Default::default(),
    };
    let mut mapping = LoweredSubComponentMapping::default();
    let mut property_bindings = vec![];
    let mut repeated = vec![];

    if let Some(parent) = component.parent_element.upgrade() {
        // Add properties for the model data and index
        if parent.borrow().repeated.is_some() {
            sub_component.properties.push(Property {
                name: "model_data".into(),
                ty: crate::expression_tree::Expression::RepeaterModelReference {
                    element: component.parent_element.clone(),
                }
                .ty(),
            });
            sub_component.properties.push(Property { name: "model_index".into(), ty: Type::Int32 });
        }
    };

    crate::object_tree::recurse_elem(&component.root_element, &(), &mut |element, _| {
        let elem = element.borrow();
        for (p, x) in &elem.property_declarations {
            let property_index = sub_component.properties.len();
            mapping.property_mapping.insert(
                NamedReference::new(element, &p),
                PropertyReference::Local { sub_component_path: vec![], property_index },
            );
            sub_component
                .properties
                .push(Property { name: format!("{}_{}", elem.id, p), ty: x.property_type.clone() });
            if let Some(b) = elem.bindings.get(p.as_str()) {
                property_bindings.push((
                    PropertyReference::Local { sub_component_path: vec![], property_index },
                    b.borrow().clone(),
                ));
            }
        }
        if elem.repeated.is_some() {
            mapping.element_mapping.insert(
                element.clone().into(),
                LoweredElement::Repeated { repeated_index: repeated.len() },
            );
            repeated.push(element.clone());
            return;
        }
        match &elem.base_type {
            Type::Component(comp) => {
                let lc = state.sub_component(comp);
                let ty = lc.sub_component.clone();
                let sub_component_index = sub_component.sub_components.len();
                mapping.element_mapping.insert(
                    element.clone().into(),
                    LoweredElement::SubComponent { sub_component_index },
                );
                for (p, b) in &elem.bindings {
                    if elem.property_declarations.contains_key(p.as_str()) {
                        continue;
                    }
                    let prop_ref = state
                        .map_property_reference(&NamedReference::new(&comp.root_element, p))
                        .map(|x| property_reference_within_sub_component(x, sub_component_index));
                    property_bindings.push((prop_ref.unwrap(), b.borrow().clone()));
                }
                sub_component
                    .sub_components
                    .push(SubComponentInstance { ty, name: elem.id.clone() });
            }

            Type::Native(n) => {
                let item_index = sub_component.items.len();
                mapping
                    .element_mapping
                    .insert(element.clone().into(), LoweredElement::NativeItem { item_index });
                for (p, b) in &elem.bindings {
                    if elem.property_declarations.contains_key(p.as_str()) {
                        continue;
                    }
                    property_bindings.push((
                        PropertyReference::InNativeItem {
                            sub_component_path: vec![],
                            item_index,
                            prop_name: p.clone(),
                        },
                        b.borrow().clone(),
                    ));
                }
                sub_component.items.push(Item {
                    ty: n.clone(),
                    name: elem.id.clone(),
                    is_flickable_viewport: elem.is_flickable_viewport,
                })
            }
            _ => unreachable!(),
        };
    });
    let ctx = ExpressionContext { mapping: &mapping, state, parent: parent_context, component };
    for (prop, binding) in property_bindings {
        for tw in &binding.two_way_bindings {
            sub_component
                .two_way_bindings
                .push((prop.clone(), mapping.map_property_reference(tw, state).unwrap()))
        }
        if let Some(expression) =
            super::lower_expression::lower_expression(&binding.expression, &ctx)
        {
            let animation = binding
                .animation
                .as_ref()
                .and_then(|a| super::lower_expression::lower_animation(a, &ctx));
            sub_component
                .property_init
                .push((prop.clone(), BindingExpression { expression, animation }))
        }
    }
    sub_component.repeated =
        repeated.into_iter().map(|elem| lower_repeated_component(&elem, &ctx)).collect();
    sub_component.popup_windows = component
        .popup_windows
        .borrow()
        .iter()
        .map(|popup| lower_popup_component(&popup.component, &ctx))
        .collect();

    crate::generator::for_each_const_properties(component, |elem, n| {
        if let Some(x) = ctx.map_property_reference(&NamedReference::new(elem, n)) {
            sub_component.const_properties.push(x);
        }
    });

    LoweredSubComponent { sub_component: Rc::new(sub_component), mapping }
}

fn lower_repeated_component(elem: &ElementRc, ctx: &ExpressionContext) -> RepeatedElement {
    let e = elem.borrow();
    let component = e.base_type.as_component().clone();
    let repeated = e.repeated.as_ref().unwrap();

    let sc = lower_sub_component(&component, &ctx.state, Some(ctx));
    RepeatedElement {
        model: super::lower_expression::lower_expression(&repeated.model, ctx).unwrap(),
        sub_tree: ItemTree {
            tree: make_tree(ctx.state, &component.root_element, &sc, &[]),
            root: Rc::try_unwrap(sc.sub_component).unwrap(),
            parent_context: Some(e.enclosing_component.upgrade().unwrap().id.clone()),
        },
        index_prop: 1,
        data_prop: 0,
    }
}

fn lower_popup_component(component: &Rc<Component>, ctx: &ExpressionContext) -> ItemTree {
    let sc = lower_sub_component(component, &ctx.state, Some(ctx));
    ItemTree {
        tree: make_tree(ctx.state, &component.root_element, &sc, &[]),
        root: Rc::try_unwrap(sc.sub_component).unwrap(),
        parent_context: Some(
            component
                .parent_element
                .upgrade()
                .unwrap()
                .borrow()
                .enclosing_component
                .upgrade()
                .unwrap()
                .id
                .clone(),
        ),
    }
}

fn lower_global(
    global: &Rc<Component>,
    global_index: usize,
    state: &mut LoweringState,
) -> GlobalComponent {
    let mut mapping = LoweredSubComponentMapping::default();
    let mut properties = vec![];
    let mut const_properties = vec![];

    for (p, x) in &global.root_element.borrow().property_declarations {
        let property_index = properties.len();
        let nr = NamedReference::new(&global.root_element, &p);
        mapping.property_mapping.insert(
            nr.clone(),
            PropertyReference::Local { sub_component_path: vec![], property_index },
        );

        properties.push(Property { name: p.clone(), ty: x.property_type.clone() });
        const_properties.push(nr.is_constant());
        state
            .global_properties
            .insert(nr.clone(), PropertyReference::Global { global_index, property_index });
    }

    let mut init_values = vec![None; properties.len()];

    let ctx = ExpressionContext { mapping: &mapping, state, parent: None, component: global };
    for (prop, binding) in &global.root_element.borrow().bindings {
        assert!(binding.borrow().two_way_bindings.is_empty());
        assert!(binding.borrow().animation.is_none());
        if let Some(expression) =
            super::lower_expression::lower_expression(&binding.borrow().expression, &ctx)
        {
            let nr = NamedReference::new(&global.root_element, prop);
            let property_index = match mapping.property_mapping[&nr] {
                PropertyReference::Local { property_index, .. } => property_index,
                _ => unreachable!(),
            };
            init_values[property_index] = Some(expression);
        }
    }

    GlobalComponent { name: global.id.clone(), properties, init_values, const_properties }
}

fn make_tree(
    state: &LoweringState,
    element: &ElementRc,
    component: &LoweredSubComponent,
    sub_component_path: &[usize],
) -> TreeNode {
    let e = element.borrow();
    let children = e.children.iter().map(|c| make_tree(state, c, component, sub_component_path));
    match component.mapping.element_mapping.get(&ByAddress(element.clone())).unwrap() {
        LoweredElement::SubComponent { sub_component_index } => {
            let sub_component = e.sub_component().unwrap();
            let new_sub_component_path = sub_component_path
                .iter()
                .copied()
                .chain(std::iter::once(*sub_component_index))
                .collect::<Vec<_>>();
            let mut tree_node = make_tree(
                state,
                &sub_component.root_element,
                state.sub_component(sub_component),
                &new_sub_component_path,
            );
            tree_node.children.extend(children);
            tree_node
        }
        LoweredElement::NativeItem { item_index } => TreeNode {
            sub_component_path: sub_component_path.into(),
            item_index: *item_index,
            children: children.collect(),
            repeated: false,
        },
        LoweredElement::Repeated { repeated_index } => TreeNode {
            sub_component_path: sub_component_path.into(),
            item_index: *repeated_index,
            children: vec![],
            repeated: true,
        },
    }
}
