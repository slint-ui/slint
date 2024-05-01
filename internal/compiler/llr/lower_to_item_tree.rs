// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use by_address::ByAddress;

use super::lower_expression::ExpressionContext;
use crate::expression_tree::Expression as tree_Expression;
use crate::langtype::{ElementType, Type};
use crate::llr::item_tree::*;
use crate::namedreference::NamedReference;
use crate::object_tree::{Component, ElementRc, PropertyVisibility};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

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
    let public_properties = public_properties(component, &sc.mapping, &state);
    let item_tree = ItemTree {
        tree: make_tree(&state, &component.root_element, &sc, &[]),
        root: Rc::try_unwrap(sc.sub_component).unwrap(),
        parent_context: None,
    };
    let root = PublicComponent {
        item_tree,
        globals,
        sub_components: component
            .used_types
            .borrow()
            .sub_components
            .iter()
            .map(|tree_sub_compo| {
                state.sub_components[&ByAddress(tree_sub_compo.clone())].sub_component.clone()
            })
            .collect(),
        public_properties,
        private_properties: component.private_properties.borrow().clone(),
    };
    super::optim_passes::run_passes(&root);
    root
}

#[derive(Default)]
pub struct LoweringState {
    global_properties: HashMap<NamedReference, PropertyReference>,
    sub_components: HashMap<ByAddress<Rc<Component>>, LoweredSubComponent>,
}

#[derive(Debug, Clone)]
pub enum LoweredElement {
    SubComponent { sub_component_index: usize },
    NativeItem { item_index: u32 },
    Repeated { repeated_index: u32 },
    ComponentPlaceholder { repeated_index: u32 },
}

#[derive(Default, Debug, Clone)]
pub struct LoweredSubComponentMapping {
    pub element_mapping: HashMap<ByAddress<ElementRc>, LoweredElement>,
    pub property_mapping: HashMap<NamedReference, PropertyReference>,
    pub repeater_count: u32,
    pub container_count: u32,
}

impl LoweredSubComponentMapping {
    pub fn map_property_reference(
        &self,
        from: &NamedReference,
        state: &LoweringState,
    ) -> PropertyReference {
        if let Some(x) = self.property_mapping.get(from) {
            return x.clone();
        }
        if let Some(x) = state.global_properties.get(from) {
            return x.clone();
        }
        let element = from.element();
        if let Some(alias) = element
            .borrow()
            .property_declarations
            .get(from.name())
            .and_then(|x| x.is_alias.as_ref())
        {
            return self.map_property_reference(alias, state);
        }
        match self.element_mapping.get(&element.clone().into()).unwrap() {
            LoweredElement::SubComponent { sub_component_index } => {
                if let ElementType::Component(base) = &element.borrow().base_type {
                    return property_reference_within_sub_component(
                        state.map_property_reference(&NamedReference::new(
                            &base.root_element,
                            from.name(),
                        )),
                        *sub_component_index,
                    );
                }
                unreachable!()
            }
            LoweredElement::NativeItem { item_index } => {
                return PropertyReference::InNativeItem {
                    sub_component_path: vec![],
                    item_index: *item_index,
                    prop_name: from.name().into(),
                };
            }
            LoweredElement::Repeated { .. } => unreachable!(),
            LoweredElement::ComponentPlaceholder { .. } => unreachable!(),
        }
    }
}

pub struct LoweredSubComponent {
    sub_component: Rc<SubComponent>,
    mapping: LoweredSubComponentMapping,
}

impl LoweringState {
    pub fn map_property_reference(&self, from: &NamedReference) -> PropertyReference {
        if let Some(x) = self.global_properties.get(from) {
            return x.clone();
        }

        let element = from.element();
        let enclosing = self
            .sub_components
            .get(&element.borrow().enclosing_component.upgrade().unwrap().into())
            .unwrap();

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
        | PropertyReference::InNativeItem { sub_component_path, .. }
        | PropertyReference::Function { sub_component_path, .. } => {
            sub_component_path.insert(0, sub_component);
        }
        PropertyReference::InParent { .. } => panic!("the sub-component had no parents"),
        PropertyReference::Global { .. } | PropertyReference::GlobalFunction { .. } => (),
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
        functions: Default::default(),
        items: Default::default(),
        repeated: Default::default(),
        component_containers: Default::default(),
        popup_windows: Default::default(),
        sub_components: Default::default(),
        property_init: Default::default(),
        change_callbacks: Default::default(),
        animations: Default::default(),
        two_way_bindings: Default::default(),
        const_properties: Default::default(),
        init_code: Default::default(),
        geometries: Default::default(),
        // just initialize to dummy expression right now and it will be set later
        layout_info_h: super::Expression::BoolLiteral(false).into(),
        layout_info_v: super::Expression::BoolLiteral(false).into(),
        accessible_prop: Default::default(),
        prop_analysis: Default::default(),
    };
    let mut mapping = LoweredSubComponentMapping::default();
    let mut repeated = vec![];
    let mut component_container_data = vec![];
    let mut accessible_prop = Vec::new();
    let mut change_callbacks = Vec::new();

    if let Some(parent) = component.parent_element.upgrade() {
        // Add properties for the model data and index
        if parent.borrow().repeated.as_ref().map_or(false, |x| !x.is_conditional_element) {
            sub_component.properties.push(Property {
                name: "model_data".into(),
                ty: crate::expression_tree::Expression::RepeaterModelReference {
                    element: component.parent_element.clone(),
                }
                .ty(),
                ..Property::default()
            });
            sub_component.properties.push(Property {
                name: "model_index".into(),
                ty: Type::Int32,
                ..Property::default()
            });
        }
    };

    let s: Option<ElementRc> = None;
    let mut repeater_offset = 0;
    crate::object_tree::recurse_elem(&component.root_element, &s, &mut |element, parent| {
        let elem = element.borrow();
        for (p, x) in &elem.property_declarations {
            if x.is_alias.is_some() {
                continue;
            }
            if let Type::Function { return_type, args } = &x.property_type {
                let function_index = sub_component.functions.len();
                mapping.property_mapping.insert(
                    NamedReference::new(element, p),
                    PropertyReference::Function { sub_component_path: vec![], function_index },
                );
                sub_component.functions.push(Function {
                    name: p.into(),
                    ret_ty: (**return_type).clone(),
                    args: args.clone(),
                    // will be replaced later
                    code: super::Expression::CodeBlock(vec![]),
                });
                continue;
            }
            let property_index = sub_component.properties.len();
            mapping.property_mapping.insert(
                NamedReference::new(element, p),
                PropertyReference::Local { sub_component_path: vec![], property_index },
            );
            sub_component.properties.push(Property {
                name: format!("{}_{}", elem.id, p),
                ty: x.property_type.clone(),
                ..Property::default()
            });
        }
        if elem.is_component_placeholder {
            mapping.element_mapping.insert(
                element.clone().into(),
                LoweredElement::ComponentPlaceholder {
                    repeated_index: component_container_data.len() as u32,
                },
            );
            mapping.container_count += 1;
            component_container_data.push(parent.as_ref().unwrap().clone());
            return None;
        }
        if elem.repeated.is_some() {
            mapping.element_mapping.insert(
                element.clone().into(),
                LoweredElement::Repeated { repeated_index: repeated.len() as u32 },
            );
            repeated.push(element.clone());
            mapping.repeater_count += 1;
            return None;
        }
        match &elem.base_type {
            ElementType::Component(comp) => {
                let lc = state.sub_component(comp);
                let ty = lc.sub_component.clone();
                let sub_component_index = sub_component.sub_components.len();
                mapping.element_mapping.insert(
                    element.clone().into(),
                    LoweredElement::SubComponent { sub_component_index },
                );
                sub_component.sub_components.push(SubComponentInstance {
                    ty: ty.clone(),
                    name: elem.id.clone(),
                    index_in_tree: *elem.item_index.get().unwrap(),
                    index_of_first_child_in_tree: *elem.item_index_of_first_children.get().unwrap(),
                    repeater_offset,
                });
                repeater_offset += ty.repeater_count();
            }

            ElementType::Native(n) => {
                let item_index = sub_component.items.len() as u32;
                mapping
                    .element_mapping
                    .insert(element.clone().into(), LoweredElement::NativeItem { item_index });
                sub_component.items.push(Item {
                    ty: n.clone(),
                    name: elem.id.clone(),
                    index_in_tree: *elem.item_index.get().unwrap(),
                })
            }
            _ => unreachable!(),
        };
        for (key, nr) in &elem.accessibility_props.0 {
            // TODO: we also want to split by type (role/string/...)
            let enum_value =
                crate::generator::to_pascal_case(key.strip_prefix("accessible-").unwrap());
            accessible_prop.push((*elem.item_index.get().unwrap(), enum_value, nr.clone()));
        }

        for (prop, expr) in &elem.change_callbacks {
            change_callbacks.push((NamedReference::new(&element, prop), expr.borrow().clone()));
        }

        Some(element.clone())
    });
    let ctx = ExpressionContext { mapping: &mapping, state, parent: parent_context, component };
    crate::generator::handle_property_bindings_init(component, |e, p, binding| {
        let nr = NamedReference::new(e, p);
        let prop = ctx.map_property_reference(&nr);

        if let Type::Function { .. } = nr.ty() {
            if let PropertyReference::Function { sub_component_path, function_index } = prop {
                assert!(sub_component_path.is_empty());
                sub_component.functions[function_index].code =
                    super::lower_expression::lower_expression(&binding.expression, &ctx);
            } else {
                unreachable!()
            }
            return;
        }

        for tw in &binding.two_way_bindings {
            sub_component.two_way_bindings.push((prop.clone(), ctx.map_property_reference(tw)))
        }
        if !matches!(binding.expression, tree_Expression::Invalid) {
            let expression =
                super::lower_expression::lower_expression(&binding.expression, &ctx).into();

            let is_constant = binding.analysis.as_ref().map_or(false, |a| a.is_const);
            let animation = binding
                .animation
                .as_ref()
                .filter(|_| !is_constant)
                .map(|a| super::lower_expression::lower_animation(a, &ctx));

            sub_component.prop_analysis.insert(
                prop.clone(),
                PropAnalysis {
                    property_init: Some(sub_component.property_init.len()),
                    analysis: get_property_analysis(e, p),
                },
            );

            let is_state_info = matches!(
                e.borrow().lookup_property(p).property_type,
                Type::Struct { name: Some(name), .. } if name.ends_with("::StateInfo")
            );

            sub_component.property_init.push((
                prop.clone(),
                BindingExpression {
                    expression,
                    animation,
                    is_constant,
                    is_state_info,
                    use_count: 0.into(),
                },
            ));
        }

        if e.borrow()
            .property_analysis
            .borrow()
            .get(p)
            .map_or(true, |a| a.is_set || a.is_set_externally)
        {
            if let Some(anim) = binding.animation.as_ref() {
                match super::lower_expression::lower_animation(anim, &ctx) {
                    Animation::Static(anim) => {
                        sub_component.animations.insert(prop, anim);
                    }
                    Animation::Transition(_) => {
                        // Cannot set a property with a transition anyway
                    }
                }
            }
        }
    });
    sub_component.component_containers = component_container_data
        .into_iter()
        .map(|component_container| {
            lower_component_container(&component_container, &sub_component, &ctx)
        })
        .collect();
    sub_component.repeated =
        repeated.into_iter().map(|elem| lower_repeated_component(&elem, &ctx)).collect();
    for s in &mut sub_component.sub_components {
        s.repeater_offset +=
            (sub_component.repeated.len() + sub_component.component_containers.len()) as u32;
    }

    sub_component.popup_windows = component
        .popup_windows
        .borrow()
        .iter()
        .map(|popup| lower_popup_component(&popup.component, &ctx))
        .collect();

    crate::generator::for_each_const_properties(component, |elem, n| {
        let x = ctx.map_property_reference(&NamedReference::new(elem, n));
        sub_component.const_properties.push(x);
    });

    sub_component.init_code = component
        .init_code
        .borrow()
        .iter()
        .map(|e| super::lower_expression::lower_expression(e, &ctx).into())
        .collect();

    sub_component.layout_info_h = super::lower_expression::get_layout_info(
        &component.root_element,
        &ctx,
        &component.root_constraints.borrow(),
        crate::layout::Orientation::Horizontal,
    )
    .into();
    sub_component.layout_info_v = super::lower_expression::get_layout_info(
        &component.root_element,
        &ctx,
        &component.root_constraints.borrow(),
        crate::layout::Orientation::Vertical,
    )
    .into();

    sub_component.accessible_prop = accessible_prop
        .into_iter()
        .map(|(idx, key, nr)| {
            let prop = ctx.map_property_reference(&nr);
            let expr = match nr.ty() {
                Type::Bool => super::Expression::Condition {
                    condition: super::Expression::PropertyReference(prop).into(),
                    true_expr: super::Expression::StringLiteral("true".into()).into(),
                    false_expr: super::Expression::StringLiteral("false".into()).into(),
                },
                Type::Int32 | Type::Float32 => super::Expression::Cast {
                    from: super::Expression::PropertyReference(prop).into(),
                    to: Type::String,
                },
                Type::String => super::Expression::PropertyReference(prop),
                Type::Enumeration(e) if e.name == "AccessibleRole" => {
                    super::Expression::PropertyReference(prop)
                }
                Type::Callback { return_type: None, args } => super::Expression::CallBackCall {
                    callback: prop,
                    arguments: (0..args.len())
                        .map(|index| super::Expression::FunctionParameterReference { index })
                        .collect(),
                },
                _ => panic!("Invalid type for accessible property"),
            };

            ((idx, key), expr.into())
        })
        .collect();

    sub_component.change_callbacks = change_callbacks
        .into_iter()
        .map(|(nr, exprs)| {
            let prop = ctx.map_property_reference(&nr);
            let expr =
                super::lower_expression::lower_expression(&tree_Expression::CodeBlock(exprs), &ctx);
            (prop, expr.into())
        })
        .collect();

    crate::object_tree::recurse_elem(&component.root_element, &(), &mut |element, _| {
        let elem = element.borrow();
        if elem.repeated.is_some() {
            return;
        };
        let Some(geom) = &elem.geometry_props else { return };
        let item_index = *elem.item_index.get().unwrap() as usize;
        if item_index >= sub_component.geometries.len() {
            sub_component.geometries.resize(item_index + 1, Default::default());
        }
        sub_component.geometries[item_index] = Some(lower_geometry(geom, &ctx).into());
    });

    LoweredSubComponent { sub_component: Rc::new(sub_component), mapping }
}

fn lower_geometry(
    geom: &crate::object_tree::GeometryProps,
    ctx: &ExpressionContext<'_>,
) -> super::Expression {
    let mut fields = BTreeMap::default();
    let mut values = HashMap::with_capacity(4);
    for (f, v) in [("x", &geom.x), ("y", &geom.y), ("width", &geom.width), ("height", &geom.height)]
    {
        fields.insert(f.into(), Type::LogicalLength);
        values
            .insert(f.into(), super::Expression::PropertyReference(ctx.map_property_reference(v)));
    }
    super::Expression::Struct {
        ty: Type::Struct { fields, name: None, node: None, rust_attributes: None },
        values,
    }
}

fn get_property_analysis(elem: &ElementRc, p: &str) -> crate::object_tree::PropertyAnalysis {
    let mut a = elem.borrow().property_analysis.borrow().get(p).cloned().unwrap_or_default();
    let mut elem = elem.clone();
    loop {
        if let Some(d) = elem.borrow().property_declarations.get(p) {
            if let Some(nr) = &d.is_alias {
                a.merge(&get_property_analysis(&nr.element(), nr.name()));
            }
            return a;
        }
        let base = elem.borrow().base_type.clone();
        match base {
            ElementType::Native(n) => {
                if n.properties.get(p).map_or(false, |p| p.is_native_output()) {
                    a.is_set = true;
                }
            }
            ElementType::Component(c) => {
                elem = c.root_element.clone();
                if let Some(a2) = elem.borrow().property_analysis.borrow().get(p) {
                    a.merge_with_base(a2);
                }
                continue;
            }
            _ => (),
        };
        return a;
    }
}

fn lower_repeated_component(elem: &ElementRc, ctx: &ExpressionContext) -> RepeatedElement {
    let e = elem.borrow();
    let component = e.base_type.as_component().clone();
    let repeated = e.repeated.as_ref().unwrap();

    let sc: LoweredSubComponent = lower_sub_component(&component, ctx.state, Some(ctx));

    let geom = component.root_element.borrow().geometry_props.clone().unwrap();

    let listview = repeated.is_listview.as_ref().map(|lv| ListViewInfo {
        viewport_y: ctx.map_property_reference(&lv.viewport_y),
        viewport_height: ctx.map_property_reference(&lv.viewport_height),
        viewport_width: ctx.map_property_reference(&lv.viewport_width),
        listview_height: ctx.map_property_reference(&lv.listview_height),
        listview_width: ctx.map_property_reference(&lv.listview_width),
        prop_y: sc.mapping.map_property_reference(&geom.y, ctx.state),
        prop_width: sc.mapping.map_property_reference(&geom.width, ctx.state),
        prop_height: sc.mapping.map_property_reference(&geom.height, ctx.state),
    });

    RepeatedElement {
        model: super::lower_expression::lower_expression(&repeated.model, ctx).into(),
        sub_tree: ItemTree {
            tree: make_tree(ctx.state, &component.root_element, &sc, &[]),
            root: Rc::try_unwrap(sc.sub_component).unwrap(),
            parent_context: Some(e.enclosing_component.upgrade().unwrap().id.clone()),
        },
        index_prop: (!repeated.is_conditional_element).then_some(1),
        data_prop: (!repeated.is_conditional_element).then_some(0),
        index_in_tree: *e.item_index.get().unwrap(),
        listview,
    }
}

fn lower_component_container(
    container: &ElementRc,
    sub_component: &SubComponent,
    _ctx: &ExpressionContext,
) -> ComponentContainerElement {
    let c = container.borrow();

    let component_container_index = *c.item_index.get().unwrap();
    let component_container_items_index = sub_component
        .items
        .iter()
        .position(|i| i.index_in_tree == component_container_index)
        .unwrap() as u32;

    ComponentContainerElement {
        component_container_item_tree_index: component_container_index,
        component_container_items_index,
        component_placeholder_item_tree_index: *c.item_index_of_first_children.get().unwrap(),
    }
}

fn lower_popup_component(component: &Rc<Component>, ctx: &ExpressionContext) -> ItemTree {
    let sc = lower_sub_component(component, ctx.state, Some(ctx));
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
    let mut prop_analysis = vec![];
    let mut functions = vec![];

    for (p, x) in &global.root_element.borrow().property_declarations {
        let property_index = properties.len();
        let nr = NamedReference::new(&global.root_element, p);

        if let Type::Function { return_type, args } = &x.property_type {
            let function_index = functions.len();
            mapping.property_mapping.insert(
                nr.clone(),
                PropertyReference::Function { sub_component_path: vec![], function_index },
            );
            state.global_properties.insert(
                nr.clone(),
                PropertyReference::GlobalFunction { global_index, function_index },
            );
            functions.push(Function {
                name: p.into(),
                ret_ty: (**return_type).clone(),
                args: args.clone(),
                // will be replaced later
                code: super::Expression::CodeBlock(vec![]),
            });
            continue;
        }

        mapping.property_mapping.insert(
            nr.clone(),
            PropertyReference::Local { sub_component_path: vec![], property_index },
        );

        properties.push(Property {
            name: p.clone(),
            ty: x.property_type.clone(),
            ..Property::default()
        });
        if !matches!(x.property_type, Type::Callback { .. }) {
            const_properties.push(nr.is_constant());
        } else {
            const_properties.push(false);
        }
        prop_analysis.push(
            global
                .root_element
                .borrow()
                .property_analysis
                .borrow()
                .get(p)
                .cloned()
                .unwrap_or_default(),
        );
        state
            .global_properties
            .insert(nr.clone(), PropertyReference::Global { global_index, property_index });
    }

    let mut init_values = vec![None; properties.len()];

    let ctx = ExpressionContext { mapping: &mapping, state, parent: None, component: global };
    for (prop, binding) in &global.root_element.borrow().bindings {
        assert!(binding.borrow().two_way_bindings.is_empty());
        assert!(binding.borrow().animation.is_none());
        let expression =
            super::lower_expression::lower_expression(&binding.borrow().expression, &ctx);

        let nr = NamedReference::new(&global.root_element, prop);
        let property_index = match mapping.property_mapping[&nr] {
            PropertyReference::Local { property_index, .. } => property_index,
            PropertyReference::Function { ref sub_component_path, function_index } => {
                assert!(sub_component_path.is_empty());
                functions[function_index].code = expression;
                continue;
            }
            _ => unreachable!(),
        };
        let is_constant = binding.borrow().analysis.as_ref().map_or(false, |a| a.is_const);
        init_values[property_index] = Some(BindingExpression {
            expression: expression.into(),
            animation: None,
            is_constant,
            is_state_info: false,
            use_count: 0.into(),
        });
    }

    let is_builtin = if let Some(builtin) = global.root_element.borrow().native_class() {
        // We just generate the property so we know how to address them
        for (p, x) in &builtin.properties {
            let property_index = properties.len();
            properties.push(Property { name: p.clone(), ty: x.ty.clone(), ..Property::default() });
            let nr = NamedReference::new(&global.root_element, p);
            state
                .global_properties
                .insert(nr, PropertyReference::Global { global_index, property_index });
            prop_analysis.push(
                global
                    .root_element
                    .borrow()
                    .property_analysis
                    .borrow()
                    .get(p)
                    .cloned()
                    .unwrap_or_default(),
            );
        }
        true
    } else {
        false
    };

    let public_properties = public_properties(global, &mapping, state);
    GlobalComponent {
        name: global.root_element.borrow().id.clone(),
        properties,
        functions,
        init_values,
        const_properties,
        public_properties,
        private_properties: global.private_properties.borrow().clone(),
        exported: !global.exported_global_names.borrow().is_empty(),
        aliases: global.global_aliases(),
        is_builtin,
        prop_analysis,
    }
}

fn make_tree(
    state: &LoweringState,
    element: &ElementRc,
    component: &LoweredSubComponent,
    sub_component_path: &[usize],
) -> TreeNode {
    let e = element.borrow();
    let children = e.children.iter().map(|c| make_tree(state, c, component, sub_component_path));
    let repeater_count = component.mapping.repeater_count;
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
            tree_node.is_accessible |= !e.accessibility_props.0.is_empty();
            tree_node
        }
        LoweredElement::NativeItem { item_index } => TreeNode {
            is_accessible: !e.accessibility_props.0.is_empty(),
            sub_component_path: sub_component_path.into(),
            item_index: *item_index,
            children: children.collect(),
            repeated: false,
            component_container: false,
        },
        LoweredElement::Repeated { repeated_index } => TreeNode {
            is_accessible: false,
            sub_component_path: sub_component_path.into(),
            item_index: *repeated_index,
            children: vec![],
            repeated: true,
            component_container: false,
        },
        LoweredElement::ComponentPlaceholder { repeated_index } => TreeNode {
            is_accessible: false,
            sub_component_path: sub_component_path.into(),
            item_index: *repeated_index + repeater_count,
            children: vec![],
            repeated: false,
            component_container: true,
        },
    }
}

fn public_properties(
    component: &Component,
    mapping: &LoweredSubComponentMapping,
    state: &LoweringState,
) -> PublicProperties {
    component
        .root_element
        .borrow()
        .property_declarations
        .iter()
        .filter(|(_, c)| c.expose_in_public_api)
        .map(|(p, c)| {
            let property_reference = mapping
                .map_property_reference(&NamedReference::new(&component.root_element, p), state);
            PublicProperty {
                name: p.clone(),
                ty: c.property_type.clone(),
                prop: property_reference,
                read_only: c.visibility == PropertyVisibility::Output,
            }
        })
        .collect()
}
