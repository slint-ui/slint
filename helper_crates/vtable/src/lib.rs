/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
/*!
This crate allows you to create ffi-friendly virtual tables.

## Features

 - A `#[vtable]` macro to annotate a VTable struct to generate the traits and structure
   to safely work with it.
 - `VRef`/`VRefMut`/`VBox` types. They are fat reference/box types which wrap a pointer to
   the vtable, and a pointer to the object.
 - Ability to store constants in a vtable.
 - These constants can even be a field offset.

## Example of use:

```
use vtable::*;
// we are going to declare a VTable structure for an Animal trait
#[vtable]
#[repr(C)]
struct AnimalVTable {
    /// pointer to a function that makes a noise.  The `VRef<AnimalVTable>` is the type of
    /// the self object.
    ///
    /// Note: the #[vtable] macro will automatically add `extern "C"` if that is missing.
    make_noise: fn(VRef<AnimalVTable>, i32) -> i32,

    /// if there is a 'drop' member, it is considered as the destructor.
    drop: fn(VRefMut<AnimalVTable>),
}

struct Dog(i32);

// The #[vtable] macro created the Animal Trait
impl Animal for Dog {
    fn make_noise(&self, intensity: i32) -> i32 {
        println!("Wof!");
        return self.0 * intensity;
    }
}

// the vtable macro also exposed a macro to create a vtable
AnimalVTable_static!(static DOG_VT for Dog);

// with that, it is possible to instantiate a VBox
let animal_box = VBox::<AnimalVTable>::new(Dog(42));
assert_eq!(animal_box.make_noise(2), 42 * 2);
```

The `#[vtable]` macro created the "Animal" trait.

Note that the `#[vtable]` macro is applied to the VTable struct so
that `cbindgen` can see the actual vtable.


*/

#[doc(no_inline)]
pub use const_field_offset::*;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut, Drop};
use core::{pin::Pin, ptr::NonNull};
#[doc(inline)]
pub use vtable_macro::*;

/// Internal trait that is implemented by the `#[vtable]` macro.
///
/// Safety: The Target object needs to be implemented correctly.
/// And there should be a VTable::VTable::new<T> function that returns a
/// VTable suitable for the type T.
pub unsafe trait VTableMeta {
    /// That's the trait object that implements the functions
    ///
    /// NOTE: the size must be `2*size_of::<usize>`
    /// and a `repr(C)` with `(vtable, ptr)` so it has the same layout as
    /// the inner and VBox/VRef/VRefMut
    type Target;

    /// That's the VTable itself (so most likely Self)
    type VTable: 'static;
}

/// This trait is implemented by the `#[vtable]` macro.
///
/// It is implemented if the macro has a "drop" function.
pub trait VTableMetaDrop: VTableMeta {
    /// Safety: the Target needs to be pointing to a valid allocated pointer
    unsafe fn drop(ptr: *mut Self::Target);
    fn new_box<X: HasStaticVTable<Self>>(value: X) -> VBox<Self>;
}

/// Allow to associate a VTable with a type.
///
/// Safety: the VTABLE and STATIC_VTABLE need to be a a valid virtual table
/// corresponding to pointer to Self instance.
pub unsafe trait HasStaticVTable<VT>
where
    VT: ?Sized + VTableMeta,
{
    /// Safety: must be a valid VTable for Self
    fn static_vtable() -> &'static VT::VTable;
}

#[derive(Copy, Clone)]
/// The inner structure of VRef, VRefMut, and VBox.
///
/// Invariant: _vtable and _ptr are valid pointer for the lifetime of the container.
/// _ptr is an instance of the object represented by _vtable
#[allow(dead_code)]
#[repr(C)]
struct Inner {
    vtable: *const u8,
    ptr: *const u8,
}

impl Inner {
    /// Transmute a reference to self into a reference to T::Target.
    fn deref<T: ?Sized + VTableMeta>(&self) -> *const T::Target {
        debug_assert_eq!(core::mem::size_of::<T::Target>(), core::mem::size_of::<Inner>());
        self as *const Inner as *const T::Target
    }
}

/// An equivalent of a Box that holds a pointer to a VTable and a pointer to an instance.
/// A VBox frees the instance when dropped.
///
/// The type parameter is supposed to be the VTable type.
///
/// The VBox implements Deref so one can access all the members of the vtable.
///
/// This is only valid if the VTable has a `drop` type (so that the `#[vtable]` macro
/// implements the `VTableMetaDrop` trait for it)
#[repr(transparent)]
pub struct VBox<T: ?Sized + VTableMetaDrop> {
    inner: Inner,
    phantom: PhantomData<T::Target>,
}

impl<T: ?Sized + VTableMetaDrop> Deref for VBox<T> {
    type Target = T::Target;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner.deref::<T>() }
    }
}
impl<T: ?Sized + VTableMetaDrop> DerefMut for VBox<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.inner.deref::<T>() as *mut _) }
    }
}

impl<T: ?Sized + VTableMetaDrop> Drop for VBox<T> {
    fn drop(&mut self) {
        unsafe {
            T::drop(self.inner.deref::<T>() as *mut _);
        }
    }
}

impl<T: ?Sized + VTableMetaDrop> VBox<T> {
    /// Create a new VBox from an instance of a type that can be associated with a VTable.
    ///
    /// Will move the instance on the heap.
    ///
    /// (the `HasStaticVTable` is implemented by the `“MyTrait”VTable_static!` macro generated by
    /// the #[vtable] macro)
    pub fn new<X: HasStaticVTable<T>>(value: X) -> Self {
        T::new_box(value)
    }

    /// Safety: the `ptr` needs to be a valid for the `vtable`, and properly allocated so it can be dropped
    pub unsafe fn from_raw(vtable: NonNull<T::VTable>, ptr: NonNull<u8>) -> Self {
        Self {
            inner: Inner { vtable: vtable.cast().as_ptr(), ptr: ptr.cast().as_ptr() },
            phantom: PhantomData,
        }
    }

    /// Gets a VRef pointing to this box
    pub fn borrow<'b>(&'b self) -> VRef<'b, T> {
        unsafe { VRef::from_inner(self.inner) }
    }

    /// Gets a VRefMut pointing to this box
    pub fn borrow_mut<'b>(&'b mut self) -> VRefMut<'b, T> {
        unsafe { VRefMut::from_inner(self.inner) }
    }

    /// Leaks the content of the box.
    pub fn leak(self) -> VRefMut<'static, T> {
        let inner = self.inner;
        core::mem::forget(self);
        unsafe { VRefMut::from_inner(inner) }
    }
}

/// `VRef<'a MyTraitVTable>` can be thought as a `&'a dyn MyTrait`
///
/// It will dereference to a structure that has the same members as MyTrait.
#[repr(transparent)]
pub struct VRef<'a, T: ?Sized + VTableMeta> {
    inner: Inner,
    phantom: PhantomData<&'a T::Target>,
}

// Need to implement manually otheriwse it is not implemented if T do not implement Copy / Clone
impl<'a, T: ?Sized + VTableMeta> Copy for VRef<'a, T> {}

impl<'a, T: ?Sized + VTableMeta> Clone for VRef<'a, T> {
    fn clone(&self) -> Self {
        Self { inner: self.inner, phantom: PhantomData }
    }
}

impl<'a, T: ?Sized + VTableMeta> Deref for VRef<'a, T> {
    type Target = T::Target;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner.deref::<T>() }
    }
}

impl<'a, T: ?Sized + VTableMeta> VRef<'a, T> {
    /// Create a new VRef from an reference of a type that can be associated with a VTable.
    ///
    /// (the `HasStaticVTable` is implemented by the `“MyTrait”VTable_static!` macro generated by
    /// the #[vtable] macro)
    pub fn new<X: HasStaticVTable<T>>(value: &'a X) -> Self {
        Self {
            inner: Inner {
                vtable: X::static_vtable() as *const T::VTable as *const u8,
                ptr: value as *const X as *const u8,
            },
            phantom: PhantomData,
        }
    }

    /// Create a new Pin<VRef<_>> from a pinned reference. This is similar to `VRef::new`.
    pub fn new_pin<X: HasStaticVTable<T>>(value: core::pin::Pin<&'a X>) -> Pin<Self> {
        // Since Value is pinned, this means it is safe to construct a Pin
        unsafe {
            Pin::new_unchecked(Self {
                inner: Inner {
                    vtable: X::static_vtable() as *const T::VTable as *const u8,
                    ptr: value.get_ref() as *const X as *const u8,
                },
                phantom: PhantomData,
            })
        }
    }

    unsafe fn from_inner(inner: Inner) -> Self {
        Self { inner, phantom: PhantomData }
    }

    /// Safety: the `ptr` needs to be a valid for the `vtable`, and properly allocated so it can be dropped
    pub unsafe fn from_raw(vtable: NonNull<T::VTable>, ptr: NonNull<u8>) -> Self {
        Self {
            inner: Inner { vtable: vtable.cast().as_ptr(), ptr: ptr.cast().as_ptr() },
            phantom: PhantomData,
        }
    }

    /// Return a reference of the given type if the type is matching.
    pub fn downcast<X: HasStaticVTable<T>>(&self) -> Option<&X> {
        if self.inner.vtable == X::static_vtable() as *const _ as *const u8 {
            // Safety: We just checked that the vtable fits
            unsafe { Some(&*(self.inner.ptr as *const X)) }
        } else {
            None
        }
    }

    /// Return a reference of the given type if the type is matching
    pub fn downcast_pin<X: HasStaticVTable<T>>(this: Pin<Self>) -> Option<Pin<&'a X>> {
        let inner = unsafe { Pin::into_inner_unchecked(this).inner };
        if inner.vtable == X::static_vtable() as *const _ as *const u8 {
            // Safety: We just checked that the vtable fits
            unsafe { Some(Pin::new_unchecked(&*(inner.ptr as *const X))) }
        } else {
            None
        }
    }
}

/// `VRefMut<'a MyTraitVTable>` can be thought as a `&'a mut dyn MyTrait`
///
/// It will dereference to a structure that has the same members as MyTrait.
#[repr(transparent)]
pub struct VRefMut<'a, T: ?Sized + VTableMeta> {
    inner: Inner,
    phantom: PhantomData<&'a mut T::Target>,
}

impl<'a, T: ?Sized + VTableMeta> Deref for VRefMut<'a, T> {
    type Target = T::Target;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner.deref::<T>() }
    }
}

impl<'a, T: ?Sized + VTableMeta> DerefMut for VRefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.inner.deref::<T>() as *mut _) }
    }
}

impl<'a, T: ?Sized + VTableMeta> VRefMut<'a, T> {
    /// Create a new VRef from a mutable reference of a type that can be associated with a VTable.
    ///
    /// (the `HasStaticVTable` is implemented by the `“MyTrait”VTable_static!` macro generated by
    /// the #[vtable] macro)
    pub fn new<X: HasStaticVTable<T>>(value: &'a mut X) -> Self {
        Self {
            inner: Inner {
                vtable: X::static_vtable() as *const T::VTable as *const u8,
                ptr: value as *mut X as *const u8,
            },
            phantom: PhantomData,
        }
    }

    unsafe fn from_inner(inner: Inner) -> Self {
        Self { inner, phantom: PhantomData }
    }

    /// Safety: the `ptr` needs to be a valid for the `vtable`, and properly allocated so it can be dropped
    pub unsafe fn from_raw(vtable: NonNull<T::VTable>, ptr: NonNull<u8>) -> Self {
        Self {
            inner: Inner { vtable: vtable.cast().as_ptr(), ptr: ptr.cast().as_ptr() },
            phantom: PhantomData,
        }
    }

    /// Borrow this to obtain a VRef.
    pub fn borrow<'b>(&'b self) -> VRef<'b, T> {
        unsafe { VRef::from_inner(self.inner) }
    }

    /// Borrow this to obtain a new VRefMut.
    pub fn borrow_mut<'b>(&'b mut self) -> VRefMut<'b, T> {
        unsafe { VRefMut::from_inner(self.inner) }
    }

    /// Create a VRef with the same lifetime as the original lifetime.
    pub fn into_ref(self) -> VRef<'a, T> {
        unsafe { VRef::from_inner(self.inner) }
    }

    /// Return a reference of the given type if the type is matching.
    pub fn downcast<X: HasStaticVTable<T>>(&mut self) -> Option<&mut X> {
        if self.inner.vtable == X::static_vtable() as *const _ as *const u8 {
            // Safety: We just checked that the vtable fits
            unsafe { Some(&mut *(self.inner.ptr as *mut X)) }
        } else {
            None
        }
    }
}

/** Creates a `VRef` or a `VRefMut` suitable for an instance that implements the trait

When possible, `VRef::new` or `VRefMut::new` should be preferred, as they use a static vtable.
But when using the generated `XxxVTable_static!` macro that is not possible and this macro can be
used instead.
Note that the `downcast` will not work with references created with this macro.

```
use vtable::*;
#[vtable]
struct MyVTable { /* ... */ }
struct Something { /* ... */};
impl My for Something {};

let mut s = Something { /* ... */};
// declare a my_vref variable for the said VTable
new_vref!(let my_vref : VRef<MyVTable> for My = &s);

// same but mutable
new_vref!(let mut my_vref_m : VRefMut<MyVTable> for My = &mut s);

```
*/
#[macro_export]
macro_rules! new_vref {
    (let $ident:ident : VRef<$vtable:ty> for $trait_:path = $e:expr) => {
        // ensure that the type of the expression is correct
        let vtable = {
            use $crate::VTableMeta;
            fn get_vt<X: $trait_>(_: &X) -> <$vtable as VTableMeta>::VTable {
                <$vtable as VTableMeta>::VTable::new::<X>()
            }
            get_vt($e)
        };

        let $ident = {
            use $crate::VTableMeta;
            fn create<'a, X: $trait_>(
                vtable: &'a <$vtable as VTableMeta>::VTable,
                val: &'a X,
            ) -> $crate::VRef<'a, <$vtable as VTableMeta>::VTable> {
                use ::core::ptr::NonNull;
                // Safety: we constructed the vtable such that it fits for the value
                unsafe { $crate::VRef::from_raw(NonNull::from(vtable), NonNull::from(val).cast()) }
            }
            create(&vtable, $e)
        };
    };
    (let mut $ident:ident : VRefMut<$vtable:ty> for $trait_:path = $e:expr) => {
        // ensure that the type of the expression is correct
        let vtable = {
            use $crate::VTableMeta;
            fn get_vt<X: $trait_>(_: &X) -> <$vtable as VTableMeta>::VTable {
                <$vtable as VTableMeta>::VTable::new::<X>()
            }
            get_vt($e)
        };

        let mut $ident = {
            use $crate::VTableMeta;
            fn create<'a, X: $trait_>(
                vtable: &'a <$vtable as VTableMeta>::VTable,
                val: &'a mut X,
            ) -> $crate::VRefMut<'a, <$vtable as VTableMeta>::VTable> {
                use ::core::ptr::NonNull;
                // Safety: we constructed the vtable such that it fits for the value
                unsafe {
                    $crate::VRefMut::from_raw(NonNull::from(vtable), NonNull::from(val).cast())
                }
            }
            create(&vtable, $e)
        };
    };
}

/// Represents an offset to a field of type matching the vtable, within the Base container structure.
#[repr(C)]
pub struct VOffset<Base, T: ?Sized + VTableMeta, PinFlag = NotPinned> {
    vtable: &'static T::VTable,
    /// Safety invariant: the vtable is valid, and the field at the given offset within Base is
    /// matching with the vtable
    offset: usize,
    phantom: PhantomData<FieldOffset<Base, (), PinFlag>>,
}

impl<Base, T: ?Sized + VTableMeta, PinFlag> core::fmt::Debug for VOffset<Base, T, PinFlag> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "VOffset({})", self.offset)
    }
}

impl<Base, T: ?Sized + VTableMeta, Flag> VOffset<Base, T, Flag> {
    #[inline]
    pub fn apply<'a>(self, x: &'a Base) -> VRef<'a, T> {
        let ptr = x as *const Base as *const u8;
        unsafe {
            VRef::from_raw(
                NonNull::from(self.vtable),
                NonNull::new_unchecked(ptr.add(self.offset) as *mut _),
            )
        }
    }

    #[inline]
    pub fn apply_mut<'a>(self, x: &'a mut Base) -> VRefMut<'a, T> {
        let ptr = x as *mut Base as *mut u8;
        unsafe {
            VRefMut::from_raw(
                NonNull::from(self.vtable),
                NonNull::new_unchecked(ptr.add(self.offset)),
            )
        }
    }

    #[inline]
    pub fn new<X: HasStaticVTable<T>>(o: FieldOffset<Base, X, Flag>) -> Self {
        Self { vtable: X::static_vtable(), offset: o.get_byte_offset(), phantom: PhantomData }
    }

    /// Create a new VOffset from raw data
    ///
    /// Safety: there must be a field that matches the vtable at offset T in base.
    #[inline]
    pub unsafe fn from_raw(vtable: &'static T::VTable, offset: usize) -> Self {
        Self { vtable, offset, phantom: PhantomData }
    }
}

impl<Base, T: ?Sized + VTableMeta> VOffset<Base, T, AllowPin> {
    #[inline]
    pub fn apply_pin<'a>(self, x: Pin<&'a Base>) -> Pin<VRef<'a, T>> {
        let ptr = x.get_ref() as *const Base as *mut u8;
        unsafe {
            Pin::new_unchecked(VRef::from_raw(
                NonNull::from(self.vtable),
                NonNull::new_unchecked(ptr.add(self.offset)),
            ))
        }
    }
}

// Need to implement manually otheriwse it is not implemented if T do not implement Copy / Clone
impl<Base, T: ?Sized + VTableMeta, Flag> Copy for VOffset<Base, T, Flag> {}

impl<Base, T: ?Sized + VTableMeta, Flag> Clone for VOffset<Base, T, Flag> {
    fn clone(&self) -> Self {
        Self { vtable: self.vtable, offset: self.offset, phantom: PhantomData }
    }
}

#[cfg(doctest)]
mod compile_fail_tests;
