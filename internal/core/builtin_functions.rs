// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The implementation of the `.slint` builtin functions whose result only depends on their arguments.
//!
//! See the `runtime` functions in `i_slint_common::for_each_builtin_function!`.

#![allow(unsafe_code)]
#![allow(missing_docs)]

use crate::api::LogicalPosition;
use crate::graphics::{Brush, Color, CornerShape, HsvaColor, OklchColor, RgbaColorU8};
use crate::input::Keys;
use crate::item_tree::ItemRc;
use crate::items::{
    ContextMenu, Flickable, FontMetrics, OperatingSystemType, ScrollBehavior, SwipeGestureHandler,
    TextInput, WindowItem,
};
use crate::lengths::logical_position_to_api;
use crate::string::ToSharedString;
use crate::styled_text::StyledText;
use crate::window::WindowAdapterRc;
use crate::{SharedString, SlintContext, date_time};
use core::pin::Pin;
#[cfg(not(feature = "std"))]
use num_traits::float::Float;
use num_traits::ops::euclid::Euclid;

/// Declares the functions of the builtins that have a body, and their C function for the C++ code:
/// a pointer to the result comes first, then pointers to the arguments.
/// The C function of a function that takes a context passes the thread's context.
macro_rules! builtin_functions {
    ($(
        $(#[$attr:meta])*
        $Name:ident $(($Payload:ident))? : $(#[$runtime_attr:meta])*
            $purity:ident fn $($name:ident)? ($($params:tt)*) -> $Ret:tt $(=> $RetType:tt)? $($body:block)?
    )*) => {
        $(builtin_function!($(#[$runtime_attr])* $($name)? ($($params)*) -> $Ret $($body)?);)*
    };
}

macro_rules! builtin_function {
    ($(#[$attr:meta])* ($($params:tt)*) -> $Ret:tt) => {};
    ($(#[$attr:meta])* $name:ident ($(& $context:ident $(,)?)? $($param:ident: $ty:tt $(<$gen:ident>)? $(=> $as:tt)?),*) -> $Ret:tt $body:block) => {
        $(#[$attr])*
        #[inline]
        pub fn $name(
            $($context: &SlintContext,)?
            $($param: $ty $(<'_, $gen>)?),*
        ) -> $Ret $body

        $(#[$attr])*
        #[cfg(feature = "ffi")]
        const _: () = {
            #[unsafe(export_name = concat!("slint_builtin_", stringify!($name)))]
            extern "C" fn ffi(out: &mut $Ret, $($param: &ffi_param_type!(($ty $(<$gen>)?) $($as)?)),*) {
                $(let Some($context) = SlintContext::current() else { return };)?
                *out = $name($(&$context,)? $(ffi_argument!($param, ($ty $(<$gen>)?) $($as)?)),*);
            }
        };
    };
}

#[cfg(feature = "ffi")]
macro_rules! ffi_param_type {
    (($($ty:tt)*) ElementReference) => { BuiltinItemArg<'_> };
    (($($ty:tt)*) $($as:tt)?) => { $($ty)* };
}

#[cfg(feature = "ffi")]
macro_rules! ffi_argument {
    ($param:ident, (ItemRc) ElementReference) => {
        ItemRc::new($param.item_tree.clone(), $param.index)
    };
    ($param:ident, (ItemArg<$item:ident>) ElementReference) => {
        ItemArg {
            // SAFETY: the C++ code passes an item of the type of the builtin's parameter
            item: unsafe { Pin::new_unchecked(&*($param.item as *const $item)) },
            rc: ItemRc::new($param.item_tree.clone(), $param.index),
        }
    };
    ($param:ident, ($($ty:tt)*) $($as:tt)?) => {
        $param.clone()
    };
}

/// An element passed to a builtin: its native item and the item's `ItemRc`.
/// The generated code passes one to the builtins that only take the `ItemRc` too.
pub struct ItemArg<'a, T> {
    pub item: Pin<&'a T>,
    pub rc: ItemRc,
}

impl<T> From<ItemArg<'_, T>> for ItemRc {
    fn from(arg: ItemArg<'_, T>) -> Self {
        arg.rc
    }
}

/// Calls `f` with the window adapter of the item's window, created if it doesn't exist yet.
fn with_window_adapter<R: Default>(item: &ItemRc, f: impl FnOnce(&WindowAdapterRc) -> R) -> R {
    let mut window_adapter = None;
    vtable::VRc::borrow_pin(item.item_tree()).as_ref().window_adapter(true, &mut window_adapter);
    window_adapter.map(|window_adapter| f(&window_adapter)).unwrap_or_default()
}

/// An element passed to a builtin by the C++ code, see `ItemArg`.
#[cfg(feature = "ffi")]
#[repr(C)]
pub struct BuiltinItemArg<'a> {
    item: *const core::ffi::c_void,
    item_tree: &'a vtable::VRc<crate::item_tree::ItemTreeVTable>,
    index: u32,
}

#[cfg(feature = "path")]
use crate::items::Path;

i_slint_common::for_each_builtin_function!(builtin_functions);
