use core::ptr::NonNull;
use corelib::abi::datastructures::{ComponentBox, ComponentRef, ComponentRefMut, ComponentVTable};
use corelib::{Property, SharedString};
use sixtyfps_compiler::expression_tree::Expression;
use sixtyfps_compiler::typeregister::Type;
use sixtyfps_compiler::*;
use std::collections::HashMap;
use structopt::StructOpt;

type SetterFn = unsafe fn(*mut u8, eval::Value);
type GetterFn = unsafe fn(*mut u8) -> eval::Value;

#[derive(StructOpt)]
struct Cli {
    #[structopt(name = "path to .60 file", parse(from_os_str))]
    path: std::path::PathBuf,
}

trait PropertyWriter {
    unsafe fn write(ptr: *mut u8, value: eval::Value);
    unsafe fn read(ptr: *mut u8) -> eval::Value;
}

impl PropertyWriter for f32 {
    unsafe fn write(ptr: *mut u8, value: eval::Value) {
        let val: Self = match value {
            eval::Value::Number(v) => v as _,
            _ => todo!(),
        };
        (*(ptr as *mut Property<Self>)).set(val);
    }
    unsafe fn read(ptr: *mut u8) -> eval::Value {
        let s: Self = (*(ptr as *mut Property<Self>)).get();
        eval::Value::Number(s as _)
    }
}

impl PropertyWriter for bool {
    unsafe fn write(_ptr: *mut u8, _value: eval::Value) {
        todo!("Boolean expression not implemented")
    }
    unsafe fn read(_ptr: *mut u8) -> eval::Value {
        todo!("Boolean expression not implemented")
    }
}

impl PropertyWriter for u32 {
    unsafe fn write(ptr: *mut u8, value: eval::Value) {
        let val: Self = match value {
            eval::Value::Number(v) => v as _,
            _ => todo!(),
        };
        (*(ptr as *mut Property<Self>)).set(val);
    }
    unsafe fn read(ptr: *mut u8) -> eval::Value {
        let s: Self = (*(ptr as *mut Property<Self>)).get();
        eval::Value::Number(s as _)
    }
}

impl PropertyWriter for SharedString {
    unsafe fn write(ptr: *mut u8, value: eval::Value) {
        let val: Self = match value {
            eval::Value::String(v) => v,
            _ => todo!(),
        };
        (*(ptr as *mut Property<Self>)).set(val);
    }
    unsafe fn read(ptr: *mut u8) -> eval::Value {
        let s: Self = (*(ptr as *mut Property<Self>)).get();
        eval::Value::String(s)
    }
}

unsafe fn construct<T: Default>(ptr: *mut u8) {
    core::ptr::write(ptr as *mut T, T::default());
}

unsafe fn set_property<T: PropertyWriter>(ptr: *mut u8, e: eval::Value) {
    T::write(ptr, e);
}

unsafe fn get_property<T: PropertyWriter>(ptr: *mut u8) -> eval::Value {
    T::read(ptr)
}

extern "C" fn dummy_destroy(_: ComponentRefMut) {
    panic!();
}

extern "C" fn dummy_create(_: &ComponentVTable) -> ComponentBox {
    panic!()
}

struct ItemWithinComponent<'a> {
    offset: usize,
    rtti: &'a RuntimeTypeInfo,
    init_properties: HashMap<String, Expression>,
}

mod eval;

struct PropertiesWithinComponent {
    offset: usize,
    set: SetterFn,
    get: GetterFn,
    create: unsafe fn(*mut u8),
}
pub struct ComponentImpl<'a> {
    mem: *mut u8,
    items: HashMap<String, ItemWithinComponent<'a>>,
    custom_properties: HashMap<String, PropertiesWithinComponent>,
}

#[repr(C)]
struct MyComponentType {
    ct: ComponentVTable,
    it: Vec<corelib::abi::datastructures::ItemTreeNode>,
}

extern "C" fn item_tree(r: ComponentRef<'_>) -> *const corelib::abi::datastructures::ItemTreeNode {
    // FIXME! unsafe is not correct here, as the ComponentVTable might not be a MyComponentType
    // (one can safely take a copy of the vtable and call the create function to get a box)
    unsafe { (*(r.get_vtable() as *const ComponentVTable as *const MyComponentType)).it.as_ptr() }
}

struct RuntimeTypeInfo {
    vtable: *const corelib::abi::datastructures::ItemVTable,
    construct: unsafe fn(*mut u8),
    properties: HashMap<&'static str, (usize, SetterFn, GetterFn)>,
    size: usize,
}

fn main() -> std::io::Result<()> {
    let args = Cli::from_args();
    let source = std::fs::read_to_string(&args.path)?;
    let (syntax_node, mut diag) = parser::parse(&source);
    diag.current_path = args.path;
    let mut tr = typeregister::TypeRegister::builtin();
    let tree = object_tree::Document::from_node(syntax_node, &mut diag, &mut tr);
    run_passes(&tree, &mut diag, &mut tr);
    if !diag.inner.is_empty() {
        diag.print(source);
        std::process::exit(-1);
    }

    use corelib::abi::primitives::{Image, Rectangle, Text, TouchArea};

    // FIXME: thus obviously is unsafe and not great
    let mut rtti = HashMap::new();

    let offsets = Rectangle::field_offsets();
    rtti.insert(
        "Rectangle",
        RuntimeTypeInfo {
            vtable: &corelib::abi::primitives::RectangleVTable as _,
            construct: construct::<Rectangle>,
            properties: [
                (
                    "x",
                    (
                        offsets.x.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "y",
                    (
                        offsets.y.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "width",
                    (
                        offsets.width.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "height",
                    (
                        offsets.height.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "color",
                    (
                        offsets.color.get_byte_offset(),
                        set_property::<u32> as _,
                        get_property::<f32> as _,
                    ),
                ),
            ]
            .iter()
            .cloned()
            .collect(),
            size: std::mem::size_of::<Rectangle>(),
        },
    );

    let offsets = Image::field_offsets();
    rtti.insert(
        "Image",
        RuntimeTypeInfo {
            vtable: &corelib::abi::primitives::ImageVTable as _,
            construct: construct::<Image>,
            properties: [
                (
                    "x",
                    (
                        offsets.x.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "y",
                    (
                        offsets.y.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "width",
                    (
                        offsets.width.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "height",
                    (
                        offsets.height.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "source",
                    (
                        offsets.source.get_byte_offset(),
                        set_property::<SharedString> as _,
                        get_property::<SharedString> as _,
                    ),
                ),
            ]
            .iter()
            .cloned()
            .collect(),
            size: std::mem::size_of::<Image>(),
        },
    );

    let offsets = Text::field_offsets();
    rtti.insert(
        "Text",
        RuntimeTypeInfo {
            vtable: &corelib::abi::primitives::TextVTable as _,
            construct: construct::<Text>,
            properties: [
                (
                    "x",
                    (
                        offsets.x.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "y",
                    (
                        offsets.y.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "text",
                    (
                        offsets.text.get_byte_offset(),
                        set_property::<SharedString> as _,
                        get_property::<SharedString> as _,
                    ),
                ),
                (
                    "color",
                    (
                        offsets.color.get_byte_offset(),
                        set_property::<u32> as _,
                        get_property::<f32> as _,
                    ),
                ),
            ]
            .iter()
            .cloned()
            .collect(),
            size: std::mem::size_of::<Text>(),
        },
    );

    let offsets = TouchArea::field_offsets();
    rtti.insert(
        "TouchArea",
        RuntimeTypeInfo {
            vtable: &corelib::abi::primitives::TouchAreaVTable as _,
            construct: construct::<TouchArea>,
            properties: [
                (
                    "x",
                    (
                        offsets.x.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "y",
                    (
                        offsets.y.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "width",
                    (
                        offsets.width.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "height",
                    (
                        offsets.height.get_byte_offset(),
                        set_property::<f32> as _,
                        get_property::<f32> as _,
                    ),
                ),
                (
                    "pressed",
                    (
                        offsets.pressed.get_byte_offset(),
                        set_property::<bool> as _,
                        get_property::<bool> as _,
                    ),
                ),
            ]
            .iter()
            .cloned()
            .collect(),
            size: std::mem::size_of::<TouchArea>(),
        },
    );

    let l = lower::LoweredComponent::lower(&tree.root_component);

    let mut tree_array = vec![];
    let mut current_offset = 0usize;
    let mut items_types = HashMap::new();

    generator::build_array_helper(&l, |item, child_offset| {
        let rt = &rtti[&*item.native_type.class_name];
        tree_array.push(corelib::abi::datastructures::ItemTreeNode::Item {
            offset: current_offset as isize,
            vtable: rt.vtable,
            children_index: child_offset,
            chilren_count: item.children.len() as _,
        });
        items_types.insert(
            item.id.clone(),
            ItemWithinComponent {
                offset: current_offset,
                rtti: rt,
                init_properties: item.init_properties.clone(),
            },
        );
        current_offset += rt.size;
    });

    let mut custom_properties = HashMap::new();
    for (name, decl) in &l.property_declarations {
        fn create_and_set<T: PropertyWriter + Default + 'static>(
        ) -> (SetterFn, GetterFn, unsafe fn(*mut u8)) {
            (set_property::<T>, get_property::<T>, construct::<Property<T>>)
        }
        let (set, get, create) = match decl.property_type {
            Type::Float32 => create_and_set::<f32>(),
            Type::Int32 => create_and_set::<u32>(),
            Type::String => create_and_set::<SharedString>(),
            Type::Color => create_and_set::<u32>(),
            Type::Image => create_and_set::<SharedString>(),
            Type::Bool => create_and_set::<bool>(),
            _ => panic!("bad type"),
        };
        custom_properties.insert(
            name.clone(),
            PropertiesWithinComponent { offset: current_offset, set, get, create },
        );
        // FIXME: get the actual size depending of the type
        current_offset += 32;
    }

    let t = ComponentVTable { create: dummy_create, drop: dummy_destroy, item_tree };
    let t = MyComponentType { ct: t, it: tree_array };

    let mut my_impl = Vec::<u64>::new();
    my_impl.resize(current_offset / 8 + 1, 0);
    let mem = my_impl.as_mut_ptr() as *mut u8;

    for PropertiesWithinComponent { offset, create, .. } in custom_properties.values() {
        unsafe { create(mem.offset(*offset as isize)) };
    }

    let ctx = ComponentImpl { mem, items: items_types, custom_properties };

    for ItemWithinComponent { offset, rtti, init_properties } in ctx.items.values() {
        unsafe {
            let item = mem.offset(*offset as isize);
            (rtti.construct)(item as _);
            for (prop, expr) in init_properties {
                let v = eval::eval_expression(expr, &ctx);
                if let Some((o, set, _)) = rtti.properties.get(prop.as_str()) {
                    set(item.offset(*o as isize), v);
                } else {
                    let PropertiesWithinComponent { offset, set, .. } =
                        ctx.custom_properties[prop.as_str()];
                    set(item.offset(offset as isize), v);
                }
            }
        }
    }

    let component_ref = unsafe {
        ComponentRefMut::from_raw(NonNull::from(&t).cast(), NonNull::new(mem).unwrap().cast())
    };

    gl::sixtyfps_runtime_run_component_with_gl_renderer(component_ref);

    Ok(())
}
