use core::cell::RefCell;
use core::ptr::NonNull;
use corelib::abi::datastructures::{ComponentBox, ComponentRef, ComponentVTable, ItemVTable};
use corelib::rtti::PropertyInfo;
use corelib::{EvaluationContext, Property, SharedString};
use object_tree::Element;
use sixtyfps_compiler::typeregister::Type;
use sixtyfps_compiler::*;
use std::collections::HashMap;
use std::rc::Rc;

mod dynamic_type;
mod eval;

struct ItemWithinComponent {
    offset: usize,
    rtti: Rc<RuntimeTypeInfo>,
    elem: Rc<RefCell<Element>>,
}

impl ItemWithinComponent {
    unsafe fn item_from_component(&self, mem: *const u8) -> vtable::VRef<ItemVTable> {
        vtable::VRef::from_raw(
            NonNull::from(self.rtti.vtable),
            NonNull::new(mem.add(self.offset) as _).unwrap(),
        )
    }
}

struct PropertiesWithinComponent {
    offset: usize,
    prop: Box<dyn PropertyInfo<u8, eval::Value>>,
}
pub struct ComponentImpl {
    mem: *mut u8,
    component_type: Rc<MyComponentType>,
}

#[repr(C)]
pub struct MyComponentType {
    ct: ComponentVTable,
    dynamic_type: Rc<dynamic_type::TypeInfo>,
    it: Vec<corelib::abi::datastructures::ItemTreeNode>,
    items: HashMap<String, ItemWithinComponent>,
    custom_properties: HashMap<String, PropertiesWithinComponent>,
    /// the usize is the offset within `mem` to the Signal<()>
    custom_signals: HashMap<String, usize>,
    /// Keep the Rc alive
    original: object_tree::Document,
}

extern "C" fn item_tree(r: ComponentRef<'_>) -> *const corelib::abi::datastructures::ItemTreeNode {
    // FIXME! unsafe is not correct here, as the ComponentVTable might not be a MyComponentType
    // (one can safely take a copy of the vtable and call the create function to get a box)
    unsafe { (*(r.get_vtable() as *const ComponentVTable as *const MyComponentType)).it.as_ptr() }
}

struct RuntimeTypeInfo {
    vtable: &'static ItemVTable,
    type_info: dynamic_type::StaticTypeInfo,
    properties: HashMap<&'static str, Box<dyn eval::ErasedPropertyInfo>>,
    /// The uszie is an offset within this item to the Signal.
    /// Ideally, we would need a vtable::VFieldOffset<ItemVTable, corelib::Signal<()>>
    signals: HashMap<&'static str, usize>,
}

fn rtti_for<
    T: 'static + Default + corelib::rtti::BuiltinItem + vtable::HasStaticVTable<ItemVTable>,
>() -> (&'static str, Rc<RuntimeTypeInfo>) {
    (
        T::name(),
        Rc::new(RuntimeTypeInfo {
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

pub fn load(
    source: &str,
    path: &std::path::Path,
) -> Result<Rc<MyComponentType>, sixtyfps_compiler::diagnostics::Diagnostics> {
    let (syntax_node, mut diag) = parser::parse(&source);
    diag.current_path = path.into();
    let mut tr = typeregister::TypeRegister::builtin();
    let tree = object_tree::Document::from_node(syntax_node, &mut diag, &mut tr);
    if !diag.inner.is_empty() {
        return Err(diag);
    }
    run_passes(&tree, &mut diag, &mut tr);
    if !diag.inner.is_empty() {
        return Err(diag);
    }

    let mut rtti = HashMap::new();
    {
        use corelib::abi::primitives::*;
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
        let rt = &rtti[&*item.base_type.as_builtin().class_name];
        let offset = builder.add_field(rt.type_info);
        tree_array.push(corelib::abi::datastructures::ItemTreeNode::Item {
            offset: offset as isize,
            vtable: rt.vtable,
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
            Type::Image => property_info::<SharedString>(),
            Type::Bool => property_info::<bool>(),
            Type::Signal => {
                custom_signals
                    .insert(name.clone(), builder.add_field_type::<corelib::Signal<()>>());
                continue;
            }
            _ => panic!("bad type"),
        };
        custom_properties.insert(
            name.clone(),
            PropertiesWithinComponent { offset: builder.add_field(type_info), prop },
        );
    }

    let t = ComponentVTable { create: component_create, drop: component_destroy, item_tree };
    let t = MyComponentType {
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
    let component_type =
        Rc::<MyComponentType>::from_raw(s as *const ComponentVTable as *const MyComponentType);
    // We need to increment the ref-count, as from_raw doesn't do that.
    std::mem::forget(component_type.clone());
    instentiate(component_type)
}

pub fn instentiate(component_type: Rc<MyComponentType>) -> ComponentBox {
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
                        as *mut corelib::Signal<()>);
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
    let _vtable_rc = Rc::<MyComponentType>::from_raw(component_ref.get_vtable()
        as *const ComponentVTable
        as *const MyComponentType);

    dynamic_type::TypeInfo::delete_instance(component_ref.as_ptr() as *mut dynamic_type::Instance);
}
