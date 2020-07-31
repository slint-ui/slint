/*!
This create provides weak pointers for `Pin<std::rc::Rc<T>>` and  `Pin<std::rc::Arc<T>>`

## Motivation

`Pin<std::rc::Rc<T>>` and `Pin<std::rc::Arc<T>>` cannot be converted safely to
their `Weak<T>` equivalent if `T` does not implement `Unpin`.
That's because it would otherwise be possible to do something like this:

```
# use std::{pin::Pin, marker::PhantomPinned, rc::{Rc, Weak}};
struct SomeStruct(PhantomPinned);
let pinned = Rc::pin(SomeStruct(PhantomPinned));

// This is unsafe ...
let weak = unsafe {
    Rc::downgrade(&Pin::into_inner_unchecked(pinned.clone()))
};

// ... because otherwise it would be possible to move the content of pinned:
let mut unpinned_rc = weak.upgrade().unwrap();
std::mem::drop((pinned, weak));
// unpinned_rc is now the only reference so this will work:
let x = std::mem::replace(
    Rc::get_mut(&mut unpinned_rc).unwrap(),
    SomeStruct(PhantomPinned),
);
```

In that example, `x` is the original `SomeStruct` which we moved in memory,
**that is undefined behavior**, do not do that at home.

## `WeakPin`

This crate simply provide a `rc::WeakPin` and `sync::WeakPin` which allow to
get weak pointer from `Pin<std::rc::Rc>` and `Pin<srd::sync::Arc>`.

This is safe because you can one can only get back a `Pin` out of it when
trying to upgrade the weak pointer.

`WeakPin` can be created using the `WeakPin` downgrade function.

## Example

```
use pin_weak::rc::*;
# use std::marker::PhantomPinned;
struct SomeStruct(PhantomPinned, usize);
let pinned = Rc::pin(SomeStruct(PhantomPinned, 42));
let weak = WeakPin::downgrade(pinned.clone());
assert_eq!(weak.upgrade().unwrap().1, 42);
std::mem::drop(pinned);
assert!(weak.upgrade().is_none());
```

*/

#![no_std]
extern crate alloc;

/// The implementation is in a macro because it is repeated for Arc and Rc
macro_rules! implementation {
    ($Rc:ident, $Weak:ident, $rc_lit:literal) => {
        #[doc(no_inline)]
        /// re-exported for convinience
        pub use core::pin::Pin;
        /// This is a safe wrapper around something that could be compared to `Pin<Weak<T>>`
        ///
        /// The typical way to obtain a `WeakPin` is to call `WeakPin::downgrade`
        #[derive(Debug)]
        pub struct WeakPin<T: ?Sized>(Weak<T>);
        impl<T> Default for WeakPin<T> {
            fn default() -> Self { Self(Weak::default()) }
        }
        impl<T: ?Sized> Clone for WeakPin<T> {
            fn clone(&self) -> Self { Self(self.0.clone()) }
        }
        impl<T: ?Sized> WeakPin<T> {
            /// Equivalent function to `
            #[doc = $rc_lit]
            /// ::downgrade`,  but taking a `Pin<
            #[doc = $rc_lit]
            /// <T>>` instead.
            pub fn downgrade(rc: Pin<$Rc<T>>) -> Self {
                // Safety: we will never return anythning else than a Pin<Rc>
                unsafe { Self($Rc::downgrade(&Pin::into_inner_unchecked(rc))) }
            }
            /// Equivalent function to `Weak::upgrade` but returning a `Pin<
            #[doc = $rc_lit]
            /// <T>>` instead.
            pub fn upgrade(&self) -> Option<Pin<$Rc<T>>> {
                // Safety: the weak was contructed from a Pin<Rc<T>>
                self.0.upgrade().map(|rc| unsafe { Pin::new_unchecked(rc) })
            }
        }

        #[test]
        fn test() {
            struct Foo {
                _p: core::marker::PhantomPinned,
                u: u32,
            }
            impl Foo {
                fn new(u: u32) -> Self {
                    Self { _p: core::marker::PhantomPinned, u }
                }
            }
            let c = $Rc::pin(Foo::new(44));
            let weak1 = WeakPin::downgrade(c.clone());
            assert_eq!(weak1.upgrade().unwrap().u, 44);
            assert_eq!(weak1.clone().upgrade().unwrap().u, 44);
            let weak2 = WeakPin::downgrade(c.clone());
            assert_eq!(weak2.upgrade().unwrap().u, 44);
            assert_eq!(weak1.upgrade().unwrap().u, 44);
            // note that this moves c and therefore it will be dropped
            let weak3 = WeakPin::downgrade(c);
            assert!(weak3.upgrade().is_none());
            assert!(weak2.upgrade().is_none());
            assert!(weak1.upgrade().is_none());
            assert!(weak1.clone().upgrade().is_none());

            let def = WeakPin::<alloc::boxed::Box<&'static mut ()>>::default();
            assert!(def.upgrade().is_none());
            assert!(def.clone().upgrade().is_none());
        }
    };
}

pub mod rc {
    #[doc(no_inline)]
    /// re-exported for convinience
    pub use alloc::rc::{Rc, Weak};
    implementation! {Rc, Weak, "Rc"}
}

pub mod sync {
    #[doc(no_inline)]
    /// re-exported for convinience
    pub use alloc::sync::{Arc, Weak};
    implementation! {Arc, Weak, "Arc"}
}
