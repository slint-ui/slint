/*!
 This module create dynamic types
*/

use core::alloc::Layout;
use std::rc::Rc;

unsafe fn construct_fn<T: Default>(ptr: *mut u8) {
    core::ptr::write(ptr as *mut T, T::default());
}
unsafe fn drop_fn<T>(ptr: *mut u8) {
    core::ptr::read(ptr as *const T);
}

#[derive(Copy, Clone)]
pub struct StaticTypeInfo {
    construct: Option<unsafe fn(*mut u8)>,
    drop: Option<unsafe fn(*mut u8)>,
    mem_layout: Layout,
}

impl StaticTypeInfo {
    pub fn new<T: Default>() -> StaticTypeInfo {
        let drop = if core::mem::needs_drop::<T>() { Some(drop_fn::<T> as _) } else { None };
        StaticTypeInfo { construct: Some(construct_fn::<T>), drop, mem_layout: Layout::new::<T>() }
    }
}

struct FieldInfo {
    construct: Option<unsafe fn(*mut u8)>,
    drop: Option<unsafe fn(*mut u8)>,
    offset: usize,
}

pub struct TypeInfo {
    mem_layout: core::alloc::Layout,
    fields: Vec<FieldInfo>,
}

pub struct TypeBuilder {
    /// max alignement in byte of the types
    align: usize,
    /// Size in byte of the tpye so far (not including the trailling padding)
    size: usize,
    fields: Vec<FieldInfo>,
}

impl TypeBuilder {
    pub fn new() -> Self {
        let mut s = Self { align: 1, size: 0, fields: vec![] };
        type T = Rc<TypeInfo>;
        s.add_field(StaticTypeInfo {
            construct: None,
            drop: Some(drop_fn::<T>),
            mem_layout: Layout::new::<T>(),
        });
        s
    }

    pub fn add_field_type<T: Default>(&mut self) -> usize {
        self.add_field(StaticTypeInfo::new::<T>())
    }

    pub fn add_field(&mut self, ty: StaticTypeInfo) -> usize {
        let align = ty.mem_layout.align();
        let len_rounded_up = self.size.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1);

        self.fields.push(FieldInfo {
            construct: ty.construct,
            drop: ty.drop,
            offset: len_rounded_up,
        });
        self.size = len_rounded_up + ty.mem_layout.size();
        self.align = self.align.max(align);
        len_rounded_up
    }

    pub fn build(self) -> Rc<TypeInfo> {
        let size = self.size.wrapping_add(self.align).wrapping_sub(1) & !self.align.wrapping_sub(1);
        Rc::new(TypeInfo {
            mem_layout: core::alloc::Layout::from_size_align(size, self.align).unwrap(),
            fields: self.fields,
        })
    }
}

impl TypeInfo {
    pub fn create_instance(self: Rc<Self>) -> *mut Instance {
        unsafe {
            let mem = std::alloc::alloc(self.mem_layout);
            std::ptr::write(mem as *mut Rc<_>, self.clone());
            for f in &self.fields {
                if let Some(ctor) = f.construct {
                    ctor(mem.add(f.offset));
                }
            }
            mem as *mut Instance
        }
    }

    pub unsafe fn delete_instance(instance: *mut Instance) {
        let type_info = (*instance).type_info.clone();
        let mem = instance as *mut u8;
        for f in &type_info.fields {
            if let Some(dtor) = f.drop {
                dtor(mem.add(f.offset));
            }
        }
        std::alloc::dealloc(mem, type_info.mem_layout);
    }
}

#[repr(C)]
pub struct Instance {
    type_info: Rc<TypeInfo>,
    _opaque: [u8; 0],
}
