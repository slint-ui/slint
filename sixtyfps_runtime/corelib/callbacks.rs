/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
Callback that can be connected to  one sigle handler.

TODO: reconsider if we should rename that to `Event`
but then it should also be renamed everywhere, including in the language grammar
*/

#![warn(missing_docs)]

use core::cell::Cell;

/// A Callback that can be connected to a handler.
///
/// The Arg represents the argument. It should always be a tuple
///
#[repr(C)]
pub struct Callback<Arg: ?Sized, Ret = ()> {
    /// FIXME: Box<dyn> is a fat object and we probaly want to put an erased type in there
    handler: Cell<Option<Box<dyn Fn(&Arg) -> Ret>>>,
}

impl<Arg: ?Sized, Ret> Default for Callback<Arg, Ret> {
    fn default() -> Self {
        Self { handler: Default::default() }
    }
}

impl<Arg: ?Sized, Ret: Default> Callback<Arg, Ret> {
    /// Emit the callback with the given argument.
    pub fn emit(&self, a: &Arg) -> Ret {
        if let Some(h) = self.handler.take() {
            let r = h(a);
            assert!(self.handler.take().is_none(), "Callback Handler set while emitted");
            self.handler.set(Some(h));
            r
        } else {
            Default::default()
        }
    }

    /// Set an handler to be called when the callback is emited
    ///
    /// There can only be one single handler per callback.
    pub fn set_handler(&self, f: impl Fn(&Arg) -> Ret + 'static) {
        self.handler.set(Some(Box::new(f)));
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
    c.clicked.emit(&());
    assert_eq!(c.pressed.get(), true);
}

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
    static_assertions::assert_eq_align!(CallbackOpaque, Callback<(String,)>);
    static_assertions::assert_eq_size!(CallbackOpaque, Callback<(String,)>);

    /// Initialize the callback.
    /// sixtyfps_callback_drop must be called.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_callback_init(out: *mut CallbackOpaque) {
        assert_eq!(core::mem::size_of::<CallbackOpaque>(), core::mem::size_of::<Callback<()>>());
        core::ptr::write(out as *mut Callback<()>, Default::default());
    }

    /// Emit the callback
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_callback_emit(sig: *const CallbackOpaque, arg: *const c_void) {
        let sig = &*(sig as *const Callback<c_void>);
        sig.emit(&*arg);
    }

    /// Set callback handler.
    ///
    /// The binding has signature fn(user_data)
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_callback_set_handler(
        sig: *const CallbackOpaque,
        binding: extern "C" fn(user_data: *mut c_void, arg: *const c_void),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) {
        let sig = &mut *(sig as *mut Callback<c_void>);

        struct UserData {
            user_data: *mut c_void,
            drop_user_data: Option<extern "C" fn(*mut c_void)>,
        }

        impl Drop for UserData {
            fn drop(&mut self) {
                if let Some(x) = self.drop_user_data {
                    x(self.user_data)
                }
            }
        }
        let ud = UserData { user_data, drop_user_data };

        let real_binding = move |arg: &()| {
            binding(ud.user_data, arg as *const c_void);
        };
        sig.set_handler(real_binding);
    }

    /// Destroy callback
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_callback_drop(handle: *mut CallbackOpaque) {
        core::ptr::read(handle as *mut Callback<()>);
    }
}
