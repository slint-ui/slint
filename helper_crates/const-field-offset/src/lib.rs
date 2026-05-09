// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

/*!
This crate provides the [`FieldOffsets`] derive macro to obtain constant
[`FieldOffset`]s for fields of a `#[repr(C)]` struct.

The [`FieldOffset`] type is re-exported from the `field-offset` crate.

## Usage

```rust
use const_field_offset::{FieldOffsets, FieldOffset};

#[derive(FieldOffsets)]
#[repr(C)]
struct Foo { a: u32, b: f64 }

let foo_b: FieldOffset<Foo, f64> = Foo::FIELD_OFFSETS.b();
assert_eq!(*foo_b.apply(&Foo { a: 1, b: 42.0 }), 42.0);
```

The `FIELD_OFFSETS` constant is a zero-sized type with a const fn method
per field. Each method returns the corresponding [`FieldOffset`].

For pin-projecting offsets, use `#[pin]`:

```rust
use const_field_offset::{FieldOffsets, FieldOffset, AllowPin};

#[derive(FieldOffsets)]
#[repr(C)]
#[pin]
struct Foo { a: u32, b: f64 }

let foo_b: FieldOffset<Foo, f64, AllowPin> = Foo::FIELD_OFFSETS.b();
let pinned = Box::pin(Foo { a: 1, b: 42.0 });
assert_eq!(*foo_b.apply_pin(pinned.as_ref()), 42.0);
```

The `#[pin]` attribute enforces that the struct is `!Unpin` and does not
implement `Drop` (use `#[pin_drop]` with [`PinnedDrop`] instead).
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
    #[allow(clippy::float_cmp)]
    fn test_simple() {
        let foo_b = Foo::FIELD_OFFSETS.b();
        let mut x = Foo { a: 1, b: 2.0, c: false };
        assert_eq!(*foo_b.apply(&x), 2.0);
        *foo_b.apply_mut(&mut x) = 42.0;
        assert_eq!(x.b, 42.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_nested() {
        let mut x = Bar { x: 0, y: Foo { a: 1, b: 2.0, c: false } };
        let bar_y_b = Bar::FIELD_OFFSETS.y() + Foo::FIELD_OFFSETS.b();
        *bar_y_b.apply_mut(&mut x) = 42.0;
        assert_eq!(x.y.b, 42.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_pin() {
        use alloc::boxed::Box;
        let foo_b = Foo::FIELD_OFFSETS.b();
        let foo_b_pin = unsafe { foo_b.as_pinned_projection() };
        let foo_object = Box::pin(Foo { a: 21, b: 22.0, c: true });
        let pb: Pin<&f64> = foo_b_pin.apply_pin(foo_object.as_ref());
        assert_eq!(*pb, 22.0);

        let mut x = Box::pin(Bar { x: 0, y: Foo { a: 1, b: 52.0, c: false } });
        let bar_y_b = Bar::FIELD_OFFSETS.y() + foo_b_pin;
        assert_eq!(*bar_y_b.apply(&*x), 52.0);

        let bar_y_pin = unsafe { Bar::FIELD_OFFSETS.y().as_pinned_projection() };
        *(bar_y_pin + foo_b_pin).apply_pin_mut(x.as_mut()) = 12.;
        assert_eq!(x.y.b, 12.0);
    }

    // Verify FIELD_OFFSETS is const-evaluable
    const _CONST_FOO_B: FieldOffset<Foo, f64> = Foo::FIELD_OFFSETS.b();
    const _CONST_BAR_Y: FieldOffset<Bar, Foo> = Bar::FIELD_OFFSETS.y();
}

/**
Test that one can't implement Unpin for pinned struct

This should work:

```rust
use const_field_offset::FieldOffsets;
#[repr(C)]
#[derive(FieldOffsets)]
struct Foo {
    x: u32,
}
impl Unpin for Foo {}
```

But not this:
```compile_fail
use const_field_offset::FieldOffsets;
#[repr(C)]
#[derive(FieldOffsets)]
#[pin]
struct Foo {
    x: u32,
}
impl Unpin for Foo {}
```
*/
#[cfg(doctest)]
const NO_IMPL_UNPIN: u32 = 0;
