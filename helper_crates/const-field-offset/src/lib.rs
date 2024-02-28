// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

/*!
This crate expose the [`FieldOffsets`] derive macro and the types it uses.

The macro allows to get const FieldOffset for member of a `#[repr(C)]` struct.

The [`FieldOffset`] type is re-exported from the `field-offset` crate.
*/
#![no_std]

#[cfg(test)]
extern crate alloc;

use core::pin::Pin;

#[doc(inline)]
pub use const_field_offset_macro::FieldOffsets;

pub use field_offset::{AllowPin, FieldOffset, NotPinned};

/// This trait needs to be implemented if you use the `#[pin_drop]` attribute. It enables
/// you to implement Drop for your type safely.
pub trait PinnedDrop {
    /// This is the equivalent to the regular Drop trait with the difference that self
    /// is pinned.
    fn drop(self: Pin<&mut Self>);

    #[doc(hidden)]
    fn do_safe_pinned_drop(&mut self) {
        let p = unsafe { Pin::new_unchecked(self) };
        p.drop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as const_field_offset;
    // ### Structures were change to repr(c) and to inherit FieldOffsets

    // Example structures
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
    #[allow(clippy::float_cmp)] // We want bit-wise equality here
    fn test_simple() {
        // Get a pointer to `b` within `Foo`
        let foo_b = Foo::FIELD_OFFSETS.b;

        // Construct an example `Foo`
        let mut x = Foo { a: 1, b: 2.0, c: false };

        // Apply the pointer to get at `b` and read it
        {
            let y = foo_b.apply(&x);
            assert_eq!(*y, 2.0);
        }

        // Apply the pointer to get at `b` and mutate it
        {
            let y = foo_b.apply_mut(&mut x);
            *y = 42.0;
        }
        assert_eq!(x.b, 42.0);
    }

    #[test]
    #[allow(clippy::float_cmp)] // We want bit-wise equality here
    fn test_nested() {
        // Construct an example `Foo`
        let mut x = Bar { x: 0, y: Foo { a: 1, b: 2.0, c: false } };

        // Combine the pointer-to-members
        let bar_y_b = Bar::FIELD_OFFSETS.y + Foo::FIELD_OFFSETS.b;

        // Apply the pointer to get at `b` and mutate it
        {
            let y = bar_y_b.apply_mut(&mut x);
            *y = 42.0;
        }
        assert_eq!(x.y.b, 42.0);
    }

    #[test]
    #[allow(clippy::float_cmp)] // We want bit-wise equality here
    fn test_pin() {
        use ::alloc::boxed::Box;
        // Get a pointer to `b` within `Foo`
        let foo_b = Foo::FIELD_OFFSETS.b;
        let foo_b_pin = unsafe { foo_b.as_pinned_projection() };
        let foo_object = Box::pin(Foo { a: 21, b: 22.0, c: true });
        let pb: Pin<&f64> = foo_b_pin.apply_pin(foo_object.as_ref());
        assert_eq!(*pb, 22.0);

        let mut x = Box::pin(Bar { x: 0, y: Foo { a: 1, b: 52.0, c: false } });
        let bar_y_b = Bar::FIELD_OFFSETS.y + foo_b_pin;
        assert_eq!(*bar_y_b.apply(&*x), 52.0);

        let bar_y_pin = unsafe { Bar::FIELD_OFFSETS.y.as_pinned_projection() };
        *(bar_y_pin + foo_b_pin).apply_pin_mut(x.as_mut()) = 12.;
        assert_eq!(x.y.b, 12.0);
    }
}

/**
Test that one can't implement Unpin for pinned struct

This should work:

```
#[derive(const_field_offset::FieldOffsets)]
#[repr(C)]
#[pin]
struct MyStructPin { a: u32 }
```

But this not:

```compile_fail
#[derive(const_field_offset::FieldOffsets)]
#[repr(C)]
#[pin]
struct MyStructPin { a: u32 }
impl Unpin for MyStructPin {};
```

*/
#[cfg(doctest)]
const NO_IMPL_UNPIN: u32 = 0;

#[doc(hidden)]
#[cfg(feature = "field-offset-trait")]
mod internal {
    use super::*;
    pub trait CombineFlag {
        type Output;
    }
    impl CombineFlag for (AllowPin, AllowPin) {
        type Output = AllowPin;
    }
    impl CombineFlag for (NotPinned, AllowPin) {
        type Output = NotPinned;
    }
    impl CombineFlag for (AllowPin, NotPinned) {
        type Output = NotPinned;
    }
    impl CombineFlag for (NotPinned, NotPinned) {
        type Output = NotPinned;
    }
}

#[cfg(feature = "field-offset-trait")]
pub trait ConstFieldOffset: Copy {
    /// The type of the container
    type Container;
    /// The type of the field
    type Field;

    /// Can be AllowPin or NotPinned
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
        Self: ConstFieldOffset<PinFlag = AllowPin>,
    {
        Self::OFFSET.apply_pin(x)
    }
    fn apply_pin_mut<'a>(self, x: Pin<&'a mut Self::Container>) -> Pin<&'a mut Self::Field>
    where
        Self: ConstFieldOffset<PinFlag = AllowPin>,
    {
        Self::OFFSET.apply_pin_mut(x)
    }
}

/// This can be used to transmute a FieldOffset from a NotPinned to any pin flag.
/// This is only valid if we know that the offset is actually valid for this Flag.
#[cfg(feature = "field-offset-trait")]
union TransmutePinFlag<Container, Field, PinFlag> {
    x: FieldOffset<Container, Field, PinFlag>,
    y: FieldOffset<Container, Field>,
}

/// Helper class used as the result of the addition of two types that implement the `ConstFieldOffset` trait
#[derive(Copy, Clone)]
#[cfg(feature = "field-offset-trait")]
pub struct ConstFieldOffsetSum<A: ConstFieldOffset, B: ConstFieldOffset>(pub A, pub B);

#[cfg(feature = "field-offset-trait")]
impl<A: ConstFieldOffset, B: ConstFieldOffset> ConstFieldOffset for ConstFieldOffsetSum<A, B>
where
    A: ConstFieldOffset<Field = B::Container>,
    (A::PinFlag, B::PinFlag): internal::CombineFlag,
{
    type Container = A::Container;
    type Field = B::Field;
    type PinFlag = <(A::PinFlag, B::PinFlag) as internal::CombineFlag>::Output;
    const OFFSET: FieldOffset<Self::Container, Self::Field, Self::PinFlag> = unsafe {
        TransmutePinFlag {
            y: FieldOffset::new_from_offset(
                A::OFFSET.get_byte_offset() + B::OFFSET.get_byte_offset(),
            ),
        }
        .x
    };
}

#[cfg(feature = "field-offset-trait")]
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
