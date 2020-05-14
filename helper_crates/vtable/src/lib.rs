use core::ops::{Deref, DerefMut, Drop};
//use core::ptr::NonNull;
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

   /* pub fn as_ptr(&self) -> *mut #impl_name {
        self.inner.ptr.as_ptr()
    }*/
}
*/
#[derive(Clone, Copy)]
pub struct VRef<'a, T: ?Sized + VTableMeta> {
    inner: T::TraitObject,
    _phantom: core::marker::PhantomData<&'a T::Trait>,
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
        Self { inner, _phantom: core::marker::PhantomData }
    }
    pub unsafe fn inner(x: &Self) -> T::TraitObject {
        x.inner
    }
}

pub struct VRefMut<'a, T: ?Sized + VTableMeta> {
    inner: T::TraitObject,
    _phantom: core::marker::PhantomData<&'a mut T::Trait>,
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
        Self { inner, _phantom: core::marker::PhantomData }
    }
    pub unsafe fn inner(x: &Self) -> T::TraitObject {
        x.inner
    }
}
