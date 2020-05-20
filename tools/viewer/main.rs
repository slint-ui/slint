use core::ptr::NonNull;
use corelib::abi::datastructures::{ComponentBox, ComponentRef, ComponentRefMut, ComponentVTable};
use corelib::{Property, SharedString};
use sixtyfps_compiler::object_tree::Expression;
use std::collections::HashMap;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Cli {
    #[structopt(name = "path to .60 file", parse(from_os_str))]
    path: std::path::PathBuf,
}

trait PropertyWriter {
    unsafe fn write(ptr: *mut u8, value: &Expression);
}

impl PropertyWriter for f32 {
    unsafe fn write(ptr: *mut u8, value: &Expression) {
        let val: Self = match value {
            Expression::NumberLiteral(v) => *v as _,
            _ => todo!(),
        };
        (*(ptr as *mut Property<Self>)).set(val);
    }
}

impl PropertyWriter for u32 {
    unsafe fn write(ptr: *mut u8, value: &Expression) {
        let val: Self = match value {
            Expression::NumberLiteral(v) => *v as _,
            _ => todo!(),
        };
        (*(ptr as *mut Property<Self>)).set(val);
    }
}

impl PropertyWriter for SharedString {
    unsafe fn write(ptr: *mut u8, value: &Expression) {
        let val: Self = match value {
            Expression::StringLiteral(v) => (**v).into(),
            _ => todo!(),
        };
        (*(ptr as *mut Property<Self>)).set(val.clone());
    }
}

unsafe fn construct<T: Default>(ptr: *mut u8) {
    core::ptr::write(ptr as *mut T, T::default());
}

unsafe fn set_property<T: PropertyWriter>(ptr: *mut u8, e: &Expression) {
    T::write(ptr, e);
}

extern "C" fn dummy_destroy(_: ComponentRefMut) {
    panic!();
}

extern "C" fn dummy_create(_: &ComponentVTable) -> ComponentBox {
    panic!()
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
    properties: HashMap<&'static str, (usize, unsafe fn(*mut u8, &Expression))>,
    size: usize,
}

fn main() -> std::io::Result<()> {
    use sixtyfps_compiler::*;
    let args = Cli::from_args();
    let source = std::fs::read_to_string(&args.path)?;
    let (syntax_node, mut diag) = parser::parse(&source);
    diag.current_path = args.path;
    let mut tr = typeregister::TypeRegister::builtin();
    let tree = object_tree::Document::from_node(syntax_node, &mut diag, &mut tr);
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
                ("x", (offsets.x.get_byte_offset(), set_property::<f32> as _)),
                ("y", (offsets.y.get_byte_offset(), set_property::<f32> as _)),
                ("width", (offsets.width.get_byte_offset(), set_property::<f32> as _)),
                ("height", (offsets.height.get_byte_offset(), set_property::<f32> as _)),
                ("color", (offsets.color.get_byte_offset(), set_property::<u32> as _)),
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
                ("x", (offsets.x.get_byte_offset(), set_property::<f32> as _)),
                ("y", (offsets.y.get_byte_offset(), set_property::<f32> as _)),
                ("width", (offsets.width.get_byte_offset(), set_property::<f32> as _)),
                ("height", (offsets.height.get_byte_offset(), set_property::<f32> as _)),
                ("source", (offsets.source.get_byte_offset(), set_property::<SharedString> as _)),
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
            vtable: &corelib::abi::primitives::ImageVTable as _,
            construct: construct::<Text>,
            properties: [
                ("x", (offsets.x.get_byte_offset(), set_property::<f32> as _)),
                ("y", (offsets.y.get_byte_offset(), set_property::<f32> as _)),
                ("text", (offsets.text.get_byte_offset(), set_property::<SharedString> as _)),
                ("color", (offsets.color.get_byte_offset(), set_property::<u32> as _)),
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
                ("x", (offsets.x.get_byte_offset(), set_property::<f32> as _)),
                ("y", (offsets.y.get_byte_offset(), set_property::<f32> as _)),
                ("width", (offsets.width.get_byte_offset(), set_property::<f32> as _)),
                ("height", (offsets.height.get_byte_offset(), set_property::<f32> as _)),
            ]
            .iter()
            .cloned()
            .collect(),
            size: std::mem::size_of::<TouchArea>(),
        },
    );


    let l = lower::LoweredComponent::lower(&*tree.root_component);

    let mut tree_array = vec![];
    let mut current_offset = 0usize;
    let mut items_types = vec![];

    generator::build_array_helper(&l, |item, child_offset| {
        let rt = &rtti[&*item.native_type.class_name];
        tree_array.push(corelib::abi::datastructures::ItemTreeNode::Item {
            offset: current_offset as isize,
            vtable: rt.vtable,
            children_index: child_offset,
            chilren_count: item.children.len() as _,
        });
        items_types.push((current_offset, rt, item.init_properties.clone()));
        current_offset += rt.size;
    });

    let t = ComponentVTable { create: dummy_create, drop: dummy_destroy, item_tree };
    let t = MyComponentType { ct: t, it: tree_array };

    let mut my_impl = Vec::<u64>::new();
    my_impl.resize(current_offset / 8 + 1, 0);
    let mem = my_impl.as_mut_ptr() as *mut u8;

    for (offset, rtti, properties) in items_types {
        unsafe {
            let item = mem.offset(offset as isize);
            (rtti.construct)(item as _);
            for (prop, expr) in properties {
                let (o, set) = rtti.properties[&*prop];
                set(item.offset(o as isize), &expr);
            }
        }
    }

    let component_ref = unsafe {
        ComponentRefMut::from_raw(NonNull::from(&t).cast(), NonNull::new(mem).unwrap().cast())
    };

    gl::sixtyfps_runtime_run_component_with_gl_renderer(component_ref);

    Ok(())
}
