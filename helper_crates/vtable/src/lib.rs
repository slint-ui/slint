use core::ops::{Deref, DerefMut, Drop};
use core::ptr::NonNull;
use core::marker::PhantomData;
pub use vtable_macro::vtable;

pub unsafe trait VTableMeta {
    /// that's the rust trait.  (e.g: `Hello`)
    type Trait: ?Sized;
    /// that's the vtable struct `HelloVTable`
    type VTable;

    /// That's the trait object that implements the trait.
    /// NOTE: the size must be 2*size_of<usize>
    type TraitObject: Copy;


    /// That maps from the tait object from the trait iteself
    /// (In other word, return 'to' since 'to' implements trait,
    /// but we can't represent that in rust right now, hence these helper)
    ///
    /// Safety: the trait object need to be pointing to valid pointer / vtable
    unsafe fn map_to(to: &Self::TraitObject) -> &Self::Trait;
    /// Same as map_to, but mutable
    unsafe fn map_to_mut(to: &mut Self::TraitObject) -> &mut Self::Trait;

    /// Return a raw pointer to the inside of the impl
    unsafe fn get_ptr(from: &Self::TraitObject) -> NonNull<u8>;

    /// Create a trait object from its raw parts
    unsafe fn from_raw(vtable: NonNull<Self::VTable>, ptr: NonNull<u8>) -> Self::TraitObject;

    /// return a reference to the vtable
    unsafe fn get_vtable(from: &Self::TraitObject) -> &Self::VTable;


}

pub trait VTableMetaDrop: VTableMeta {
    /// Safety: the traitobject need to be pointing to a valid allocated pointer
    unsafe fn drop(ptr: Self::TraitObject);
}

// These checks are not enough to ensure that this is not unsafe.
fn sanity_checks<T: ?Sized + VTableMeta>() {
    debug_assert_eq!(core::mem::size_of::<T::TraitObject>(), 2 * core::mem::size_of::<usize>());
}

#[repr(C)]
pub struct VBox<T: ?Sized + VTableMetaDrop> {
    inner: T::TraitObject,
}

impl<T: ?Sized + VTableMetaDrop> Deref for VBox<T> {
    type Target = T::Trait;
    fn deref(&self) -> &Self::Target {
        sanity_checks::<T>();
        unsafe { T::map_to(&self.inner) }
    }
}
impl<T: ?Sized + VTableMetaDrop> DerefMut for VBox<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        sanity_checks::<T>();
        unsafe { T::map_to_mut(&mut self.inner) }
    }
}

impl<T: ?Sized + VTableMetaDrop> Drop for VBox<T> {
    fn drop(&mut self) {
        unsafe {
            T::drop(self.inner);
        }
    }
}

impl<T: ?Sized + VTableMetaDrop> VBox<T> {
    pub unsafe fn from_inner(inner: T::TraitObject) -> Self {
        Self { inner }
    }
    pub unsafe fn inner(x: &Self) -> T::TraitObject {
        x.inner
    }
    pub unsafe fn get_ptr(x: &Self) -> NonNull<u8> {
        T::get_ptr(&x.inner)
    }
    pub unsafe fn from_raw(vtable: NonNull<T::VTable>, ptr: NonNull<u8>) -> Self {
        Self {inner : T::from_raw(vtable, ptr)}
    }
    pub fn get_vtable(&self) -> &T::VTable {
        unsafe { T::get_vtable(&self.inner) }
    }
}

/*
impl<T: ?Sized + VTableMeta> VBox<T> {
    /// Construct the box from raw pointer of a vtable and a corresponding pointer
    pub unsafe fn from_inner(
        vtable: core::ptr::NonNull<#vtable_name>,
        ptr: core::ptr::NonNull<#impl_name>,
    ) -> Self {
        Self{inner: #to_name{vtable, ptr}}
    }

    /*pub fn vtable(&self) -> & #vtable_name {
        unsafe { self.inner.vtable.as_ref() }
    }*/

   /* pub fn get_ptr(&self) -> *mut #impl_name {
        self.inner.ptr.get_ptr()
    }*/
}
*/

pub struct VRef<'a, T: ?Sized + VTableMeta> {
    inner: T::TraitObject,
    _phantom: PhantomData<&'a T::Trait>,
}

// Need to implement manually otheriwse it is not implemented if T do not implement Copy / Clone
impl<'a, T: ?Sized + VTableMeta> Copy for VRef<'a, T> {}

impl<'a, T: ?Sized + VTableMeta> Clone for VRef<'a, T> {
    fn clone(&self) -> Self {
        Self { inner: self.inner, _phantom: self._phantom }
    }
}

impl<'a, T: ?Sized + VTableMeta> Deref for VRef<'a, T> {
    type Target = T::Trait;
    fn deref(&self) -> &Self::Target {
        sanity_checks::<T>();
        unsafe { T::map_to(&self.inner) }
    }
}

impl<'a, T: ?Sized + VTableMeta> VRef<'a, T> {
    pub unsafe fn from_inner(inner: T::TraitObject) -> Self {
        Self { inner, _phantom: PhantomData }
    }
    pub unsafe fn inner(x: &Self) -> T::TraitObject {
        x.inner
    }
    pub unsafe fn get_ptr(x: &Self) -> NonNull<u8> {
        T::get_ptr(&x.inner)
    }
    pub unsafe fn from_raw(vtable: NonNull<T::VTable>, ptr: NonNull<u8>) -> Self {
        Self {inner : T::from_raw(vtable, ptr), _phantom: PhantomData }
    }
    pub fn get_vtable(&self) -> &T::VTable {
        unsafe { T::get_vtable(&self.inner) }
    }
}

pub struct VRefMut<'a, T: ?Sized + VTableMeta> {
    inner: T::TraitObject,
    _phantom: PhantomData<&'a mut T::Trait>,
}

impl<'a, T: ?Sized + VTableMeta> Deref for VRefMut<'a, T> {
    type Target = T::Trait;
    fn deref(&self) -> &Self::Target {
        sanity_checks::<T>();
        unsafe { T::map_to(&self.inner) }
    }
}

impl<'a, T: ?Sized + VTableMeta> DerefMut for VRefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        sanity_checks::<T>();
        unsafe { T::map_to_mut(&mut self.inner) }
    }
}

impl<'a, T: ?Sized + VTableMeta> VRefMut<'a, T> {
    pub unsafe fn from_inner(inner: T::TraitObject) -> Self {
        Self { inner, _phantom: PhantomData }
    }
    pub unsafe fn inner(x: &Self) -> T::TraitObject {
        x.inner
    }
    pub unsafe fn get_ptr(x: &Self) -> NonNull<u8> {
        T::get_ptr(&x.inner)
    }
    pub fn borrow<'b>(&'b self) -> VRef<'b, T> {
        unsafe { VRef::from_inner(VRefMut::inner(self)) }
    }
    pub fn borrow_mut<'b>(&'b mut self) -> VRefMut<'b, T> {
        unsafe { VRefMut::from_inner(VRefMut::inner(self)) }
    }
    pub fn into_ref(self) -> VRef<'a, T> {
        unsafe { VRef::from_inner(VRefMut::inner(&self)) }
    }
    pub fn get_vtable(&self) -> &T::VTable {
        unsafe { T::get_vtable(&self.inner) }
    }
}
