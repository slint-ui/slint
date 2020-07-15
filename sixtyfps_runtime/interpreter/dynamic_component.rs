use crate::{dynamic_type, eval};

use core::convert::TryInto;
use core::ptr::NonNull;
use dynamic_type::{Instance, InstanceBox};
use object_tree::{Element, ElementRc};
use sixtyfps_compilerlib::typeregister::Type;
use sixtyfps_compilerlib::*;
use sixtyfps_corelib::abi::datastructures::{
    ComponentVTable, ItemTreeNode, ItemVTable, ItemVisitorRefMut, Resource, WindowProperties,
};
use sixtyfps_corelib::abi::primitives::PropertyAnimation;
use sixtyfps_corelib::abi::{properties::PropertyListenerScope, slice::Slice};
use sixtyfps_corelib::rtti::PropertyInfo;
use sixtyfps_corelib::ComponentRefPin;
use sixtyfps_corelib::{rtti, Color, Property, SharedString, Signal};
use std::collections::HashMap;
use std::{pin::Pin, rc::Rc};

pub struct ComponentBox {
    instance: InstanceBox,
    component_type: Rc<ComponentDescription>,
}

impl ComponentBox {
    /// Borrow this component as a `Pin<ComponentRef>`
    pub fn borrow(&self) -> ComponentRefPin {
        unsafe {
            Pin::new_unchecked(vtable::VRef::from_raw(
                NonNull::from(&self.component_type.ct).cast(),
                self.instance.as_ptr().cast(),
            ))
        }
    }

    /// Borrow this component as a `Pin<ComponentRefMut>`
    pub fn borrow_mut(&mut self) -> Pin<sixtyfps_corelib::abi::datastructures::ComponentRefMut> {
        unsafe {
            Pin::new_unchecked(vtable::VRefMut::from_raw(
                NonNull::from(&self.component_type.ct).cast(),
                self.instance.as_ptr().cast(),
            ))
        }
    }

    pub fn description(&self) -> Rc<ComponentDescription> {
        return self.component_type.clone();
    }

    pub fn window_properties<'a>(&'a self) -> WindowProperties<'a> {
        let component = self.borrow();
        let component_type = unsafe {
            &*(component.get_vtable() as *const ComponentVTable as *const ComponentDescription)
        };

        let info = &component_type.items[component_type.original.root_element.borrow().id.as_str()];

        let get_prop = |name| {
            if info.elem.borrow().lookup_property(name) != Type::Length {
                None
            } else {
                info.rtti.properties.get(name).map(|p| unsafe {
                    &*(component.as_ptr().add(info.offset).add(p.offset()) as *const Property<f32>)
                })
            }
        };

        WindowProperties { width: get_prop("width"), height: get_prop("height") }
    }
}

pub(crate) struct ItemWithinComponent {
    offset: usize,
    pub(crate) rtti: Rc<ItemRTTI>,
    elem: ElementRc,
}

impl ItemWithinComponent {
    pub(crate) unsafe fn item_from_component(
        &self,
        mem: *const u8,
    ) -> Pin<vtable::VRef<ItemVTable>> {
        Pin::new_unchecked(vtable::VRef::from_raw(
            NonNull::from(self.rtti.vtable),
            NonNull::new(mem.add(self.offset) as _).unwrap(),
        ))
    }
}

pub(crate) struct PropertiesWithinComponent {
    pub(crate) offset: usize,
    pub(crate) prop: Box<dyn PropertyInfo<u8, eval::Value>>,
}

pub(crate) struct RepeaterWithinComponent {
    /// The component description of the items to repeat
    pub(crate) component_to_repeat: Rc<ComponentDescription>,
    /// Offset of the `Vec<ComponentBox>`
    pub(crate) offset: usize,
    /// The model
    pub(crate) model: expression_tree::Expression,
    /// Offset of the PropertyListenerScope
    listener: Option<usize>,
}

type RepeaterVec = Vec<ComponentBox>;

/// ComponentDescription is a representation of a component suitable for interpretation
///
/// It contains information about how to create and destroy the Component.
/// Its first member is the ComponentVTable for this component, since it is a `#[repr(C)]`
/// structure, it is valid to cast a pointer to the ComponentVTable back to a
/// ComponentDescription to access the extra field that are needed at runtime
#[repr(C)]
pub struct ComponentDescription {
    pub(crate) ct: ComponentVTable,
    dynamic_type: Rc<dynamic_type::TypeInfo>,
    it: Vec<ItemTreeNode<crate::dynamic_type::Instance>>,
    pub(crate) items: HashMap<String, ItemWithinComponent>,
    pub(crate) custom_properties: HashMap<String, PropertiesWithinComponent>,
    /// the usize is the offset within `mem` to the Signal<()>
    pub(crate) custom_signals: HashMap<String, usize>,
    /// The repeaters
    pub(crate) repeater: Vec<RepeaterWithinComponent>,
    /// Offset to a Option<ComponentPinRef>
    pub(crate) parent_component_offset: Option<usize>,
    /// Keep the Rc alive
    pub(crate) original: Rc<object_tree::Component>,
}

unsafe extern "C" fn visit_children_item(
    component: ComponentRefPin,
    index: isize,
    v: ItemVisitorRefMut,
) {
    let component_type =
        &*(component.get_vtable() as *const ComponentVTable as *const ComponentDescription);
    let item_tree = &component_type.it;
    sixtyfps_corelib::item_tree::visit_item_tree(
        Pin::new_unchecked(&*(component.as_ptr() as *const Instance)),
        component,
        item_tree.as_slice().into(),
        index,
        v,
        |_, mut visitor, index| {
            let rep_in_comp = &component_type.repeater[index];
            let vec = &mut *(component.as_ptr().add(rep_in_comp.offset) as *mut RepeaterVec);
            if let Some(listener_offset) = rep_in_comp.listener {
                let listener = Pin::new_unchecked(
                    &*(component.as_ptr().add(listener_offset) as *const PropertyListenerScope),
                );
                if listener.is_dirty() {
                    listener.evaluate(|| {
                        match eval::eval_expression(&rep_in_comp.model, &*component_type, component)
                        {
                            crate::Value::Number(count) => populate_model(
                                vec,
                                rep_in_comp,
                                component,
                                (0..count as i32)
                                    .into_iter()
                                    .map(|v| crate::Value::Number(v as f64)),
                            ),
                            crate::Value::Array(a) => {
                                populate_model(vec, rep_in_comp, component, a.into_iter())
                            }
                            crate::Value::Bool(b) => populate_model(
                                vec,
                                rep_in_comp,
                                component,
                                (if b { Some(crate::Value::Void) } else { None }).into_iter(),
                            ),
                            _ => panic!("Unsupported model"),
                        }
                    });
                }
            }
            for x in vec {
                x.borrow().as_ref().visit_children_item(-1, visitor.borrow_mut());
            }
        },
    );
}

/// Information attached to a builtin item
pub(crate) struct ItemRTTI {
    vtable: &'static ItemVTable,
    type_info: dynamic_type::StaticTypeInfo,
    pub(crate) properties: HashMap<&'static str, Box<dyn eval::ErasedPropertyInfo>>,
    /// The uszie is an offset within this item to the Signal.
    /// Ideally, we would need a vtable::VFieldOffset<ItemVTable, corelib::Signal<()>>
    pub(crate) signals: HashMap<&'static str, usize>,
}

fn rtti_for<T: 'static + Default + rtti::BuiltinItem + vtable::HasStaticVTable<ItemVTable>>(
) -> (&'static str, Rc<ItemRTTI>) {
    (
        T::name(),
        Rc::new(ItemRTTI {
            vtable: T::static_vtable(),
            type_info: dynamic_type::StaticTypeInfo::new::<T>(),
            properties: T::properties()
                .into_iter()
                .map(|(k, v)| (k, Box::new(v) as Box<dyn eval::ErasedPropertyInfo>))
                .collect(),
            signals: T::signals().into_iter().map(|(k, v)| (k, v.get_byte_offset())).collect(),
        }),
    )
}

/// Create a ComponentDescription from a source.
/// The path corresponding to the source need to be passed as well (path is used for diagnostics
/// and loading relative assets)
pub fn load(
    source: String,
    path: &std::path::Path,
) -> Result<Rc<ComponentDescription>, sixtyfps_compilerlib::diagnostics::Diagnostics> {
    let (syntax_node, mut diag) = parser::parse(source);
    diag.current_path = path.into();
    let tr = typeregister::TypeRegister::builtin();
    let tree = object_tree::Document::from_node(syntax_node.into(), &mut diag, &tr);
    if !diag.inner.is_empty() {
        return Err(diag);
    }
    let compiler_config = CompilerConfiguration::default();
    run_passes(&tree, &mut diag, &compiler_config);
    if !diag.inner.is_empty() {
        return Err(diag);
    }
    Ok(generate_component(&tree.root_component, &mut diag))
}

fn generate_component(
    root_component: &Rc<object_tree::Component>,
    diag: &mut diagnostics::Diagnostics,
) -> Rc<ComponentDescription> {
    let mut rtti = HashMap::new();
    {
        use sixtyfps_corelib::abi::primitives::*;
        rtti.extend(
            [
                rtti_for::<Image>(),
                rtti_for::<Text>(),
                rtti_for::<Rectangle>(),
                rtti_for::<TouchArea>(),
                rtti_for::<Path>(),
            ]
            .iter()
            .cloned(),
        );
    }
    let rtti = Rc::new(rtti);

    let mut tree_array = vec![];
    let mut items_types = HashMap::new();
    let mut builder = dynamic_type::TypeBuilder::new();

    let mut repeater = vec![];

    generator::build_array_helper(root_component, |rc_item, child_offset| {
        let item = rc_item.borrow();
        if let Some(repeated) = &item.repeated {
            tree_array.push(ItemTreeNode::DynamicTree { index: repeater.len() });
            let base_component = match &item.base_type {
                Type::Component(c) => c,
                _ => panic!("should be a component because of the repeater_component pass"),
            };
            repeater.push(RepeaterWithinComponent {
                component_to_repeat: generate_component(base_component, diag),
                offset: builder.add_field_type::<RepeaterVec>(),
                model: repeated.model.clone(),
                listener: if repeated.model.is_constant() {
                    None
                } else {
                    Some(builder.add_field_type::<PropertyListenerScope>())
                },
            });
        } else {
            let rt = &rtti[&*item.base_type.as_builtin().class_name];
            let offset = builder.add_field(rt.type_info);
            tree_array.push(ItemTreeNode::Item {
                item: unsafe { vtable::VOffset::from_raw(rt.vtable, offset) },
                children_index: child_offset,
                chilren_count: item.children.len() as _,
            });
            items_types.insert(
                item.id.clone(),
                ItemWithinComponent { offset, rtti: rt.clone(), elem: rc_item.clone() },
            );
        }
    });

    let mut custom_properties = HashMap::new();
    let mut custom_signals = HashMap::new();
    fn property_info<T: Clone + Default + 'static>(
    ) -> (Box<dyn PropertyInfo<u8, eval::Value>>, dynamic_type::StaticTypeInfo)
    where
        T: std::convert::TryInto<eval::Value>,
        eval::Value: std::convert::TryInto<T>,
    {
        // Fixme: using u8 in PropertyInfo<> is not sound, we would need to materialize a type for out component
        (
            Box::new(unsafe {
                vtable::FieldOffset::<u8, Property<T>, _>::new_from_offset_pinned(0)
            }),
            dynamic_type::StaticTypeInfo::new::<Property<T>>(),
        )
    }
    fn animated_property_info<
        T: Clone + Default + sixtyfps_corelib::abi::properties::InterpolatedPropertyValue + 'static,
    >() -> (Box<dyn PropertyInfo<u8, eval::Value>>, dynamic_type::StaticTypeInfo)
    where
        T: std::convert::TryInto<eval::Value>,
        eval::Value: std::convert::TryInto<T>,
    {
        // Fixme: using u8 in PropertyInfo<> is not sound, we would need to materialize a type for out component
        (
            Box::new(unsafe {
                rtti::MaybeAnimatedPropertyInfoWrapper(
                    vtable::FieldOffset::<u8, Property<T>, _>::new_from_offset_pinned(0),
                )
            }),
            dynamic_type::StaticTypeInfo::new::<Property<T>>(),
        )
    }

    for (name, decl) in &root_component.root_element.borrow().property_declarations {
        let (prop, type_info) = match decl.property_type {
            Type::Float32 => animated_property_info::<f32>(),
            Type::Int32 => animated_property_info::<i32>(),
            Type::String => property_info::<SharedString>(),
            Type::Color => animated_property_info::<Color>(),
            Type::Duration => animated_property_info::<i64>(),
            Type::Length => animated_property_info::<f32>(),
            Type::LogicalLength => animated_property_info::<f32>(),
            Type::Resource => property_info::<Resource>(),
            Type::Bool => property_info::<bool>(),
            Type::Signal => {
                custom_signals.insert(name.clone(), builder.add_field_type::<Signal<()>>());
                continue;
            }
            _ => panic!("bad type"),
        };
        custom_properties.insert(
            name.clone(),
            PropertiesWithinComponent { offset: builder.add_field(type_info), prop },
        );
    }
    if root_component.parent_element.upgrade().is_some() {
        let (prop, type_info) = property_info::<u32>();
        custom_properties.insert(
            "index".into(),
            PropertiesWithinComponent { offset: builder.add_field(type_info), prop },
        );
        // FIXME: make it a property for the correct type instead of being generic
        let (prop, type_info) = property_info::<eval::Value>();
        custom_properties.insert(
            "model_data".into(),
            PropertiesWithinComponent { offset: builder.add_field(type_info), prop },
        );
    } else {
        let (prop, type_info) = property_info::<f32>();
        custom_properties.insert(
            "dpi".into(),
            PropertiesWithinComponent { offset: builder.add_field(type_info), prop },
        );
    }

    let parent_component_offset = if root_component.parent_element.upgrade().is_some() {
        Some(builder.add_field_type::<Option<ComponentRefPin>>())
    } else {
        None
    };

    extern "C" fn layout_info(
        _: ComponentRefPin,
    ) -> sixtyfps_corelib::abi::datastructures::LayoutInfo {
        todo!()
    }

    let t = ComponentVTable { visit_children_item, layout_info, compute_layout };
    let t = ComponentDescription {
        ct: t,
        dynamic_type: builder.build(),
        it: tree_array,
        items: items_types,
        custom_properties,
        custom_signals,
        original: root_component.clone(),
        repeater,
        parent_component_offset,
    };

    Rc::new(t)
}

fn animation_for_property(
    component_type: Rc<ComponentDescription>,
    eval_context: ComponentRefPin,
    all_animations: &HashMap<String, ElementRc>,
    property_name: &String,
) -> Option<PropertyAnimation> {
    match all_animations.get(property_name) {
        Some(anim_elem) => Some(eval::new_struct_with_bindings(
            &anim_elem.borrow().bindings,
            &*component_type,
            eval_context,
        )),
        None => None,
    }
}

fn animation_for_element_property(
    component_type: Rc<ComponentDescription>,
    eval_context: ComponentRefPin,
    element: &Element,
    property_name: &String,
) -> Option<PropertyAnimation> {
    animation_for_property(
        component_type,
        eval_context,
        &element.property_animations,
        property_name,
    )
}

fn populate_model(
    vec: &mut Vec<ComponentBox>,
    rep_in_comp: &RepeaterWithinComponent,
    component: ComponentRefPin,
    model: impl Iterator<Item = eval::Value> + ExactSizeIterator,
) {
    vec.resize_with(model.size_hint().1.unwrap(), || {
        instantiate(rep_in_comp.component_to_repeat.clone(), Some(component))
    });
    for (i, (x, val)) in vec.iter().zip(model).enumerate() {
        rep_in_comp
            .component_to_repeat
            .set_property(x.borrow(), "index", i.try_into().unwrap())
            .unwrap();
        rep_in_comp.component_to_repeat.set_property(x.borrow(), "model_data", val).unwrap();
    }
}

pub fn instantiate(
    component_type: Rc<ComponentDescription>,
    parent_ctx: Option<ComponentRefPin>,
) -> ComponentBox {
    let instance = component_type.dynamic_type.clone().create_instance();
    let mem = instance.as_ptr().as_ptr() as *mut u8;
    let component_box = ComponentBox { instance, component_type: component_type.clone() };

    if let Some(parent) = parent_ctx {
        unsafe {
            *(mem.add(component_type.parent_component_offset.unwrap())
                as *mut Option<ComponentRefPin>) = Some(parent);
        }
    } else {
        component_type
            .set_property(component_box.borrow(), "dpi", crate::Value::Number(1.))
            .unwrap();
    }

    for item_within_component in component_type.items.values() {
        unsafe {
            let item = item_within_component.item_from_component(mem);
            let elem = item_within_component.elem.borrow();
            for (prop, expr) in &elem.bindings {
                let ty = elem.lookup_property(prop.as_str());
                if ty == Type::Signal {
                    let signal = &mut *(item_within_component
                        .rtti
                        .signals
                        .get(prop.as_str())
                        .map(|o| item.as_ptr().add(*o) as *mut u8)
                        .or_else(|| {
                            component_type.custom_signals.get(prop.as_str()).map(|o| mem.add(*o))
                        })
                        .unwrap_or_else(|| panic!("unkown signal {}", prop))
                        as *mut Signal<()>);
                    let expr = expr.clone();
                    let component_type = component_type.clone();
                    let instance = component_box.instance.as_ptr();
                    signal.set_handler(move |_| {
                        let c = Pin::new_unchecked(vtable::VRef::from_raw(
                            NonNull::from(&component_type.ct).cast(),
                            instance.cast(),
                        ));
                        eval::eval_expression(&expr, &*component_type, c);
                    })
                } else {
                    if let Some(prop_rtti) =
                        item_within_component.rtti.properties.get(prop.as_str())
                    {
                        let maybe_animation = animation_for_element_property(
                            component_type.clone(),
                            component_box.borrow(),
                            &elem,
                            prop,
                        );

                        if expr.is_constant() {
                            prop_rtti.set(
                                item,
                                eval::eval_expression(
                                    expr,
                                    &*component_type,
                                    component_box.borrow(),
                                ),
                                maybe_animation,
                            );
                        } else {
                            let expr = expr.clone();
                            let component_type = component_type.clone();
                            let instance = component_box.instance.as_ptr();

                            prop_rtti.set_binding(
                                item,
                                Box::new(move || {
                                    let c = Pin::new_unchecked(vtable::VRef::from_raw(
                                        NonNull::from(&component_type.ct).cast(),
                                        instance.cast(),
                                    ));
                                    eval::eval_expression(&expr, &*component_type, c)
                                }),
                                maybe_animation,
                            );
                        }
                    } else if let Some(PropertiesWithinComponent {
                        offset, prop: prop_info, ..
                    }) = component_type.custom_properties.get(prop.as_str())
                    {
                        let maybe_animation = animation_for_property(
                            component_type.clone(),
                            component_box.borrow(),
                            &component_type.original.root_element.borrow().property_animations,
                            prop,
                        );

                        if expr.is_constant() {
                            let v = eval::eval_expression(
                                expr,
                                &*component_type,
                                component_box.borrow(),
                            );
                            prop_info
                                .set(Pin::new_unchecked(&*mem.add(*offset)), v, maybe_animation)
                                .unwrap();
                        } else {
                            let expr = expr.clone();
                            let component_type = component_type.clone();
                            let instance = component_box.instance.as_ptr();
                            prop_info
                                .set_binding(
                                    Pin::new_unchecked(&*mem.add(*offset)),
                                    Box::new(move || {
                                        let c = Pin::new_unchecked(vtable::VRef::from_raw(
                                            NonNull::from(&component_type.ct).cast(),
                                            instance.cast(),
                                        ));
                                        eval::eval_expression(&expr, &*component_type, c)
                                    }),
                                    maybe_animation,
                                )
                                .unwrap();
                        }
                    } else {
                        panic!("unkown property {}", prop);
                    }
                }
            }
        }
    }

    for rep_in_comp in &component_type.repeater {
        if !rep_in_comp.model.is_constant() {
            continue;
        }
        let vec = unsafe { &mut *(mem.add(rep_in_comp.offset) as *mut RepeaterVec) };
        match eval::eval_expression(&rep_in_comp.model, &*component_type, component_box.borrow()) {
            crate::Value::Number(count) => populate_model(
                vec,
                rep_in_comp,
                component_box.borrow(),
                (0..count as i32).into_iter().map(|v| crate::Value::Number(v as f64)),
            ),
            crate::Value::Array(a) => {
                populate_model(vec, rep_in_comp, component_box.borrow(), a.into_iter())
            }
            crate::Value::Bool(b) => populate_model(
                vec,
                rep_in_comp,
                component_box.borrow(),
                (if b { Some(crate::Value::Void) } else { None }).into_iter(),
            ),
            _ => panic!("Unsupported model"),
        }
    }

    component_box
}

unsafe extern "C" fn compute_layout(component: ComponentRefPin) {
    // This is fine since we can only be called with a component that with our vtable which is a ComponentDescription
    let component_type =
        &*(component.get_vtable() as *const ComponentVTable as *const ComponentDescription);

    let resolve_prop_ref = |prop_ref: &expression_tree::Expression| {
        eval::eval_expression(&prop_ref, &component_type, component).try_into().unwrap_or_default()
    };

    for it in &component_type.original.layout_constraints.borrow().grids {
        use sixtyfps_corelib::layout::*;

        let mut row_constraint = vec![];
        let mut col_constraint = vec![];
        //let mut cells = vec![];

        row_constraint.resize_with(it.row_count(), Default::default);
        col_constraint.resize_with(it.col_count(), Default::default);

        let cells_v = it
            .elems
            .iter()
            .map(|x| {
                x.iter()
                    .map(|y| {
                        y.as_ref()
                            .map(|elem| {
                                let info = &component_type.items[elem.borrow().id.as_str()];
                                let get_prop = |name| {
                                    info.rtti.properties.get(name).map(|p| {
                                        &*(component.as_ptr().add(info.offset).add(p.offset())
                                            as *const Property<f32>)
                                    })
                                };
                                GridLayoutCellData {
                                    x: get_prop("x"),
                                    y: get_prop("y"),
                                    width: get_prop("width"),
                                    height: get_prop("height"),
                                }
                            })
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let cells = cells_v.iter().map(|x| x.as_slice().into()).collect::<Vec<Slice<_>>>();

        let within_info = &component_type.items[it.within.borrow().id.as_str()];
        let within_prop = |name| {
            within_info.rtti.properties[name]
                .get(within_info.item_from_component(component.as_ptr()))
                .try_into()
                .unwrap()
        };

        solve_grid_layout(&GridLayoutData {
            row_constraint: Slice::from(row_constraint.as_slice()),
            col_constraint: Slice::from(col_constraint.as_slice()),
            width: within_prop("width"),
            height: within_prop("height"),
            x: resolve_prop_ref(&it.x_reference),
            y: resolve_prop_ref(&it.y_reference),
            cells: Slice::from(cells.as_slice()),
        });
    }

    for it in &component_type.original.layout_constraints.borrow().paths {
        use sixtyfps_corelib::layout::*;

        let items = it
            .elements
            .iter()
            .map(|elem| {
                let item_info = &component_type.items[elem.borrow().id.as_str()];

                let get_prop = |name| {
                    item_info.rtti.properties.get(name).map(|p| {
                        &*(component.as_ptr().add(item_info.offset).add(p.offset())
                            as *const Property<f32>)
                    })
                };

                let item = item_info.item_from_component(component.as_ptr());
                let get_prop_value = |name| {
                    item_info.rtti.properties.get(name).map(|p| p.get(item)).unwrap_or_default()
                };
                PathLayoutItemData {
                    x: get_prop("x"),
                    y: get_prop("y"),
                    width: get_prop_value("width").try_into().unwrap_or_default(),
                    height: get_prop_value("height").try_into().unwrap_or_default(),
                }
            })
            .collect::<Vec<_>>();

        let path_elements = eval::convert_path(&it.path, component_type, component);

        solve_path_layout(&PathLayoutData {
            items: Slice::from(items.as_slice()),
            elements: &path_elements,
            x: resolve_prop_ref(&it.x_reference),
            y: resolve_prop_ref(&it.y_reference),
            width: resolve_prop_ref(&it.width_reference),
            height: resolve_prop_ref(&it.height_reference),
            offset: resolve_prop_ref(&it.offset_reference),
        });
    }
}

/// Get the component description from a ComponentRef
///
/// Safety: the component must have been created by the interpreter
pub unsafe fn get_component_type<'a>(component: ComponentRefPin<'a>) -> &'a ComponentDescription {
    &*(Pin::into_inner_unchecked(component).get_vtable() as *const ComponentVTable
        as *const ComponentDescription)
}
