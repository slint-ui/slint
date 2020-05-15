use core::ptr::NonNull;
use corelib::abi::datastructures::{ComponentBox, ComponentRef, ComponentRefMut, ComponentVTable};
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
        std::ptr::write(ptr as *mut Self, val);
    }
}

impl PropertyWriter for u32 {
    unsafe fn write(ptr: *mut u8, value: &Expression) {
        let val: Self = match value {
            Expression::NumberLiteral(v) => *v as _,
            _ => todo!(),
        };
        std::ptr::write(ptr as *mut Self, val);
    }
}

impl PropertyWriter for *const i8 {
    unsafe fn write(ptr: *mut u8, value: &Expression) {
        let val: Self = match value {
            Expression::StringLiteral(v) => {
                // FIXME that's a leak
                std::ffi::CString::new(v.as_str()).unwrap().into_raw() as _
            }
            _ => todo!(),
        };
        std::ptr::write(ptr as *mut Self, val);
    }
}

unsafe fn construct<T: Default>(ptr: *mut corelib::abi::datastructures::ItemImpl) {
    core::ptr::write(ptr as *mut T, T::default());
}

unsafe fn set_property<T: PropertyWriter>(ptr: *mut u8, e: &Expression) {
    T::write(ptr, e);
}

unsafe extern "C" fn dummy_destroy(_: ComponentRefMut) {
    panic!();
}

unsafe extern "C" fn dummy_create(_: *const ComponentVTable) -> ComponentBox {
    panic!()
}

#[repr(C)]
struct MyComponentType {
    ct: ComponentVTable,
    it: Vec<corelib::abi::datastructures::ItemTreeNode>,
}

unsafe extern "C" fn item_tree(
    r: ComponentRef<'_>,
) -> *const corelib::abi::datastructures::ItemTreeNode {
    (*(ComponentRef::get_vtable(&r).as_ptr() as *const MyComponentType)).it.as_ptr()
}

struct RuntimeTypeInfo {
    vtable: *const corelib::abi::datastructures::ItemVTable,
    construct: unsafe fn(*mut corelib::abi::datastructures::ItemImpl),
    properties: HashMap<&'static str, (usize, unsafe fn(*mut u8, &Expression))>,
    size: usize,
}

fn main() -> std::io::Result<()> {
    use sixtyfps_compiler::*;
    let args = Cli::from_args();
    let source = std::fs::read_to_string(&args.path)?;
    let (syntax_node, mut diag) = parser::parse(&source);
    let tr = typeregister::TypeRegister::builtin();
    let tree = object_tree::Document::from_node(syntax_node, &mut diag, &tr);
    if !diag.inner.is_empty() {
        diag.print(args.path.to_string_lossy().into_owned(), source);
        std::process::exit(-1);
    }

    use corelib::abi::primitives::{Image, Rectangle};

    // FIXME: thus obviously is unsafe and not great
    let mut rtti = HashMap::new();
    rtti.insert(
        "Rectangle",
        RuntimeTypeInfo {
            vtable: &corelib::abi::primitives::RectangleVTable as _,
            construct: construct::<Rectangle>,
            properties: [
                ("x", (Rectangle::field_offsets().x, set_property::<f32> as _)),
                ("y", (Rectangle::field_offsets().y, set_property::<f32> as _)),
                ("width", (Rectangle::field_offsets().width, set_property::<f32> as _)),
                ("height", (Rectangle::field_offsets().height, set_property::<f32> as _)),
                ("color", (Rectangle::field_offsets().color, set_property::<u32> as _)),
            ]
            .iter()
            .cloned()
            .collect(),
            size: std::mem::size_of::<Rectangle>(),
        },
    );
    rtti.insert(
        "Image",
        RuntimeTypeInfo {
            vtable: &corelib::abi::primitives::ImageVTable as _,
            construct: construct::<Image>,
            properties: [
                ("x", (Image::field_offsets().x, set_property::<f32> as _)),
                ("y", (Image::field_offsets().y, set_property::<f32> as _)),
                ("width", (Image::field_offsets().width, set_property::<f32> as _)),
                ("height", (Image::field_offsets().height, set_property::<f32> as _)),
                ("source", (Image::field_offsets().source, set_property::<*const i8> as _)),
            ]
            .iter()
            .cloned()
            .collect(),
            size: std::mem::size_of::<Image>(),
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
