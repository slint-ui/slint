// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::SharedString;

/// Location information attached to a debug message.
#[derive(Clone, Debug)]
pub struct DebugLogLocation {
    pub path: SharedString,
    pub line: usize,
    pub column: usize,
}

/// Debug handler type stored in the global context.
pub type DebugLogHandler = alloc::boxed::Box<
    dyn for<'a> Fn(Option<&DebugLogLocation>, core::fmt::Arguments<'a>) + 'static,
>;

/// implementation details for debug_log()
#[doc(hidden)]
pub fn debug_log_impl(args: core::fmt::Arguments) {
    crate::context::GLOBAL_CONTEXT.with(|p| match p.get() {
        Some(ctx) => ctx.dispatch_debug_log(None, args),
        None => default_debug_log(args),
    });
}

#[doc(hidden)]
pub fn debug_log_with_location(location: Option<&DebugLogLocation>, args: core::fmt::Arguments) {
    crate::context::GLOBAL_CONTEXT.with(|p| match p.get() {
        Some(ctx) => ctx.dispatch_debug_log(location, args),
        None => default_debug_log(args),
    });
}

#[doc(hidden)]
pub fn default_debug_log(_arguments: core::fmt::Arguments) {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            use wasm_bindgen::prelude::*;
            use std::string::ToString;

            #[wasm_bindgen]
            extern "C" {
                #[wasm_bindgen(js_namespace = console)]
                pub fn log(s: &str);
            }

            log(&_arguments.to_string());
        } else if #[cfg(feature = "std")] {
            use std::io::Write;
            // We were seeing intermittent, albeit very rare, crashes due to `eprintln` panicking
            // if the write to stderr fails. Since this is just for debug printing, it's safe
            // to silently drop this if we can't write (since it wouldn't be written to stderr
            // anyway)
            let _ = writeln!(std::io::stderr(), "{_arguments}");
        }
    }
}

#[macro_export]
/// This macro allows producing debug output that will appear on stderr in regular builds
/// and in the console log for wasm builds.
macro_rules! debug_log {
    ($($t:tt)*) => ($crate::debug_log::debug_log_impl(format_args!($($t)*)))
}
