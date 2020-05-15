use core::marker::PhantomData;
use core::ops::{Deref, DerefMut, Drop};
use core::ptr::NonNull;
pub use vtable_macro::*;

pub unsafe trait VTableMeta {
    /// That's the trait object that implements the functions
    /// NOTE: the size must be 2*size_of::<usize>
    /// and a repr(C) with (vtable, ptr) so it has the same layout as
    /// the inner and VBox/VRef/VRefMut
    type Target;

    /// That's the VTable itself (so most likely Self)
    type VTable;
}

pub trait VTableMetaDrop: VTableMeta {
    /// Safety: the Target need to be pointing to a valid allocated pointer
    unsafe fn drop(ptr: *mut Self::Target);
}

#[derive(Copy, Clone)]
/// The inner structure of VRef, VRefMut, and VBox.
///
/// Invariant: _vtable and _ptr are valid pointer for the lifetime of the container.
/// _ptr is an instance of the object represented by _vtable
#[allow(dead_code)]
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

#[repr(C)]
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
    pub unsafe fn from_raw(vtable: NonNull<T::VTable>, ptr: NonNull<u8>) -> Self {
        Self {
            inner: Inner { vtable: vtable.cast().as_ptr(), ptr: ptr.cast().as_ptr() },
            phantom: PhantomData,
        }
    }
    pub fn borrow<'b>(&'b self) -> VRef<'b, T> {
        unsafe { VRef::from_inner(self.inner) }
    }
    pub fn borrow_mut<'b>(&'b mut self) -> VRefMut<'b, T> {
        unsafe { VRefMut::from_inner(self.inner) }
    }
}

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
    unsafe fn from_inner(inner: Inner) -> Self {
        Self { inner, phantom: PhantomData }
    }
    pub unsafe fn from_raw(vtable: NonNull<T::VTable>, ptr: NonNull<u8>) -> Self {
        Self {
            inner: Inner { vtable: vtable.cast().as_ptr(), ptr: ptr.cast().as_ptr() },
            phantom: PhantomData,
        }
    }
}

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
    unsafe fn from_inner(inner: Inner) -> Self {
        Self { inner, phantom: PhantomData }
    }
    pub unsafe fn from_raw(vtable: NonNull<T::VTable>, ptr: NonNull<u8>) -> Self {
        Self {
            inner: Inner { vtable: vtable.cast().as_ptr(), ptr: ptr.cast().as_ptr() },
            phantom: PhantomData,
        }
    }
    pub fn borrow<'b>(&'b self) -> VRef<'b, T> {
        unsafe { VRef::from_inner(self.inner) }
    }
    pub fn borrow_mut<'b>(&'b mut self) -> VRefMut<'b, T> {
        unsafe { VRefMut::from_inner(self.inner) }
    }
    pub fn into_ref(self) -> VRef<'a, T> {
        unsafe { VRef::from_inner(self.inner) }
    }
}

#[cfg(doctest)]
mod compile_fail_tests;
