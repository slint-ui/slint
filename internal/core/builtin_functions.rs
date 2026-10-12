// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The implementation of the `.slint` builtin functions whose result only depends on their arguments.
//!
//! See the `runtime` functions in `i_slint_common::for_each_builtin_function!`.

#![allow(unsafe_code)]
#![allow(missing_docs)]

use crate::graphics::{Brush, Color, HsvaColor, OklchColor, RgbaColorU8};
use crate::input::Keys;
use crate::items::OperatingSystemType;
use crate::string::ToSharedString;
use crate::styled_text::StyledText;
use crate::{SharedString, SlintContext, date_time};
#[cfg(not(feature = "std"))]
use num_traits::float::Float;
use num_traits::ops::euclid::Euclid;

/// Declares the functions of the builtins that have a body, and their C function for the C++ code:
/// a pointer to the result comes first, then pointers to the arguments.
/// The C function of a function that takes a context passes the thread's context.
macro_rules! builtin_functions {
    ($(
        $(#[$attr:meta])*
        $Name:ident $(($Payload:ident))? :
            $purity:ident fn $($name:ident)? ($($params:tt)*) -> $Ret:tt $(=> $RetType:tt)? $($body:block)?
    )*) => {
        $(builtin_function!($($name)? ($($params)*) -> $Ret $($body)?);)*
    };
}

macro_rules! builtin_function {
    (($($params:tt)*) -> $Ret:tt) => {};
    ($name:ident ($(& $context:ident $(,)?)? $($param:ident: $ty:tt $(=> $as:tt)?),*) -> $Ret:tt $body:block) => {
        #[inline]
        pub fn $name($($context: &SlintContext,)? $($param: $ty),*) -> $Ret $body

        #[cfg(feature = "ffi")]
        const _: () = {
            #[unsafe(export_name = concat!("slint_builtin_", stringify!($name)))]
            extern "C" fn ffi(out: &mut $Ret, $($param: &$ty),*) {
                $(let Some($context) = SlintContext::current() else { return };)?
                *out = $name($(&$context,)? $($param.clone()),*);
            }
        };
    };
}

i_slint_common::for_each_builtin_function!(builtin_functions);
