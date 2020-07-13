/*
The FieldOffster structure is forked from https://docs.rs/field-offset/0.3.0/src/field_offset/lib.rs.html

The changes include:
 - Only the FieldOffset structure was imported, not the macros
 - re-export of the FieldOffsets derive macro
 - add const in most method
 - Add a PinFlag flag

(there is a `//###` comment in front of some change)

*/

//### added no_std  (because we can)
#![no_std]

#[cfg(test)]
extern crate alloc;

use core::fmt;
use core::marker::PhantomData;
use core::mem;
use core::ops::Add;
use core::pin::Pin;

#[doc(inline)]
pub use const_field_offset_macro::FieldOffsets;

/// Represents a pointer to a field of type `U` within the type `T`
///
/// The `Flag` parameter can be set to `AllowPin` to enable the projection
/// from Pin<&T> to Pin<&U>
#[repr(transparent)]
pub struct FieldOffset<T, U, PinFlag = NotPinnedFlag>(
    /// Offset in bytes of the field within the struct
    usize,
    /// ### Changed from Fn in order to allow const.
    /// Should be fn(T)->U,  but we can't make that work in const context,
    /// so use the PhantomData2 indirection
    ///
    /// ```compile_fail
    /// use const_field_offset::FieldOffset;
    /// struct Foo<'a>(&'a str);
    /// fn test<'a>(foo: &Foo<'a>, of: FieldOffset<Foo<'static>, &'static str>) -> &'static str {
    ///     let of2 : FieldOffset<Foo<'a>, &'static str> = of; // This must not compile
    ///     of2.apply(foo)
    /// }
    /// ```
    /// that should compile
    /// ```
    /// use const_field_offset::FieldOffset;
    /// struct Foo<'a>(&'a str, &'static str);
    /// fn test<'a>(foo: &'a Foo<'static>, of: FieldOffset<Foo, &'static str>) -> &'a str {
    ///     let of2 : FieldOffset<Foo<'static>, &'static str> = of;
    ///     of.apply(foo)
    /// }
    /// fn test2(foo: &Foo<'static>, of: FieldOffset<Foo, &'static str>) -> &'static str {
    ///     let of2 : FieldOffset<Foo<'static>, &'static str> = of;
    ///     of.apply(foo)
    /// }
    /// fn test3<'a>(foo: &'a Foo, of: FieldOffset<Foo<'a>, &'a str>) -> &'a str {
    ///     of.apply(foo)
    /// }
    /// ```
    PhantomData<(PhantomContra<T>, *const U, PinFlag)>,
);

/// Type that can be used in the `Flag` parameter of `FieldOffset` to specify that
/// This projection is valid on Pin types.
/// See documentation of `FieldOffset::new_from_offset_pinned`
pub enum PinnedFlag {}

/// Type that can be used in the `Flag` parameter of `FieldOffset` to specify that
/// This projection is valid on Pin types.
pub enum NotPinnedFlag {}

#[doc(hidden)]
mod internal {
    use super::*;
    pub trait CombineFlag {
        type Output;
    }
    impl CombineFlag for (PinnedFlag, PinnedFlag) {
        type Output = PinnedFlag;
    }
    impl CombineFlag for (NotPinnedFlag, PinnedFlag) {
        type Output = NotPinnedFlag;
    }
    impl CombineFlag for (PinnedFlag, NotPinnedFlag) {
        type Output = NotPinnedFlag;
    }
    impl CombineFlag for (NotPinnedFlag, NotPinnedFlag) {
        type Output = NotPinnedFlag;
    }
}

/// `fn` cannot appear dirrectly in a type that need to be const.
/// Workaround that with an indiretion
struct PhantomContra<T>(fn(T));

impl<T, U> FieldOffset<T, U, NotPinnedFlag> {
    // Use MaybeUninit to get a fake T
    #[cfg(fieldoffset_maybe_uninit)]
    #[inline]
    fn with_uninit_ptr<R, F: FnOnce(*const T) -> R>(f: F) -> R {
        let uninit = mem::MaybeUninit::<T>::uninit();
        f(uninit.as_ptr())
    }

    // Use a dangling pointer to get a fake T
    #[cfg(not(fieldoffset_maybe_uninit))]
    #[inline]
    fn with_uninit_ptr<R, F: FnOnce(*const T) -> R>(f: F) -> R {
        f(mem::align_of::<T>() as *const T)
    }

    /// Construct a field offset via a lambda which returns a reference
    /// to the field in question.
    ///
    /// # Safety
    ///
    /// The lambda *must not* dereference the provided pointer or access the
    /// inner value in any way as it may point to uninitialized memory.
    ///
    /// For the returned `FieldOffset` to be safe to use, the returned pointer
    /// must be valid for *any* instance of `T`. For example, returning a pointer
    /// to a field from an enum with multiple variants will produce a `FieldOffset`
    /// which is unsafe to use.
    pub unsafe fn new<F: for<'a> FnOnce(*const T) -> *const U>(f: F) -> Self {
        let offset = Self::with_uninit_ptr(|base_ptr| {
            let field_ptr = f(base_ptr);
            (field_ptr as usize).wrapping_sub(base_ptr as usize)
        });

        // Construct an instance using the offset
        Self::new_from_offset(offset)
    }
    /// Construct a field offset directly from a byte offset.
    ///
    /// # Safety
    ///
    /// For the returned `FieldOffset` to be safe to use, the field offset
    /// must be valid for *any* instance of `T`. For example, returning the offset
    /// to a field from an enum with multiple variants will produce a `FieldOffset`
    /// which is unsafe to use.
    #[inline]
    pub const unsafe fn new_from_offset(offset: usize) -> Self {
        // ### made const so assert is not allowed
        // Sanity check: ensure that the field offset plus the field size
        // is no greater than the size of the containing struct. This is
        // not sufficient to make the function *safe*, but it does catch
        // obvious errors like returning a reference to a boxed value,
        // which is owned by `T` and so has the correct lifetime, but is not
        // actually a field.
        //assert!(offset + mem::size_of::<U>() <= mem::size_of::<T>());

        FieldOffset(offset, PhantomData)
    }
}

// Methods for applying the pointer to member
impl<T, U, Flag> FieldOffset<T, U, Flag> {
    /// Apply the field offset to a native pointer.
    #[inline]
    pub fn apply_ptr(self, x: *const T) -> *const U {
        ((x as usize) + self.0) as *const U
    }
    /// Apply the field offset to a native mutable pointer.
    #[inline]
    pub fn apply_ptr_mut(self, x: *mut T) -> *mut U {
        ((x as usize) + self.0) as *mut U
    }
    /// Apply the field offset to a reference.
    #[inline]
    pub fn apply<'a>(self, x: &'a T) -> &'a U {
        unsafe { &*self.apply_ptr(x) }
    }
    /// Apply the field offset to a mutable reference.
    #[inline]
    pub fn apply_mut<'a>(self, x: &'a mut T) -> &'a mut U {
        unsafe { &mut *self.apply_ptr_mut(x) }
    }
    /// Get the raw byte offset for this field offset.
    #[inline]
    pub const fn get_byte_offset(self) -> usize {
        self.0
    }

    // Methods for unapplying the pointer to member

    /// Unapply the field offset to a native pointer.
    ///
    /// # Safety
    ///
    /// *Warning: very unsafe!*
    ///
    /// This applies a negative offset to a pointer. If the safety
    /// implications of this are not already clear to you, then *do
    /// not* use this method. Also be aware that Rust has stronger
    /// aliasing rules than other languages, so it may be UB to
    /// dereference the resulting pointer even if it points to a valid
    /// location, due to the presence of other live references.
    #[inline]
    pub unsafe fn unapply_ptr(self, x: *const U) -> *const T {
        ((x as usize) - self.0) as *const T
    }
    /// Unapply the field offset to a native mutable pointer.
    ///
    /// # Safety
    ///
    /// *Warning: very unsafe!*
    ///
    /// This applies a negative offset to a pointer. If the safety
    /// implications of this are not already clear to you, then *do
    /// not* use this method. Also be aware that Rust has stronger
    /// aliasing rules than other languages, so it may be UB to
    /// dereference the resulting pointer even if it points to a valid
    /// location, due to the presence of other live references.
    #[inline]
    pub unsafe fn unapply_ptr_mut(self, x: *mut U) -> *mut T {
        ((x as usize) - self.0) as *mut T
    }
    /// Unapply the field offset to a reference.
    ///
    /// # Safety
    ///
    /// *Warning: very unsafe!*
    ///
    /// This applies a negative offset to a reference. If the safety
    /// implications of this are not already clear to you, then *do
    /// not* use this method. Also be aware that Rust has stronger
    /// aliasing rules than other languages, so this method may cause UB
    /// even if the resulting reference points to a valid location, due
    /// to the presence of other live references.
    #[inline]
    pub unsafe fn unapply<'a>(self, x: &'a U) -> &'a T {
        &*self.unapply_ptr(x)
    }
    /// Unapply the field offset to a mutable reference.
    ///
    /// # Safety
    ///
    /// *Warning: very unsafe!*
    ///
    /// This applies a negative offset to a reference. If the safety
    /// implications of this are not already clear to you, then *do
    /// not* use this method. Also be aware that Rust has stronger
    /// aliasing rules than other languages, so this method may cause UB
    /// even if the resulting reference points to a valid location, due
    /// to the presence of other live references.
    #[inline]
    pub unsafe fn unapply_mut<'a>(self, x: &'a mut U) -> &'a mut T {
        &mut *self.unapply_ptr_mut(x)
    }

    /// Convert this offset to an offset that is allowed to go from `Pin<&T>`
    /// to `Pin<&U>`
    ///
    /// # Safety
    ///
    /// The Pin safety rules for projection must be respected. These rules are
    /// explained in the
    /// [Pin documentation](https://doc.rust-lang.org/stable/std/pin/index.html#pinning-is-structural-for-field)
    pub const unsafe fn as_pinned_projection(self) -> FieldOffset<T, U, PinnedFlag> {
        FieldOffset::new_from_offset_pinned(self.get_byte_offset())
    }

    /// Remove the PinnedFlag
    pub const fn as_unpinned_projection(self) -> FieldOffset<T, U> {
        unsafe { FieldOffset::new_from_offset(self.get_byte_offset()) }
    }
}

impl<T, U> FieldOffset<T, U, PinnedFlag> {
    /// Construct a field offset directly from a byte offset, which can be projected from
    /// a pinned.
    ///
    /// # Safety
    ///
    /// In addition to the safety rules of FieldOffset::new_from_offset, the projection
    /// from `Pin<&T>` to `Pin<&U>` must also be allowed. The rules are explained in the
    /// [Pin documentation](https://doc.rust-lang.org/stable/std/pin/index.html#pinning-is-structural-for-field)
    #[inline]
    pub const unsafe fn new_from_offset_pinned(offset: usize) -> Self {
        FieldOffset(offset, PhantomData)
    }

    /// Apply the field offset to a reference.
    #[inline]
    pub fn apply_pin<'a>(self, x: Pin<&'a T>) -> Pin<&'a U> {
        unsafe { x.map_unchecked(|x| self.apply(x)) }
    }
    /// Apply the field offset to a mutable reference.
    #[inline]
    pub fn apply_pin_mut<'a>(self, x: Pin<&'a mut T>) -> Pin<&'a mut U> {
        unsafe { x.map_unchecked_mut(|x| self.apply_mut(x)) }
    }

    /// Unapply the field offset to a reference.
    ///
    /// # Safety
    ///
    /// *Warning: very unsafe!*
    ///
    /// This applies a negative offset to a reference. If the safety
    /// implications of this are not already clear to you, then *do
    /// not* use this method. Also be aware that Rust has stronger
    /// aliasing rules than other languages, so this method may cause UB
    /// even if the resulting reference points to a valid location, due
    /// to the presence of other live references.
    #[inline]
    pub unsafe fn unapply_pin<'a>(self, x: Pin<&'a U>) -> Pin<&'a T> {
        x.map_unchecked(|x| self.unapply(x))
    }
}

impl<T, U> From<FieldOffset<T, U, PinnedFlag>> for FieldOffset<T, U> {
    fn from(other: FieldOffset<T, U, PinnedFlag>) -> Self {
        unsafe { Self::new_from_offset(other.get_byte_offset()) }
    }
}

/// Allow chaining pointer-to-members.
///
/// Applying the resulting field offset is equivalent to applying the first
/// field offset, then applying the second field offset.
///
/// The requirements on the generic type parameters ensure this is a safe operation.
impl<T, U, V, F1, F2> Add<FieldOffset<U, V, F1>> for FieldOffset<T, U, F2>
where
    (F1, F2): internal::CombineFlag,
{
    type Output = FieldOffset<T, V, <(F1, F2) as internal::CombineFlag>::Output>;
    #[inline]
    fn add(self, other: FieldOffset<U, V, F1>) -> Self::Output {
        FieldOffset(self.0 + other.0, PhantomData)
    }
}

/// The debug implementation prints the byte offset of the field in hexadecimal.
impl<T, U, Flag> fmt::Debug for FieldOffset<T, U, Flag> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "FieldOffset({:#x})", self.0)
    }
}

impl<T, U, Flag> Copy for FieldOffset<T, U, Flag> {}
impl<T, U, Flag> Clone for FieldOffset<T, U, Flag> {
    fn clone(&self) -> Self {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as const_field_offset;
    // ### Structures were change to repr(c) and to inherit FieldOffsets

    // Example structs
    #[derive(Debug, FieldOffsets)]
    #[repr(C)]
    struct Foo {
        a: u32,
        b: f64,
        c: bool,
    }

    #[derive(Debug, FieldOffsets)]
    #[repr(C)]
    struct Bar {
        x: u32,
        y: Foo,
    }

    #[test]
    fn test_simple() {
        // Get a pointer to `b` within `Foo`
        let foo_b = Foo::field_offsets().b;

        // Construct an example `Foo`
        let mut x = Foo { a: 1, b: 2.0, c: false };

        // Apply the pointer to get at `b` and read it
        {
            let y = foo_b.apply(&x);
            assert!(*y == 2.0);
        }

        // Apply the pointer to get at `b` and mutate it
        {
            let y = foo_b.apply_mut(&mut x);
            *y = 42.0;
        }
        assert!(x.b == 42.0);
    }

    #[test]
    fn test_nested() {
        // Construct an example `Foo`
        let mut x = Bar { x: 0, y: Foo { a: 1, b: 2.0, c: false } };

        // Combine the pointer-to-members
        let bar_y_b = Bar::field_offsets().y + Foo::field_offsets().b;

        // Apply the pointer to get at `b` and mutate it
        {
            let y = bar_y_b.apply_mut(&mut x);
            *y = 42.0;
        }
        assert!(x.y.b == 42.0);
    }

    #[test]
    fn test_pin() {
        use ::alloc::boxed::Box;
        // Get a pointer to `b` within `Foo`
        let foo_b = Foo::field_offsets().b;
        let foo_b_pin = unsafe { foo_b.as_pinned_projection() };
        let foo = Box::pin(Foo { a: 21, b: 22.0, c: true });
        let pb: Pin<&f64> = foo_b_pin.apply_pin(foo.as_ref());
        assert!(*pb == 22.0);

        let mut x = Box::pin(Bar { x: 0, y: Foo { a: 1, b: 52.0, c: false } });
        let bar_y_b = Bar::field_offsets().y + foo_b_pin;
        assert!(*bar_y_b.apply(&*x) == 52.0);

        let bar_y_pin = unsafe { Bar::field_offsets().y.as_pinned_projection() };
        *(bar_y_pin + foo_b_pin).apply_pin_mut(x.as_mut()) = 12.;
        assert!(x.y.b == 12.0);
    }
}

pub trait ConstFieldOffset: Copy {
    /// The type of the container
    type Container;
    /// The type of the field
    type Field;

    /// Can be PinnedFlag or NotPinnedFlag
    type PinFlag;

    const OFFSET: FieldOffset<Self::Container, Self::Field, Self::PinFlag>;

    fn as_field_offset(self) -> FieldOffset<Self::Container, Self::Field, Self::PinFlag> {
        Self::OFFSET
    }
    fn get_byte_offset(self) -> usize {
        Self::OFFSET.get_byte_offset()
    }
    fn apply(self, x: &Self::Container) -> &Self::Field {
        Self::OFFSET.apply(x)
    }
    fn apply_mut(self, x: &mut Self::Container) -> &mut Self::Field {
        Self::OFFSET.apply_mut(x)
    }

    fn apply_pin<'a>(self, x: Pin<&'a Self::Container>) -> Pin<&'a Self::Field>
    where
        Self: ConstFieldOffset<PinFlag = PinnedFlag>,
    {
        Self::OFFSET.apply_pin(x)
    }
    fn apply_pin_mut<'a>(self, x: Pin<&'a mut Self::Container>) -> Pin<&'a mut Self::Field>
    where
        Self: ConstFieldOffset<PinFlag = PinnedFlag>,
    {
        Self::OFFSET.apply_pin_mut(x)
    }
}

#[derive(Copy, Clone)]
pub struct ConstFieldOffsetSum<A: ConstFieldOffset, B: ConstFieldOffset>(pub A, pub B);

impl<A: ConstFieldOffset, B: ConstFieldOffset> ConstFieldOffset for ConstFieldOffsetSum<A, B>
where
    A: ConstFieldOffset<Field = B::Container>,
    (A::PinFlag, B::PinFlag): internal::CombineFlag,
{
    type Container = A::Container;
    type Field = B::Field;
    type PinFlag = <(A::PinFlag, B::PinFlag) as internal::CombineFlag>::Output;
    const OFFSET: FieldOffset<Self::Container, Self::Field, Self::PinFlag> =
        FieldOffset(A::OFFSET.get_byte_offset() + B::OFFSET.get_byte_offset(), PhantomData);
}

impl<A: ConstFieldOffset, B: ConstFieldOffset, Other> ::core::ops::Add<Other>
    for ConstFieldOffsetSum<A, B>
where
    Self: ConstFieldOffset,
    Other: ConstFieldOffset<Container = <Self as ConstFieldOffset>::Field>,
{
    type Output = ConstFieldOffsetSum<Self, Other>;
    fn add(self, other: Other) -> Self::Output {
        ConstFieldOffsetSum(self, other)
    }
}
