// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use by_address::ByAddress;

use super::lower_expression::{ExpressionLoweringCtx, ExpressionLoweringCtxInner};
use crate::expression_tree::Expression as tree_Expression;
use crate::langtype::{ElementType, Struct, Type};
use crate::llr::item_tree::*;
use crate::namedreference::NamedReference;
use crate::object_tree::{self, Component, ElementRc, PropertyAnalysis, PropertyVisibility};
use crate::CompilerConfiguration;
use smol_str::{format_smolstr, SmolStr};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use typed_index_collections::TiVec;

pub fn lower_to_item_tree(
    document: &crate::object_tree::Document,
    compiler_config: &CompilerConfiguration,
) -> std::io::Result<CompilationUnit> {
    let mut state = LoweringState::default();

    #[cfg(feature = "bundle-translations")]
    {
        state.translation_builder = document.translation_builder.clone();
    }

    let mut globals = TiVec::new();
    for g in &document.used_types.borrow().globals {
        let count = globals.next_key();
        globals.push(lower_global(g, count, &mut state));
    }
    for (g, l) in document.used_types.borrow().globals.iter().zip(&mut globals) {
        lower_global_expressions(g, &mut state, l);
    }

    for c in &document.used_types.borrow().sub_components {
        let sc = lower_sub_component(c, &mut state, None, compiler_config);
        let idx = state.push_sub_component(sc);
        state.sub_component_mapping.insert(ByAddress(c.clone()), idx);
    }

    let public_components = document
        .exported_roots()
        .map(|component| {
            let mut sc = lower_sub_component(&component, &mut state, None, compiler_config);
            let public_properties = public_properties(&component, &sc.mapping, &state);
            sc.sub_component.name = component.id.clone();
            let item_tree = ItemTree {
                tree: make_tree(&state, &component.root_element, &sc, &[]),
                root: state.push_sub_component(sc),
                parent_context: None,
            };
            // For C++ codegen, the root component must have the same name as the public component
            PublicComponent {
                item_tree,
                public_properties,
                private_properties: component.private_properties.borrow().clone(),
                name: component.id.clone(),
            }
        })
        .collect();

    let popup_menu = document.popup_menu_impl.as_ref().map(|c| {
        let sc = lower_sub_component(c, &mut state, None, compiler_config);
        let sub_menu = sc.mapping.map_property_reference(
            &NamedReference::new(&c.root_element, SmolStr::new_static("sub-menu")),
            &state,
        );
        let activated = sc.mapping.map_property_reference(
            &NamedReference::new(&c.root_element, SmolStr::new_static("activated")),
            &state,
        );
        let close = sc.mapping.map_property_reference(
            &NamedReference::new(&c.root_element, SmolStr::new_static("close")),
            &state,
        );
        let entries = sc.mapping.map_property_reference(
            &NamedReference::new(&c.root_element, SmolStr::new_static("entries")),
            &state,
        );
        let item_tree = ItemTree {
            tree: make_tree(&state, &c.root_element, &sc, &[]),
            root: state.push_sub_component(sc),
            parent_context: None,
        };
        PopupMenu { item_tree, sub_menu, activated, close, entries }
    });

    let root = CompilationUnit {
        public_components,
        globals,
        sub_components: state.sub_components.into_iter().map(|sc| sc.sub_component).collect(),
        used_sub_components: document
            .used_types
            .borrow()
            .sub_components
            .iter()
            .map(|tree_sub_compo| state.sub_component_mapping[&ByAddress(tree_sub_compo.clone())])
            .collect(),
        has_debug_info: compiler_config.debug_info,
        popup_menu,
        #[cfg(feature = "bundle-translations")]
        translations: state.translation_builder.map(|x| x.result()),
    };
    super::optim_passes::run_passes(&root);
    Ok(root)
}

#[derive(Debug, Clone)]
pub enum LoweredElement {
    SubComponent { sub_component_index: SubComponentInstanceIdx },
    NativeItem { item_index: ItemInstanceIdx },
    Repeated { repeated_index: RepeatedElementIdx },
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
                            from.name().clone(),
                        )),
                        *sub_component_index,
                    );
                }
                unreachable!()
            }
            LoweredElement::NativeItem { item_index } => PropertyReference::InNativeItem {
                sub_component_path: vec![],
                item_index: *item_index,
                prop_name: from.name().to_string(),
            },
            LoweredElement::Repeated { .. } => unreachable!(),
            LoweredElement::ComponentPlaceholder { .. } => unreachable!(),
        }
    }
}

pub struct LoweredSubComponent {
    sub_component: SubComponent,
    mapping: LoweredSubComponentMapping,
}

#[derive(Default)]
pub struct LoweringState {
    global_properties: HashMap<NamedReference, PropertyReference>,
    sub_components: TiVec<SubComponentIdx, LoweredSubComponent>,
    pub sub_component_mapping: HashMap<ByAddress<Rc<Component>>, SubComponentIdx>,
    #[cfg(feature = "bundle-translations")]
    pub translation_builder: Option<crate::translations::TranslationsBuilder>,
}

impl LoweringState {
    pub fn map_property_reference(&self, from: &NamedReference) -> PropertyReference {
        if let Some(x) = self.global_properties.get(from) {
            return x.clone();
        }

        let element = from.element();
        let sc = self.sub_component(&element.borrow().enclosing_component.upgrade().unwrap());
        sc.mapping.map_property_reference(from, self)
    }

    fn sub_component<'a>(&'a self, component: &Rc<Component>) -> &'a LoweredSubComponent {
        &self.sub_components[self.sub_component_idx(component)]
    }

    fn sub_component_idx(&self, component: &Rc<Component>) -> SubComponentIdx {
        self.sub_component_mapping[&ByAddress(component.clone())]
    }

    fn push_sub_component(&mut self, sc: LoweredSubComponent) -> SubComponentIdx {
        self.sub_components.push_and_get_key(sc)
    }
}

// Map a PropertyReference within a `sub_component` to a PropertyReference to the component containing it
fn property_reference_within_sub_component(
    mut prop_ref: PropertyReference,
    sub_component: SubComponentInstanceIdx,
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

fn component_id(component: &Rc<Component>) -> SmolStr {
    if component.is_global() {
        component.root_element.borrow().id.clone()
    } else if component.id.is_empty() {
        format_smolstr!("Component_{}", component.root_element.borrow().id)
    } else {
        format_smolstr!("{}_{}", component.id, component.root_element.borrow().id)
    }
}

fn lower_sub_component(
    component: &Rc<Component>,
    state: &mut LoweringState,
    parent_context: Option<&ExpressionLoweringCtxInner>,
    compiler_config: &CompilerConfiguration,
) -> LoweredSubComponent {
    let mut sub_component = SubComponent {
        name: component_id(component),
        properties: Default::default(),
        functions: Default::default(),
        items: Default::default(),
        repeated: Default::default(),
        component_containers: Default::default(),
        popup_windows: Default::default(),
        menu_item_trees: Vec::new(),
        timers: Default::default(),
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
        element_infos: Default::default(),
        prop_analysis: Default::default(),
    };
    let mut mapping = LoweredSubComponentMapping::default();
    let mut repeated = TiVec::new();
    let mut accessible_prop = Vec::new();
    let mut change_callbacks = Vec::new();

    if let Some(parent) = component.parent_element.upgrade() {
        // Add properties for the model data and index
        if parent.borrow().repeated.as_ref().is_some_and(|x| !x.is_conditional_element) {
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
            if let Type::Function(function) = &x.property_type {
                // TODO: Function could wrap the Rc<langtype::Function>
                //       instead of cloning the return type and args?
                let function_index = sub_component.functions.push_and_get_key(Function {
                    name: p.clone(),
                    ret_ty: function.return_type.clone(),
                    args: function.args.clone(),
                    // will be replaced later
                    code: super::Expression::CodeBlock(vec![]),
                });
                mapping.property_mapping.insert(
                    NamedReference::new(element, p.clone()),
                    PropertyReference::Function { sub_component_path: vec![], function_index },
                );
                continue;
            }
            let property_index = sub_component.properties.push_and_get_key(Property {
                name: format_smolstr!("{}_{}", elem.id, p),
                ty: x.property_type.clone(),
                ..Property::default()
            });
            mapping.property_mapping.insert(
                NamedReference::new(element, p.clone()),
                PropertyReference::Local { sub_component_path: vec![], property_index },
            );
        }
        if elem.repeated.is_some() {
            let parent = if elem.is_component_placeholder { parent.clone() } else { None };

            mapping.element_mapping.insert(
                element.clone().into(),
                LoweredElement::Repeated {
                    repeated_index: repeated.push_and_get_key((element.clone(), parent)),
                },
            );
            mapping.repeater_count += 1;
            return None;
        }
        match &elem.base_type {
            ElementType::Component(comp) => {
                let ty = state.sub_component_idx(comp);
                let sub_component_index =
                    sub_component.sub_components.push_and_get_key(SubComponentInstance {
                        ty,
                        name: elem.id.clone(),
                        index_in_tree: *elem.item_index.get().unwrap(),
                        index_of_first_child_in_tree: *elem
                            .item_index_of_first_children
                            .get()
                            .unwrap(),
                        repeater_offset,
                    });
                mapping.element_mapping.insert(
                    element.clone().into(),
                    LoweredElement::SubComponent { sub_component_index },
                );
                repeater_offset += comp.repeater_count();
            }

            ElementType::Native(n) => {
                let item_index = sub_component.items.push_and_get_key(Item {
                    ty: n.clone(),
                    name: elem.id.clone(),
                    index_in_tree: *elem.item_index.get().unwrap(),
                });
                mapping
                    .element_mapping
                    .insert(element.clone().into(), LoweredElement::NativeItem { item_index });
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
            change_callbacks
                .push((NamedReference::new(element, prop.clone()), expr.borrow().clone()));
        }

        if compiler_config.debug_info {
            let element_infos = elem.element_infos();
            if !element_infos.is_empty() {
                sub_component.element_infos.insert(*elem.item_index.get().unwrap(), element_infos);
            }
        }

        Some(element.clone())
    });

    let inner = ExpressionLoweringCtxInner { mapping: &mapping, parent: parent_context, component };
    let mut ctx = ExpressionLoweringCtx { inner, state };

    crate::generator::handle_property_bindings_init(component, |e, p, binding| {
        let nr = NamedReference::new(e, p.clone());
        let prop = ctx.map_property_reference(&nr);

        if let Type::Function { .. } = nr.ty() {
            if let PropertyReference::Function { sub_component_path, function_index } = prop {
                assert!(sub_component_path.is_empty());
                sub_component.functions[function_index].code =
                    super::lower_expression::lower_expression(&binding.expression, &mut ctx);
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
                super::lower_expression::lower_expression(&binding.expression, &mut ctx).into();

            let is_constant = binding.analysis.as_ref().is_some_and(|a| a.is_const);
            let animation = binding
                .animation
                .as_ref()
                .filter(|_| !is_constant)
                .map(|a| super::lower_expression::lower_animation(a, &mut ctx));

            sub_component.prop_analysis.insert(
                prop.clone(),
                PropAnalysis {
                    property_init: Some(sub_component.property_init.len()),
                    analysis: get_property_analysis(e, p),
                },
            );

            let is_state_info = matches!(
                e.borrow().lookup_property(p).property_type,
                Type::Struct(s) if s.name.as_ref().is_some_and(|name| name.ends_with("::StateInfo"))
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
            .is_none_or(|a| a.is_set || a.is_set_externally)
        {
            if let Some(anim) = binding.animation.as_ref() {
                match super::lower_expression::lower_animation(anim, &mut ctx) {
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
    sub_component.repeated = repeated
        .into_iter()
        .map(|(elem, parent)| {
            lower_repeated_component(&elem, parent, &sub_component, &mut ctx, compiler_config)
        })
        .collect();
    for s in &mut sub_component.sub_components {
        s.repeater_offset +=
            (sub_component.repeated.len() + sub_component.component_containers.len()) as u32;
    }

    sub_component.popup_windows = component
        .popup_windows
        .borrow()
        .iter()
        .map(|popup| lower_popup_component(popup, &mut ctx, compiler_config))
        .collect();

    sub_component.menu_item_trees = component
        .menu_item_tree
        .borrow()
        .iter()
        .map(|c| {
            let sc = lower_sub_component(c, ctx.state, Some(&ctx.inner), compiler_config);
            ItemTree {
                tree: make_tree(ctx.state, &c.root_element, &sc, &[]),
                root: ctx.state.push_sub_component(sc),
                parent_context: None,
            }
        })
        .collect();

    sub_component.timers = component.timers.borrow().iter().map(|t| lower_timer(t, &ctx)).collect();

    crate::generator::for_each_const_properties(component, |elem, n| {
        let x = ctx.map_property_reference(&NamedReference::new(elem, n.clone()));
        // ensure that all const properties have analysis
        sub_component.prop_analysis.entry(x.clone()).or_insert_with(|| PropAnalysis {
            property_init: None,
            analysis: get_property_analysis(elem, n),
        });
        sub_component.const_properties.push(x);
    });

    sub_component.init_code = component
        .init_code
        .borrow()
        .iter()
        .map(|e| super::lower_expression::lower_expression(e, &mut ctx).into())
        .collect();

    sub_component.layout_info_h = super::lower_expression::get_layout_info(
        &component.root_element,
        &mut ctx,
        &component.root_constraints.borrow(),
        crate::layout::Orientation::Horizontal,
    )
    .into();
    sub_component.layout_info_v = super::lower_expression::get_layout_info(
        &component.root_element,
        &mut ctx,
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
                Type::Callback(callback) => super::Expression::CallBackCall {
                    callback: prop,
                    arguments: (0..callback.args.len())
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
            let expr = super::lower_expression::lower_expression(
                &tree_Expression::CodeBlock(exprs),
                &mut ctx,
            );
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

    LoweredSubComponent { sub_component, mapping }
}

fn lower_geometry(
    geom: &crate::object_tree::GeometryProps,
    ctx: &ExpressionLoweringCtx<'_>,
) -> super::Expression {
    let mut fields = BTreeMap::default();
    let mut values = BTreeMap::default();
    for (f, v) in [("x", &geom.x), ("y", &geom.y), ("width", &geom.width), ("height", &geom.height)]
    {
        fields.insert(f.into(), Type::LogicalLength);
        values
            .insert(f.into(), super::Expression::PropertyReference(ctx.map_property_reference(v)));
    }
    super::Expression::Struct {
        ty: Rc::new(Struct { fields, name: None, node: None, rust_attributes: None }),
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
                if n.properties.get(p).is_some_and(|p| p.is_native_output()) {
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

fn lower_repeated_component(
    elem: &ElementRc,
    parent_component_container: Option<ElementRc>,
    sub_component: &SubComponent,
    ctx: &mut ExpressionLoweringCtx,
    compiler_config: &CompilerConfiguration,
) -> RepeatedElement {
    let e = elem.borrow();
    let component = e.base_type.as_component().clone();
    let repeated = e.repeated.as_ref().unwrap();

    let sc = lower_sub_component(&component, ctx.state, Some(&ctx.inner), compiler_config);

    let listview = repeated.is_listview.as_ref().map(|lv| {
        let geom = component.root_element.borrow().geometry_props.clone().unwrap();
        ListViewInfo {
            viewport_y: ctx.map_property_reference(&lv.viewport_y),
            viewport_height: ctx.map_property_reference(&lv.viewport_height),
            viewport_width: ctx.map_property_reference(&lv.viewport_width),
            listview_height: ctx.map_property_reference(&lv.listview_height),
            listview_width: ctx.map_property_reference(&lv.listview_width),
            prop_y: sc.mapping.map_property_reference(&geom.y, ctx.state),
            prop_height: sc.mapping.map_property_reference(&geom.height, ctx.state),
        }
    });

    let parent_index = parent_component_container.map(|p| *p.borrow().item_index.get().unwrap());
    let container_item_index =
        parent_index.and_then(|pii| sub_component.items.position(|i| i.index_in_tree == pii));

    RepeatedElement {
        model: super::lower_expression::lower_expression(&repeated.model, ctx).into(),
        sub_tree: ItemTree {
            tree: make_tree(ctx.state, &component.root_element, &sc, &[]),
            root: ctx.state.push_sub_component(sc),
            parent_context: Some(e.enclosing_component.upgrade().unwrap().id.clone()),
        },
        index_prop: (!repeated.is_conditional_element).then_some(1usize.into()),
        data_prop: (!repeated.is_conditional_element).then_some(0usize.into()),
        index_in_tree: *e.item_index.get().unwrap(),
        listview,
        container_item_index,
    }
}

fn lower_popup_component(
    popup: &object_tree::PopupWindow,
    ctx: &mut ExpressionLoweringCtx,
    compiler_config: &CompilerConfiguration,
) -> PopupWindow {
    let sc = lower_sub_component(&popup.component, ctx.state, Some(&ctx.inner), compiler_config);
    use super::Expression::PropertyReference as PR;
    let position = super::lower_expression::make_struct(
        "LogicalPosition",
        [
            ("x", Type::LogicalLength, PR(sc.mapping.map_property_reference(&popup.x, ctx.state))),
            ("y", Type::LogicalLength, PR(sc.mapping.map_property_reference(&popup.y, ctx.state))),
        ],
    );

    let item_tree = ItemTree {
        tree: make_tree(ctx.state, &popup.component.root_element, &sc, &[]),
        root: ctx.state.push_sub_component(sc),
        parent_context: Some(
            popup
                .component
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
    };
    PopupWindow { item_tree, position: position.into() }
}

fn lower_timer(timer: &object_tree::Timer, ctx: &ExpressionLoweringCtx) -> Timer {
    Timer {
        interval: super::Expression::PropertyReference(ctx.map_property_reference(&timer.interval))
            .into(),
        running: super::Expression::PropertyReference(ctx.map_property_reference(&timer.running))
            .into(),
        // TODO: this calls a callback instead of inlining the callback code directly
        triggered: super::Expression::CallBackCall {
            callback: ctx.map_property_reference(&timer.triggered),
            arguments: vec![],
        }
        .into(),
    }
}

/// Lower the globals (but not their expressions as we first need to lower all the global to get proper mapping in the state)
fn lower_global(
    global: &Rc<Component>,
    global_index: GlobalIdx,
    state: &mut LoweringState,
) -> GlobalComponent {
    let mut properties = TiVec::new();
    let mut const_properties = TiVec::new();
    let mut prop_analysis = TiVec::new();
    let mut functions = TiVec::new();

    for (p, x) in &global.root_element.borrow().property_declarations {
        if x.is_alias.is_some() {
            continue;
        }
        let nr = NamedReference::new(&global.root_element, p.clone());

        if let Type::Function(function) = &x.property_type {
            // TODO: wrap the Rc<langtype::Function> instead of cloning
            let function_index = functions.push_and_get_key(Function {
                name: p.clone(),
                ret_ty: function.return_type.clone(),
                args: function.args.clone(),
                // will be replaced later
                code: super::Expression::CodeBlock(vec![]),
            });
            state.global_properties.insert(
                nr.clone(),
                PropertyReference::GlobalFunction { global_index, function_index },
            );
            continue;
        }

        let property_index = properties.push_and_get_key(Property {
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

    let is_builtin = if let Some(builtin) = global.root_element.borrow().native_class() {
        // We just generate the property so we know how to address them
        for (p, x) in &builtin.properties {
            let property_index = properties.push_and_get_key(Property {
                name: p.clone(),
                ty: x.ty.clone(),
                ..Property::default()
            });
            let nr = NamedReference::new(&global.root_element, p.clone());
            state
                .global_properties
                .insert(nr, PropertyReference::Global { global_index, property_index });
            prop_analysis.push(PropertyAnalysis {
                // Assume that a builtin global property can always be set from the builtin code
                is_set_externally: true,
                ..global
                    .root_element
                    .borrow()
                    .property_analysis
                    .borrow()
                    .get(p)
                    .cloned()
                    .unwrap_or_default()
            });
        }
        true
    } else {
        false
    };

    GlobalComponent {
        name: global.root_element.borrow().id.clone(),
        init_values: typed_index_collections::ti_vec![None; properties.len()],
        properties,
        functions,
        change_callbacks: BTreeMap::new(),
        const_properties,
        public_properties: Default::default(),
        private_properties: global.private_properties.borrow().clone(),
        exported: !global.exported_global_names.borrow().is_empty(),
        aliases: global.global_aliases(),
        is_builtin,
        prop_analysis,
    }
}

fn lower_global_expressions(
    global: &Rc<Component>,
    state: &mut LoweringState,
    lowered: &mut GlobalComponent,
) {
    // Note that this mapping doesn't contain anything useful, everything is in the state
    let mapping = LoweredSubComponentMapping::default();
    let inner = ExpressionLoweringCtxInner { mapping: &mapping, parent: None, component: global };
    let mut ctx = ExpressionLoweringCtx { inner, state };

    for (prop, binding) in &global.root_element.borrow().bindings {
        assert!(binding.borrow().two_way_bindings.is_empty());
        assert!(binding.borrow().animation.is_none());
        let expression =
            super::lower_expression::lower_expression(&binding.borrow().expression, &mut ctx);

        let nr = NamedReference::new(&global.root_element, prop.clone());
        let property_index = match ctx.state.global_properties[&nr] {
            PropertyReference::Global { property_index, .. } => property_index,
            PropertyReference::GlobalFunction { function_index, .. } => {
                lowered.functions[function_index].code = expression;
                continue;
            }
            _ => unreachable!(),
        };
        let is_constant = binding.borrow().analysis.as_ref().is_some_and(|a| a.is_const);
        lowered.init_values[property_index] = Some(BindingExpression {
            expression: expression.into(),
            animation: None,
            is_constant,
            is_state_info: false,
            use_count: 0.into(),
        });
    }

    for (prop, expr) in &global.root_element.borrow().change_callbacks {
        let nr = NamedReference::new(&global.root_element, prop.clone());
        let property_index = match ctx.state.global_properties[&nr] {
            PropertyReference::Global { property_index, .. } => property_index,
            _ => unreachable!(),
        };
        let expression = super::lower_expression::lower_expression(
            &tree_Expression::CodeBlock(expr.borrow().clone()),
            &mut ctx,
        );
        lowered.change_callbacks.insert(property_index, expression.into());
    }

    lowered.public_properties = public_properties(global, &mapping, state);
}

fn make_tree(
    state: &LoweringState,
    element: &ElementRc,
    component: &LoweredSubComponent,
    sub_component_path: &[SubComponentInstanceIdx],
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
            item_index: itertools::Either::Left(*item_index),
            children: children.collect(),
        },
        LoweredElement::Repeated { repeated_index } => TreeNode {
            is_accessible: false,
            sub_component_path: sub_component_path.into(),
            item_index: itertools::Either::Right(usize::from(*repeated_index) as u32),
            children: vec![],
        },
        LoweredElement::ComponentPlaceholder { repeated_index } => TreeNode {
            is_accessible: false,
            sub_component_path: sub_component_path.into(),
            item_index: itertools::Either::Right(*repeated_index + repeater_count),
            children: vec![],
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
            let property_reference = mapping.map_property_reference(
                &NamedReference::new(&component.root_element, p.clone()),
                state,
            );
            PublicProperty {
                name: p.clone(),
                ty: c.property_type.clone(),
                prop: property_reference,
                read_only: c.visibility == PropertyVisibility::Output,
            }
        })
        .collect()
}
