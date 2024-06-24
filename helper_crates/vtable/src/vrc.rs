// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

// cSpell: ignore pointee repr

//! implementation of vtable::Vrc

use super::*;
use portable_atomic::{AtomicU32, Ordering};

/// This trait is implemented by the [`#[vtable]`](macro@vtable) macro.
///
/// It is implemented if the macro has a "drop_in_place" function.
///
/// # Safety
///
/// The implementation of drop_in_place and dealloc must be correct
pub unsafe trait VTableMetaDropInPlace: VTableMeta {
    /// # Safety
    /// The target ptr argument needs to be pointing to a an instance of the VTable
    /// after the call to this function, the memory is still there but no longer contains
    /// a valid object.
    unsafe fn drop_in_place(vtable: &Self::VTable, ptr: *mut u8) -> vrc::Layout;
    /// # Safety
    /// The target ptr must have been allocated by the same allocator as the
    /// one which the vtable will delegate to.
    unsafe fn dealloc(vtable: &Self::VTable, ptr: *mut u8, layout: vrc::Layout);
}

/// This is a marker type to be used in [`VRc`] and [`VWeak`] to mean that the
/// actual type is not known.
// Note the use of PhantomData to make this type not Send, as a VRc<T, Dyn> cannot be send between thread
pub struct Dyn(PhantomData<*mut ()>);

/// Similar to [`core::alloc::Layout`], but `repr(C)`
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Layout {
    /// The size in bytes
    pub size: usize,
    /// The minimum alignment in bytes
    pub align: usize,
}

impl From<core::alloc::Layout> for Layout {
    fn from(layout: core::alloc::Layout) -> Self {
        Self { size: layout.size(), align: layout.align() }
    }
}

impl core::convert::TryFrom<Layout> for core::alloc::Layout {
    type Error = core::alloc::LayoutError;

    fn try_from(value: Layout) -> Result<Self, Self::Error> {
        Self::from_size_align(value.size, value.align)
    }
}

#[repr(C)]
struct VRcInner<'vt, VTable: VTableMeta, X> {
    vtable: &'vt VTable::VTable,
    /// The amount of VRc pointing to this object. When it reaches 0, the object will be dropped
    strong_ref: AtomicU32,
    /// The amount of VWeak +1. When it reaches 0, the memory will be deallocated.
    /// The +1 is there such as all the VRc together hold a weak reference to the memory
    weak_ref: AtomicU32,
    /// offset to the data from the beginning of VRcInner. This is needed to cast a VRcInner<VT, X>
    /// to VRcInner<VT, u8> as "dyn VRc" and then still be able to get the correct data pointer,
    /// since the alignment of X may not be the same as u8.
    data_offset: u16,
    /// Actual data, or an instance of `Self::Layout` iff `strong_ref == 0`.
    /// Can be seen as `union {data: X, layout: Layout}`  (but that's not stable)
    data: X,
}

impl<'vt, VTable: VTableMeta, X> VRcInner<'vt, VTable, X> {
    unsafe fn data_ptr(s: *const Self) -> *const X {
        (s as *const u8).add(*core::ptr::addr_of!((*s).data_offset) as usize) as *const X
    }
    fn as_ref(&self) -> &X {
        let ptr = self as *const Self as *const u8;
        unsafe { &*(ptr.add(self.data_offset as usize) as *const X) }
    }
}

/// A reference counted pointer to an object matching the virtual table `T`
///
/// Similar to [`alloc::rc::Rc`] where the `VTable` type parameter is a VTable struct
/// annotated with [`#[vtable]`](macro@vtable), and the `X` type parameter is the actual instance.
/// When `X` is the [`Dyn`] type marker, this means that the X is not known and the only
/// thing that can be done is to get a [`VRef<VTable>`] through the [`Self::borrow()`] function.
///
/// Other differences with the [`alloc::rc::Rc`] types are:
/// - It does not allow to access mutable reference. (No `get_mut` or `make_mut`), meaning it is
///   safe to get a Pin reference with `borrow_pin`.
/// - It is safe to pass it across ffi boundaries.
#[repr(transparent)]
pub struct VRc<VTable: VTableMetaDropInPlace + 'static, X = Dyn> {
    inner: NonNull<VRcInner<'static, VTable, X>>,
}

impl<VTable: VTableMetaDropInPlace + 'static, X> Drop for VRc<VTable, X> {
    fn drop(&mut self) {
        unsafe {
            let inner = self.inner.as_ptr();
            if (*inner).strong_ref.fetch_sub(1, Ordering::SeqCst) == 1 {
                let data =
                    (inner as *mut u8).add(*core::ptr::addr_of!((*inner).data_offset) as usize);
                let vtable = core::ptr::addr_of!((*inner).vtable);
                let mut layout = VTable::drop_in_place(*vtable, data);
                layout = core::alloc::Layout::new::<VRcInner<VTable, ()>>()
                    .extend(layout.try_into().unwrap())
                    .unwrap()
                    .0
                    .pad_to_align()
                    .into();
                if (*core::ptr::addr_of!((*inner).weak_ref)).load(Ordering::SeqCst) > 1 {
                    // at this point we are sure that no other thread can access the data
                    // since we still hold a weak reference, so the other weak references
                    // in other thread won't start destroying the object.
                    *(VRcInner::data_ptr(self.inner.cast::<VRcInner<VTable, Layout>>().as_ptr())
                        as *mut Layout) = layout;
                }
                if (*core::ptr::addr_of!((*inner).weak_ref)).fetch_sub(1, Ordering::SeqCst) == 1 {
                    VTable::dealloc(*vtable, self.inner.cast().as_ptr(), layout);
                }
            }
        }
    }
}

impl<VTable: VTableMetaDropInPlace + 'static, X> core::fmt::Debug for VRc<VTable, X> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VRc").field("inner", &self.inner).finish()
    }
}

impl<VTable: VTableMetaDropInPlace, X: HasStaticVTable<VTable>> VRc<VTable, X> {
    /// Create a new VRc from an instance of a type that can be associated with a VTable.
    ///
    /// Will move the instance on the heap.
    ///
    /// (the `HasStaticVTable` is implemented by the `“MyTrait”VTable_static!` macro generated by
    /// the #[vtable] macro)
    pub fn new(data: X) -> Self {
        let layout = core::alloc::Layout::new::<VRcInner<VTable, X>>().pad_to_align();
        // We must ensure the size is enough to hold a Layout when strong_count becomes 0
        let layout_with_layout = core::alloc::Layout::new::<VRcInner<VTable, Layout>>();
        let layout = core::alloc::Layout::from_size_align(
            layout.size().max(layout_with_layout.size()),
            layout.align().max(layout_with_layout.align()),
        )
        .unwrap();
        let mem = unsafe { alloc::alloc::alloc(layout) as *mut VRcInner<VTable, X> };
        let inner = NonNull::new(mem).unwrap();
        assert!(!mem.is_null());

        unsafe {
            mem.write(VRcInner {
                vtable: X::static_vtable(),
                strong_ref: AtomicU32::new(1),
                weak_ref: AtomicU32::new(1), // All the VRc together hold a weak_ref to the memory
                data_offset: 0,
                data,
            });
            (*mem).data_offset =
                (&(*mem).data as *const _ as usize - mem as *const _ as usize) as u16;
            VRc { inner }
        }
    }

    /// Convert a VRc of a real instance to a VRc of a Dyn instance
    pub fn into_dyn(this: Self) -> VRc<VTable, Dyn>
    where
        Self: 'static,
    {
        // Safety: they have the exact same representation: just a pointer to the same structure.
        // no Drop will be called here, so no need to increment any ref count
        unsafe { core::mem::transmute(this) }
    }
}

impl<VTable: VTableMetaDropInPlace + 'static, X: HasStaticVTable<VTable> + 'static> VRc<VTable, X> {
    /// This function allows safely holding a reference to a field inside the VRc. In order to accomplish
    /// that, you need to provide a mapping function `map_fn` in which you need to provide and return a
    /// pinned reference to the object you would like to map. The returned `VRcMapped` allows obtaining
    /// that pinned reference again using [`VRcMapped::as_pin_ref`].
    pub fn map<MappedType: ?Sized>(
        this: Self,
        map_fn: impl for<'r> FnOnce(Pin<&'r X>) -> Pin<&'r MappedType>,
    ) -> VRcMapped<VTable, MappedType> {
        VRcMapped {
            parent_strong: Self::into_dyn(this.clone()),
            object: map_fn(this.as_pin_ref()).get_ref(),
        }
    }
}

impl<VTable: VTableMetaDropInPlace + 'static> VRc<VTable, Dyn> {
    /// This function allows safely holding a reference to a field inside the VRc. In order to accomplish
    /// that, you need to provide a mapping function `map_fn` in which you need to provide and return a
    /// pinned reference to the object you would like to map. The returned `VRcMapped` allows obtaining
    /// that pinned reference again using [`VRcMapped::as_pin_ref`].
    /// This works similar to [`VRc::map`] except that it works on a type-erased VRc.
    pub fn map_dyn<MappedType: ?Sized>(
        this: Self,
        map_fn: impl for<'r> FnOnce(Pin<VRef<'r, VTable>>) -> Pin<&'r MappedType>,
    ) -> VRcMapped<VTable, MappedType> {
        VRcMapped { parent_strong: this.clone(), object: map_fn(Self::borrow_pin(&this)).get_ref() }
    }
}
impl<VTable: VTableMetaDropInPlace, X> VRc<VTable, X> {
    /// Create a Pinned reference to the inner.
    ///
    /// This is safe because we don't allow mutable reference to the inner
    pub fn as_pin_ref(&self) -> Pin<&X> {
        unsafe { Pin::new_unchecked(self) }
    }

    /// Gets a VRef pointing to this instance
    pub fn borrow(this: &Self) -> VRef<'_, VTable> {
        unsafe {
            let inner = this.inner.cast::<VRcInner<VTable, u8>>();
            VRef::from_raw(
                NonNull::from(*::core::ptr::addr_of!((*inner.as_ptr()).vtable)),
                NonNull::new_unchecked(VRcInner::data_ptr(inner.as_ptr()) as *mut u8),
            )
        }
    }

    /// Gets a `Pin<VRef>` pointing to this instance
    ///
    /// This is safe because there is no way to access a mutable reference to the pointee.
    /// (There is no `get_mut` or `make_mut`),
    pub fn borrow_pin(this: &Self) -> Pin<VRef<VTable>> {
        unsafe { Pin::new_unchecked(Self::borrow(this)) }
    }

    /// Construct a [`VWeak`] pointing to this instance.
    pub fn downgrade(this: &Self) -> VWeak<VTable, X> {
        let inner = unsafe { this.inner.as_ref() };
        inner.weak_ref.fetch_add(1, Ordering::SeqCst);
        VWeak { inner: Some(this.inner) }
    }

    /// Gets the number of strong (VRc) pointers to this allocation.
    pub fn strong_count(this: &Self) -> usize {
        unsafe { this.inner.as_ref().strong_ref.load(Ordering::SeqCst) as usize }
    }

    /// Returns true if the two VRc's point to the same allocation
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        this.inner == other.inner
    }
}

impl<VTable: VTableMetaDropInPlace + 'static, X> Clone for VRc<VTable, X> {
    fn clone(&self) -> Self {
        let inner = unsafe { self.inner.as_ref() };
        inner.strong_ref.fetch_add(1, Ordering::SeqCst);
        Self { inner: self.inner }
    }
}

impl<VTable: VTableMetaDropInPlace, X /*+ HasStaticVTable<VTable>*/> Deref for VRc<VTable, X> {
    type Target = X;
    fn deref(&self) -> &Self::Target {
        let inner = unsafe { self.inner.as_ref() };
        inner.as_ref()
    }
}

// Safety: we use atomic reference count for the internal things
unsafe impl<VTable: VTableMetaDropInPlace + Send + Sync + 'static, X: Send + Sync> Send
    for VRc<VTable, X>
{
}
unsafe impl<VTable: VTableMetaDropInPlace + Send + Sync + 'static, X: Send + Sync> Sync
    for VRc<VTable, X>
{
}

/// Weak pointer for the [`VRc`] where `VTable` is a VTable struct, and
/// `X` is the type of the instance, or [`Dyn`] if it is not known
///
/// Similar to [`alloc::rc::Weak`].
///
/// Can be constructed with [`VRc::downgrade`] and use [`VWeak::upgrade`]
/// to re-create the original VRc.
#[repr(transparent)]
pub struct VWeak<VTable: VTableMetaDropInPlace + 'static, X = Dyn> {
    inner: Option<NonNull<VRcInner<'static, VTable, X>>>,
}

impl<VTable: VTableMetaDropInPlace + 'static, X> Default for VWeak<VTable, X> {
    fn default() -> Self {
        Self { inner: None }
    }
}

impl<VTable: VTableMetaDropInPlace + 'static, X> Clone for VWeak<VTable, X> {
    fn clone(&self) -> Self {
        if let Some(inner) = self.inner {
            let inner = unsafe { inner.as_ref() };
            inner.weak_ref.fetch_add(1, Ordering::SeqCst);
        }
        VWeak { inner: self.inner }
    }
}

impl<T: VTableMetaDropInPlace + 'static, X> Drop for VWeak<T, X> {
    fn drop(&mut self) {
        if let Some(i) = self.inner {
            unsafe {
                if (*core::ptr::addr_of!((*i.as_ptr()).weak_ref)).fetch_sub(1, Ordering::SeqCst)
                    == 1
                {
                    // Safety: while allocating, we made sure that the size was big enough to
                    // hold a VRcInner<T, Layout>.
                    let vtable = &*core::ptr::addr_of!((*i.as_ptr()).vtable);
                    let layout = *(VRcInner::data_ptr(i.cast::<VRcInner<T, Layout>>().as_ptr()));
                    T::dealloc(vtable, i.cast().as_ptr(), layout);
                }
            }
        }
    }
}

impl<VTable: VTableMetaDropInPlace + 'static, X> VWeak<VTable, X> {
    /// Returns a new `VRc` if some other instance still holds a strong reference to this item.
    /// Otherwise, returns None.
    pub fn upgrade(&self) -> Option<VRc<VTable, X>> {
        if let Some(i) = self.inner {
            let inner = unsafe { i.as_ref() };
            if inner.strong_ref.load(Ordering::SeqCst) == 0 {
                None
            } else {
                inner.strong_ref.fetch_add(1, Ordering::SeqCst);
                Some(VRc { inner: i })
            }
        } else {
            None
        }
    }

    /// Returns true if the two VWeak instances point to the same allocation
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        this.inner == other.inner
    }
}

impl<VTable: VTableMetaDropInPlace + 'static, X: HasStaticVTable<VTable> + 'static>
    VWeak<VTable, X>
{
    /// Convert a VRc of a real instance to a VRc of a Dyn instance
    pub fn into_dyn(self) -> VWeak<VTable, Dyn> {
        // Safety: they have the exact same representation: just a pointer to the same structure.
        // no Drop will be called here, so no need to increment any ref count
        unsafe { core::mem::transmute(self) }
    }
}

/// Safety: The data VRc manages is held by `VRcInner`, which maintains its address when the VRc
/// is moved.
unsafe impl<VTable: VTableMetaDropInPlace + 'static, X> stable_deref_trait::StableDeref
    for VRc<VTable, X>
{
}

/// Safety: The data VRc manages is held by `VRcInner`, and a clone of a VRc merely clones the pointer
/// *to* the `VRcInner`.
unsafe impl<VTable: VTableMetaDropInPlace + 'static, X> stable_deref_trait::CloneStableDeref
    for VRc<VTable, X>
{
}

/// VRcMapped allows bundling a VRc of a type along with a reference to an object that's
/// reachable through the data the VRc owns and that satisfies the requirements of a Pin.
/// VRCMapped is constructed using [`VRc::map`] and, like VRc, has a weak counterpart, [`VWeakMapped`].
pub struct VRcMapped<VTable: VTableMetaDropInPlace + 'static, MappedType: ?Sized> {
    parent_strong: VRc<VTable, Dyn>,
    object: *const MappedType,
}

impl<VTable: VTableMetaDropInPlace + 'static, MappedType: ?Sized> Clone
    for VRcMapped<VTable, MappedType>
{
    fn clone(&self) -> Self {
        Self { parent_strong: self.parent_strong.clone(), object: self.object }
    }
}

impl<VTable: VTableMetaDropInPlace + 'static, MappedType: ?Sized> VRcMapped<VTable, MappedType> {
    /// Returns a new [`VWeakMapped`] that points to this instance and can be upgraded back to
    /// a [`Self`] as long as a `VRc`/`VMapped` exists.
    pub fn downgrade(this: &Self) -> VWeakMapped<VTable, MappedType> {
        VWeakMapped { parent_weak: VRc::downgrade(&this.parent_strong), object: this.object }
    }

    /// Create a Pinned reference to the mapped type.
    ///
    /// This is safe because the map function returns a pinned reference.
    pub fn as_pin_ref(&self) -> Pin<&MappedType> {
        unsafe { Pin::new_unchecked(self) }
    }

    /// This function allows safely holding a reference to a field inside the `VRcMapped`. In order to accomplish
    /// that, you need to provide a mapping function `map_fn` in which you need to provide and return a
    /// pinned reference to the object you would like to map. The returned `VRcMapped` allows obtaining
    /// that pinned reference again using [`VRcMapped::as_pin_ref`].
    ///
    /// See also [`VRc::map`]
    pub fn map<ReMappedType: ?Sized>(
        this: Self,
        map_fn: impl for<'r> FnOnce(Pin<&'r MappedType>) -> Pin<&'r ReMappedType>,
    ) -> VRcMapped<VTable, ReMappedType> {
        VRcMapped {
            parent_strong: this.parent_strong.clone(),
            object: map_fn(this.as_pin_ref()).get_ref(),
        }
    }

    /// Returns a strong reference to the object that the mapping originates
    /// from.
    pub fn origin(this: &Self) -> VRc<VTable> {
        this.parent_strong.clone()
    }
}

impl<VTable: VTableMetaDropInPlace + 'static, MappedType: ?Sized> Deref
    for VRcMapped<VTable, MappedType>
{
    type Target = MappedType;
    fn deref(&self) -> &Self::Target {
        // Safety: self.object was mapped from self.parent_strong, which the VRc
        // keeps alive *and* pinned.
        unsafe { &*self.object }
    }
}

/// VWeakMapped allows bundling a VWeak with a reference to an object that's reachable
/// from the object a successfully upgraded VWeak points to. VWeakMapped's API consists
/// only of the ability to create clones and to attempt upgrading back to a [`VRcMapped`].
pub struct VWeakMapped<VTable: VTableMetaDropInPlace + 'static, MappedType: ?Sized> {
    parent_weak: VWeak<VTable, Dyn>,
    object: *const MappedType,
}

impl<VTable: VTableMetaDropInPlace + 'static, MappedType: ?Sized> VWeakMapped<VTable, MappedType> {
    /// Returns a new `VRcMapped` if some other instance still holds a strong reference to the owned
    /// object. Otherwise, returns None.
    pub fn upgrade(&self) -> Option<VRcMapped<VTable, MappedType>> {
        self.parent_weak
            .upgrade()
            .map(|parent| VRcMapped { parent_strong: parent, object: self.object })
    }
}

impl<VTable: VTableMetaDropInPlace + 'static, MappedType: ?Sized> Clone
    for VWeakMapped<VTable, MappedType>
{
    fn clone(&self) -> Self {
        Self { parent_weak: self.parent_weak.clone(), object: self.object }
    }
}

impl<VTable: VTableMetaDropInPlace + 'static, MappedType> Default
    for VWeakMapped<VTable, MappedType>
{
    fn default() -> Self {
        Self { parent_weak: VWeak::default(), object: core::ptr::null() }
    }
}
