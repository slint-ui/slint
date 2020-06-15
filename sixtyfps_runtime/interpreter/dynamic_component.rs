use crate::{dynamic_type, eval};

use core::convert::TryInto;
use core::ptr::NonNull;
use dynamic_type::Instance;
use object_tree::ElementRc;
use sixtyfps_compilerlib::typeregister::Type;
use sixtyfps_compilerlib::*;
use sixtyfps_corelib::abi::datastructures::{
    ComponentBox, ComponentRef, ComponentVTable, ItemTreeNode, ItemVTable, ItemVisitorRefMut,
    Resource,
};
use sixtyfps_corelib::abi::slice::Slice;
use sixtyfps_corelib::rtti::PropertyInfo;
use sixtyfps_corelib::{rtti, EvaluationContext, Property, SharedString, Signal};
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) struct ItemWithinComponent {
    offset: usize,
    pub(crate) rtti: Rc<ItemRTTI>,
    elem: ElementRc,
}

impl ItemWithinComponent {
    pub(crate) unsafe fn item_from_component(&self, mem: *const u8) -> vtable::VRef<ItemVTable> {
        vtable::VRef::from_raw(
            NonNull::from(self.rtti.vtable),
            NonNull::new(mem.add(self.offset) as _).unwrap(),
        )
    }
}

pub(crate) struct PropertiesWithinComponent {
    pub(crate) offset: usize,
    pub(crate) prop: Box<dyn PropertyInfo<u8, eval::Value>>,
}

/// A wrapper around a pointer to a Component, and its description.
///
/// Safety: the `mem` member must be a component instantiated with the description of
/// this document.
///
/// It is similar to ComponentBox, but instead of a ComponentVTable pointer, we
/// have direct access to the ComponentDescription, so we do not have to cast
pub struct ComponentImpl {
    pub(crate) mem: *mut u8,
    pub(crate) component_type: Rc<ComponentDescription>,
}

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
    /// Keep the Rc alive
    pub(crate) original: object_tree::Document,
}

unsafe extern "C" fn visit_children_item(
    component: ComponentRef,
    index: isize,
    v: ItemVisitorRefMut,
) {
    let component_type =
        &*(component.get_vtable() as *const ComponentVTable as *const ComponentDescription);
    let item_tree = &component_type.it;
    sixtyfps_corelib::item_tree::visit_item_tree(
        &*(component.as_ptr() as *const Instance),
        component,
        item_tree.as_slice().into(),
        index,
        v,
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
    source: &str,
    path: &std::path::Path,
) -> Result<Rc<ComponentDescription>, sixtyfps_compilerlib::diagnostics::Diagnostics> {
    let (syntax_node, mut diag) = parser::parse(&source);
    diag.current_path = path.into();
    let mut tr = typeregister::TypeRegister::builtin();
    let tree = object_tree::Document::from_node(syntax_node.into(), &mut diag, &mut tr);
    if !diag.inner.is_empty() {
        return Err(diag);
    }
    let compiler_config = CompilerConfiguration::default();
    run_passes(&tree, &mut diag, &mut tr, &compiler_config);
    if !diag.inner.is_empty() {
        return Err(diag);
    }

    let mut rtti = HashMap::new();
    {
        use sixtyfps_corelib::abi::primitives::*;
        rtti.extend(
            [
                rtti_for::<Image>(),
                rtti_for::<Text>(),
                rtti_for::<Rectangle>(),
                rtti_for::<TouchArea>(),
            ]
            .iter()
            .cloned(),
        );
    }
    let rtti = Rc::new(rtti);

    let mut tree_array = vec![];
    let mut items_types = HashMap::new();
    let mut builder = dynamic_type::TypeBuilder::new();

    generator::build_array_helper(&tree.root_component, |rc_item, child_offset| {
        let item = rc_item.borrow();
        if item.repeated.is_some() {
            todo!()
        }
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
    });

    let mut custom_properties = HashMap::new();
    let mut custom_signals = HashMap::new();
    for (name, decl) in &tree.root_component.root_element.borrow().property_declarations {
        fn property_info<T: Clone + Default + 'static>(
        ) -> (Box<dyn PropertyInfo<u8, eval::Value>>, dynamic_type::StaticTypeInfo)
        where
            T: std::convert::TryInto<eval::Value>,
            eval::Value: std::convert::TryInto<T>,
        {
            // Fixme: using u8 in PropertyInfo<> is not sound, we would need to materialize a type for out component
            (
                Box::new(unsafe { vtable::FieldOffset::<u8, Property<T>>::new_from_offset(0) }),
                dynamic_type::StaticTypeInfo::new::<Property<T>>(),
            )
        }
        let (prop, type_info) = match decl.property_type {
            Type::Float32 => property_info::<f32>(),
            Type::Int32 => property_info::<u32>(),
            Type::String => property_info::<SharedString>(),
            Type::Color => property_info::<u32>(),
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

    extern "C" fn layout_info(
        _: ComponentRef,
    ) -> sixtyfps_corelib::abi::datastructures::LayoutInfo {
        todo!()
    }

    let t = ComponentVTable {
        create: component_create,
        drop: component_destroy,
        visit_children_item,
        layout_info,
        compute_layout,
    };
    let t = ComponentDescription {
        ct: t,
        dynamic_type: builder.build(),
        it: tree_array,
        items: items_types,
        custom_properties,
        custom_signals,
        original: tree,
    };

    Ok(Rc::new(t))
}

/// Safety: Can only be called for ComponentVTable which are in `MyComponentType`
unsafe extern "C" fn component_create(s: &ComponentVTable) -> ComponentBox {
    // This is safe because we have an instance of ComponentVTable which is the first field of MyComponentType
    // And the only way to get a MyComponentType is through the load function which returns a Rc
    let component_type = Rc::<ComponentDescription>::from_raw(
        s as *const ComponentVTable as *const ComponentDescription,
    );
    // We need to increment the ref-count, as from_raw doesn't do that.
    std::mem::forget(component_type.clone());
    instentiate(component_type)
}

pub fn instentiate(component_type: Rc<ComponentDescription>) -> ComponentBox {
    let instance = component_type.dynamic_type.clone().create_instance();
    let mem = instance as *mut u8;

    let ctx = Rc::new(ComponentImpl { mem, component_type: component_type.clone() });

    let component_box = unsafe {
        ComponentBox::from_raw(
            NonNull::from(&ctx.component_type.ct).cast(),
            NonNull::new(mem).unwrap().cast(),
        )
    };
    let eval_context = EvaluationContext { component: component_box.borrow() };

    for item_within_component in ctx.component_type.items.values() {
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
                            ctx.component_type
                                .custom_signals
                                .get(prop.as_str())
                                .map(|o| mem.add(*o))
                        })
                        .unwrap_or_else(|| panic!("unkown signal {}", prop))
                        as *mut Signal<()>);
                    let expr = expr.clone();
                    let ctx = ctx.clone();
                    signal.set_handler(move |eval_context, _| {
                        eval::eval_expression(&expr, &*ctx, &eval_context);
                    })
                } else {
                    if let Some(prop_rtti) =
                        item_within_component.rtti.properties.get(prop.as_str())
                    {
                        if expr.is_constant() {
                            prop_rtti.set(item, eval::eval_expression(expr, &*ctx, &eval_context));
                        } else {
                            let expr = expr.clone();
                            let ctx = ctx.clone();
                            prop_rtti.set_binding(
                                item,
                                Box::new(move |eval_context| {
                                    eval::eval_expression(&expr, &*ctx, eval_context)
                                }),
                            );
                        }
                    } else if let Some(PropertiesWithinComponent { offset, prop, .. }) =
                        ctx.component_type.custom_properties.get(prop.as_str())
                    {
                        if expr.is_constant() {
                            let v = eval::eval_expression(expr, &*ctx, &eval_context);
                            prop.set(&*mem.add(*offset), v).unwrap();
                        } else {
                            let expr = expr.clone();
                            let ctx = ctx.clone();
                            prop.set_binding(
                                &*mem.add(*offset),
                                Box::new(move |eval_context| {
                                    eval::eval_expression(&expr, &*ctx, eval_context)
                                }),
                            );
                        }
                    } else {
                        panic!("unkown property {}", prop);
                    }
                }
            }
        }
    }

    // The destructor of ComponentBox will take care of reducing the count
    Rc::into_raw(component_type);
    component_box
}

unsafe extern "C" fn component_destroy(component_ref: vtable::VRefMut<ComponentVTable>) {
    // Take the reference count that the instentiate function leaked
    let _vtable_rc = Rc::<ComponentDescription>::from_raw(component_ref.get_vtable()
        as *const ComponentVTable
        as *const ComponentDescription);

    dynamic_type::TypeInfo::delete_instance(component_ref.as_ptr() as *mut dynamic_type::Instance);
}

unsafe extern "C" fn compute_layout(component: ComponentRef) {
    // This is fine since we can only be called with a component that with our vtable which is a ComponentDescription
    let component_type =
        &*(component.get_vtable() as *const ComponentVTable as *const ComponentDescription);

    let eval_context = EvaluationContext { component };

    for it in &component_type.original.root_component.layout_constraints.borrow().0 {
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
                .get(within_info.item_from_component(component.as_ptr()), &eval_context)
                .try_into()
                .unwrap()
        };

        solve_grid_layout(&GridLayoutData {
            row_constraint: Slice::from(row_constraint.as_slice()),
            col_constraint: Slice::from(col_constraint.as_slice()),
            width: within_prop("width"),
            height: within_prop("height"),
            x: 0.,
            y: 0.,
            cells: Slice::from(cells.as_slice()),
        });
    }
}
