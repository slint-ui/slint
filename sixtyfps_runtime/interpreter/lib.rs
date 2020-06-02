use core::cell::RefCell;
use core::ptr::NonNull;
use corelib::abi::datastructures::{
    ComponentBox, ComponentRef, ComponentRefMut, ComponentVTable, ItemVTable,
};
use corelib::rtti::PropertyInfo;
use corelib::{EvaluationContext, Property, SharedString};
use object_tree::Element;
use sixtyfps_compiler::typeregister::Type;
use sixtyfps_compiler::*;
use std::collections::HashMap;
use std::rc::Rc;

unsafe fn construct<T: Default>(ptr: *mut u8) {
    core::ptr::write(ptr as *mut T, T::default());
}

extern "C" fn dummy_destroy(_: ComponentRefMut) {
    panic!();
}

extern "C" fn dummy_create(_: &ComponentVTable) -> ComponentBox {
    panic!()
}

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

mod eval;

struct PropertiesWithinComponent {
    offset: usize,
    prop: Box<dyn PropertyInfo<u8, eval::Value>>,
    create: unsafe fn(*mut u8),
}
pub struct ComponentImpl {
    mem: *mut u8,
    component_type: Rc<MyComponentType>,
}

#[repr(C)]
pub struct MyComponentType {
    ct: ComponentVTable,
    it: Vec<corelib::abi::datastructures::ItemTreeNode>,
    size: usize,
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
    construct: unsafe fn(*mut u8),
    properties: HashMap<&'static str, Box<dyn eval::ErasedPropertyInfo>>,
    /// The uszie is an offset within this item to the Signal.
    /// Ideally, we would need a vtable::VFieldOffset<ItemVTable, corelib::Signal<()>>
    signals: HashMap<&'static str, usize>,
    size: usize,
}

fn rtti_for<
    T: 'static + Default + corelib::rtti::BuiltinItem + vtable::HasStaticVTable<ItemVTable>,
>() -> (&'static str, Rc<RuntimeTypeInfo>) {
    (
        T::name(),
        Rc::new(RuntimeTypeInfo {
            vtable: T::static_vtable(),
            construct: construct::<T>,
            properties: T::properties()
                .into_iter()
                .map(|(k, v)| (k, Box::new(v) as Box<dyn eval::ErasedPropertyInfo>))
                .collect(),
            signals: T::signals().into_iter().map(|(k, v)| (k, v.get_byte_offset())).collect(),
            size: core::mem::size_of::<T>(),
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
    let mut current_offset = 0usize;
    let mut items_types = HashMap::new();

    generator::build_array_helper(&tree.root_component, |rc_item, child_offset| {
        let item = rc_item.borrow();
        let rt = &rtti[&*item.base_type.as_builtin().class_name];
        tree_array.push(corelib::abi::datastructures::ItemTreeNode::Item {
            offset: current_offset as isize,
            vtable: rt.vtable,
            children_index: child_offset,
            chilren_count: item.children.len() as _,
        });
        items_types.insert(
            item.id.clone(),
            ItemWithinComponent { offset: current_offset, rtti: rt.clone(), elem: rc_item.clone() },
        );
        current_offset += rt.size;
    });

    let mut custom_properties = HashMap::new();
    let mut custom_signals = HashMap::new();
    for (name, decl) in &tree.root_component.root_element.borrow().property_declarations {
        fn create_and_set<T: Clone + Default + 'static>(
        ) -> (Box<dyn PropertyInfo<u8, eval::Value>>, unsafe fn(*mut u8))
        where
            T: std::convert::TryInto<eval::Value>,
            eval::Value: std::convert::TryInto<T>,
        {
            // Fixme: using u8 in PropertyInfo<> is not sound, we would need to materialize a type for out component
            (
                Box::new(unsafe { vtable::FieldOffset::<u8, Property<T>>::new_from_offset(0) }),
                construct::<Property<T>>,
            )
        }
        let (prop, create) = match decl.property_type {
            Type::Float32 => create_and_set::<f32>(),
            Type::Int32 => create_and_set::<u32>(),
            Type::String => create_and_set::<SharedString>(),
            Type::Color => create_and_set::<u32>(),
            Type::Image => create_and_set::<SharedString>(),
            Type::Bool => create_and_set::<bool>(),
            Type::Signal => {
                custom_signals.insert(name.clone(), current_offset);
                current_offset += core::mem::size_of::<corelib::Signal<()>>();
                continue;
            }
            _ => panic!("bad type"),
        };
        custom_properties.insert(
            name.clone(),
            PropertiesWithinComponent { offset: current_offset, prop, create },
        );
        // FIXME: get the actual size depending of the type
        current_offset += 32;
    }

    let t = ComponentVTable { create: dummy_create, drop: dummy_destroy, item_tree };
    let t = MyComponentType {
        ct: t,
        it: tree_array,
        size: current_offset,
        items: items_types,
        custom_properties,
        custom_signals,
        original: tree,
    };

    Ok(Rc::new(t))
}

/// FIXME: return a handle to the component instead of taking a callback
pub fn instentiate<T>(
    component_type: Rc<MyComponentType>,
    run: impl FnOnce(vtable::VRefMut<'static, ComponentVTable>) -> T,
) -> T {
    let mut my_impl = Vec::<u64>::new();
    my_impl.resize(component_type.size / 8 + 1, 0);
    let mem = my_impl.as_mut_ptr() as *mut u8;

    for PropertiesWithinComponent { offset, create, .. } in
        component_type.custom_properties.values()
    {
        unsafe { create(mem.offset(*offset as isize)) };
    }
    for offset in component_type.custom_signals.values() {
        unsafe { construct::<corelib::Signal<()>>(mem.offset(*offset as isize)) };
    }

    let ctx = Rc::new(ComponentImpl { mem, component_type });

    let component_ref = unsafe {
        ComponentRefMut::from_raw(
            NonNull::from(&ctx.component_type.ct).cast(),
            NonNull::new(mem).unwrap().cast(),
        )
    };
    let eval_context = EvaluationContext { component: component_ref.borrow() };

    for item_within_component in ctx.component_type.items.values() {
        unsafe {
            let item = item_within_component.item_from_component(mem);
            (item_within_component.rtti.construct)(item.as_ptr() as _);
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
                            prop.set(&*item.as_ptr().add(*offset), v).unwrap();
                        } else {
                            let expr = expr.clone();
                            let ctx = ctx.clone();
                            prop.set_binding(
                                &*item.as_ptr().add(*offset),
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

    run(component_ref)
}
