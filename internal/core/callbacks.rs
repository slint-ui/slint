// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

/*!
Callback that can be connected to one single handler.

TODO: reconsider if we should rename that to `Event`
but then it should also be renamed everywhere, including in the language grammar
*/

#![warn(missing_docs)]

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
use core::cell::Cell;

/// A Callback that can be connected to a handler.
///
/// The Arg represents the argument. It should always be a tuple
///
#[repr(C)]
pub struct Callback<Arg: ?Sized, Ret = ()> {
    /// FIXME: `Box<dyn>` is a fat object and we probably want to put an erased type in there
    handler: Cell<Option<Box<dyn FnMut(&Arg, &mut Ret)>>>,
}

impl<Arg: ?Sized, Ret> Default for Callback<Arg, Ret> {
    fn default() -> Self {
        Self { handler: Default::default() }
    }
}

impl<Arg: ?Sized, Ret: Default> Callback<Arg, Ret> {
    /// Call the callback with the given argument.
    pub fn call(&self, a: &Arg) -> Ret {
        let mut r = Ret::default();
        if let Some(mut h) = self.handler.take() {
            h(a, &mut r);
            assert!(self.handler.take().is_none(), "Callback Handler set while called");
            self.handler.set(Some(h));
        }
        r
    }

    /// Return whether a callback is registered or not.
    pub fn has_handler(&self) -> bool {
        let handler = self.handler.take();
        let result = handler.is_some();
        self.handler.set(handler);
        result
    }

    /// Set an handler to be called when the callback is called
    ///
    /// There can only be one single handler per callback.
    pub fn set_handler(&self, mut f: impl FnMut(&Arg) -> Ret + 'static) {
        self.handler.set(Some(Box::new(move |a: &Arg, r: &mut Ret| *r = f(a))));
    }
}

#[test]
fn callback_simple_test() {
    use std::rc::Rc;
    #[derive(Default)]
    struct Component {
        pressed: core::cell::Cell<bool>,
        clicked: Callback<()>,
    }
    let c = Rc::new(Component::default());
    let weak = Rc::downgrade(&c);
    c.clicked.set_handler(move |()| weak.upgrade().unwrap().pressed.set(true));
    c.clicked.call(&());
    assert!(c.pressed.get());
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[allow(non_camel_case_types)]
    type c_void = ();
    #[repr(C)]
    /// Has the same layout as Callback<_>
    pub struct CallbackOpaque(*const c_void, *const c_void);

    static_assertions::assert_eq_align!(CallbackOpaque, Callback<()>);
    static_assertions::assert_eq_size!(CallbackOpaque, Callback<()>);
    static_assertions::assert_eq_align!(CallbackOpaque, Callback<(alloc::string::String,)>);
    static_assertions::assert_eq_size!(CallbackOpaque, Callback<(alloc::string::String,)>);

    /// Initialize the callback.
    /// slint_callback_drop must be called.
    #[no_mangle]
    pub unsafe extern "C" fn slint_callback_init(out: *mut CallbackOpaque) {
        assert_eq!(core::mem::size_of::<CallbackOpaque>(), core::mem::size_of::<Callback<()>>());
        core::ptr::write(out as *mut Callback<()>, Default::default());
    }

    /// Emit the callback
    #[no_mangle]
    pub unsafe extern "C" fn slint_callback_call(
        sig: *const CallbackOpaque,
        arg: *const c_void,
        ret: *mut c_void,
    ) {
        let sig = &*(sig as *const Callback<c_void>);
        if let Some(mut h) = sig.handler.take() {
            h(&*arg, &mut *ret);
            assert!(sig.handler.take().is_none(), "Callback Handler set while called");
            sig.handler.set(Some(h));
        }
    }

    /// Set callback handler.
    ///
    /// The binding has signature fn(user_data)
    #[no_mangle]
    pub unsafe extern "C" fn slint_callback_set_handler(
        sig: *const CallbackOpaque,
        binding: extern "C" fn(user_data: *mut c_void, arg: *const c_void, ret: *mut c_void),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) {
        let sig = &mut *(sig as *mut Callback<c_void>);

        struct UserData {
            user_data: *mut c_void,
            drop_user_data: Option<extern "C" fn(*mut c_void)>,
            binding: extern "C" fn(user_data: *mut c_void, arg: *const c_void, ret: *mut c_void),
        }

        impl Drop for UserData {
            fn drop(&mut self) {
                if let Some(x) = self.drop_user_data {
                    x(self.user_data)
                }
            }
        }

        impl UserData {
            /// Safety: the arguments must be valid pointers
            unsafe fn call(&self, arg: *const c_void, ret: *mut c_void) {
                (self.binding)(self.user_data, arg, ret)
            }
        }

        let ud = UserData { user_data, drop_user_data, binding };
        sig.handler.set(Some(Box::new(move |a: &(), r: &mut ()| {
            ud.call(a as *const c_void, r as *mut c_void)
        })));
    }

    /// Destroy callback
    #[no_mangle]
    pub unsafe extern "C" fn slint_callback_drop(handle: *mut CallbackOpaque) {
        core::ptr::drop_in_place(handle as *mut Callback<()>);
    }
}
