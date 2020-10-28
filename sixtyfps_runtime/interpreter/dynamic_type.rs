/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
 This module create dynamic types

 The main entry point for this module is the TypeBuilder
*/

use core::alloc::Layout;
use generativity::Id;
use sixtyfps_corelib::rtti::FieldOffset;
use std::rc::Rc;

unsafe fn construct_fn<T: Default>(ptr: *mut u8) {
    core::ptr::write(ptr as *mut T, T::default());
}
unsafe fn drop_fn<T>(ptr: *mut u8) {
    core::ptr::read(ptr as *const T);
}

/// Information for type that can be added to a dynamic type.
///
/// Let the builder know how to construct and build these fields
#[derive(Copy, Clone)]
pub struct StaticTypeInfo {
    /// Invariant: this function must be safe to call on a uninitialized memory matching `mem_layout`.
    /// Can only be None if the field is meant to be initialized by another mean (e.g, the type pointer
    /// allocated at the begining of the type)
    construct: Option<unsafe fn(*mut u8)>,
    /// Invariant: this function must be safe to call on an instance created by the `construct` function.
    /// If None, the type does not need drop.
    drop: Option<unsafe fn(*mut u8)>,
    /// Memory latout of the type
    mem_layout: Layout,
}

impl StaticTypeInfo {
    /// Returns a StaticTypeInfo suitable for the type `T`
    pub fn new<T: Default>() -> StaticTypeInfo {
        let drop = if core::mem::needs_drop::<T>() { Some(drop_fn::<T> as _) } else { None };
        StaticTypeInfo { construct: Some(construct_fn::<T>), drop, mem_layout: Layout::new::<T>() }
    }
}

/// Internal structure representing a field within a dynamic type
struct FieldInfo {
    construct: Option<unsafe fn(*mut u8)>,
    drop: Option<unsafe fn(*mut u8)>,
    offset: usize,
}

/// A TypeInfo represents the metadata required to create and drop dynamic type
///
/// It needs to be built with the TypeBuilder.
pub struct TypeInfo<'id> {
    mem_layout: core::alloc::Layout,
    /// Invariant: each field must represent a valid field within the `mem_layout`
    /// and the construct and drop function must be valid so that each field can
    /// be constructed and droped correctly.
    /// The first FieldInfo must be related to the `Rc<TypeInfo>` member at the beginning
    fields: Vec<FieldInfo>,

    #[allow(unused)]
    id: Id<'id>,
}

/// A builder for a dynamic type.
///
/// Call `add_field()` for each type, and then `build()` to return a TypeInfo
pub struct TypeBuilder<'id> {
    /// max alignement in byte of the types
    align: usize,
    /// Size in byte of the tpye so far (not including the trailling padding)
    size: usize,
    fields: Vec<FieldInfo>,
    id: Id<'id>,
}

impl<'id> TypeBuilder<'id> {
    pub fn new(id: generativity::Guard<'id>) -> Self {
        let mut s = Self { align: 1, size: 0, fields: vec![], id: id.into() };
        type T<'id> = Rc<TypeInfo<'id>>;
        s.add_field(StaticTypeInfo {
            construct: None,
            drop: Some(drop_fn::<T<'id>>),
            mem_layout: Layout::new::<T<'id>>(),
        });
        s
    }

    /// Convinience to call add_field with the StaticTypeInfo for a field
    pub fn add_field_type<T: Default>(&mut self) -> FieldOffset<Instance<'id>, T> {
        unsafe { FieldOffset::new_from_offset_pinned(self.add_field(StaticTypeInfo::new::<T>())) }
    }

    /// Add a field in this dynamic type.
    ///
    /// Returns the offset, in bytes, of the added field in within the dinamic type.
    /// This takes care of alignment of the types.
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

    pub fn build(self) -> Rc<TypeInfo<'id>> {
        let size = self.size.wrapping_add(self.align).wrapping_sub(1) & !self.align.wrapping_sub(1);
        Rc::new(TypeInfo {
            mem_layout: core::alloc::Layout::from_size_align(size, self.align).unwrap(),
            fields: self.fields,
            id: self.id,
        })
    }
}

impl<'id> TypeInfo<'id> {
    /// Create an instance of this type.
    ///
    /// The instance will be allocated on the heap.
    /// The instance must be freed with `delete_instance`
    pub fn create_instance(self: Rc<Self>) -> InstanceBox<'id> {
        // Safety: the TypeInfo invariant means that the constructor can be called
        unsafe {
            let mem = std::alloc::alloc(self.mem_layout);
            std::ptr::write(mem as *mut Rc<_>, self.clone());
            for f in &self.fields {
                if let Some(ctor) = f.construct {
                    ctor(mem.add(f.offset));
                }
            }
            InstanceBox(core::ptr::NonNull::new_unchecked(mem as *mut Instance))
        }
    }

    /// Drop and free the memory of this instance
    ///
    /// Saferty, the instance must have been created by `TypeInfo::create_instance`
    unsafe fn delete_instance(instance: *mut Instance) {
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

/// Opaque type that represents something created with `TypeInfo::create_instance`
#[repr(C)]
pub struct Instance<'id> {
    type_info: Rc<TypeInfo<'id>>,
    _opaque: [u8; 0],
}

impl<'id> core::fmt::Debug for Instance<'id> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Instance({:p})", self)
    }
}

/// A pointer to an Instance that automaticaly frees the memory after use
pub struct InstanceBox<'id>(core::ptr::NonNull<Instance<'id>>);

impl<'id> InstanceBox<'id> {
    /// return a pointer to the instance
    pub fn as_ptr(&self) -> core::ptr::NonNull<Instance<'id>> {
        self.0
    }

    pub fn as_pin_ref(&self) -> core::pin::Pin<&Instance<'id>> {
        unsafe { core::pin::Pin::new_unchecked(self.0.as_ref()) }
    }

    pub fn as_mut(&mut self) -> &mut Instance<'id> {
        unsafe { self.0.as_mut() }
    }
}

impl<'id> Drop for InstanceBox<'id> {
    fn drop(&mut self) {
        unsafe { TypeInfo::delete_instance(self.0.as_mut()) }
    }
}
