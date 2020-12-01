/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
Signal that can be connected to  one sigle handler.

TODO: reconsider if we should rename that to `Event`
but then it should also be renamed everywhere, including in the language grammar
*/

#![warn(missing_docs)]

use core::cell::Cell;

/// A Signal that can be connected to a handler.
///
/// The Arg represents the argument. It should always be a tuple
///
#[repr(C)]
pub struct Signal<Arg: ?Sized, Ret = ()> {
    /// FIXME: Box<dyn> is a fat object and we probaly want to put an erased type in there
    handler: Cell<Option<Box<dyn Fn(&Arg) -> Ret>>>,
}

impl<Arg: ?Sized, Ret> Default for Signal<Arg, Ret> {
    fn default() -> Self {
        Self { handler: Default::default() }
    }
}

impl<Arg: ?Sized, Ret: Default> Signal<Arg, Ret> {
    /// Emit the signal with the given argument.
    pub fn emit(&self, a: &Arg) -> Ret {
        if let Some(h) = self.handler.take() {
            let r = h(a);
            assert!(self.handler.take().is_none(), "Signal Handler set while emitted");
            self.handler.set(Some(h));
            r
        } else {
            Default::default()
        }
    }

    /// Set an handler to be called when the signal is emited
    ///
    /// There can only be one single handler per signal.
    pub fn set_handler(&self, f: impl Fn(&Arg) -> Ret + 'static) {
        self.handler.set(Some(Box::new(f)));
    }
}

#[test]
fn signal_simple_test() {
    use std::rc::Rc;
    #[derive(Default)]
    struct Component {
        pressed: core::cell::Cell<bool>,
        clicked: Signal<()>,
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
    /// Has the same layout as Signal<_>
    pub struct SignalOpaque(*const c_void, *const c_void);

    static_assertions::assert_eq_align!(SignalOpaque, Signal<()>);
    static_assertions::assert_eq_size!(SignalOpaque, Signal<()>);
    static_assertions::assert_eq_align!(SignalOpaque, Signal<(String,)>);
    static_assertions::assert_eq_size!(SignalOpaque, Signal<(String,)>);

    /// Initialize the signal.
    /// sixtyfps_signal_drop must be called.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_signal_init(out: *mut SignalOpaque) {
        assert_eq!(core::mem::size_of::<SignalOpaque>(), core::mem::size_of::<Signal<()>>());
        core::ptr::write(out as *mut Signal<()>, Default::default());
    }

    /// Emit the signal
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_signal_emit(sig: *const SignalOpaque, arg: *const c_void) {
        let sig = &*(sig as *const Signal<c_void>);
        sig.emit(&*arg);
    }

    /// Set signal handler.
    ///
    /// The binding has signature fn(user_data)
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_signal_set_handler(
        sig: *const SignalOpaque,
        binding: extern "C" fn(user_data: *mut c_void, arg: *const c_void),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) {
        let sig = &mut *(sig as *mut Signal<c_void>);

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

    /// Destroy signal
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_signal_drop(handle: *mut SignalOpaque) {
        core::ptr::read(handle as *mut Signal<()>);
    }
}
