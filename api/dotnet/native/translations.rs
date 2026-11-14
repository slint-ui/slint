// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::borrow::Cow;
use std::ffi::c_void;

use crate::{OwnedString, Utf8Str, guard_bool};

/// Switches to the bundled translations of `language`; an empty string or `en` selects the original strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_select_bundled_translation(language: Utf8Str) -> bool {
    guard_bool(|| {
        i_slint_core::translations::select_bundled_translation(unsafe { language.as_str() }?)
            .map_err(|e| e.to_string())
    })
}

/// Copies `s` into a string that Rust owns, for a .NET callback to return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_string_new(s: Utf8Str) -> OwnedString {
    unsafe { s.as_str() }.map_or_else(|_| OwnedString::none(), |s| s.to_string().into())
}

/// Translates `text`, or picks the form of `text` and `plural` for `n` if `has_plural`.
/// Returns no string to keep the original.
pub type TranslateFn = unsafe extern "C" fn(
    user_data: *mut c_void,
    text: Utf8Str,
    plural: Utf8Str,
    n: u64,
    has_plural: bool,
    context: Utf8Str,
    has_context: bool,
) -> OwnedString;

/// A .NET `ITranslator`; dropping it releases `user_data` through `drop_user_data`.
struct ManagedTranslator {
    translate: TranslateFn,
    user_data: *mut c_void,
    drop_user_data: unsafe extern "C" fn(*mut c_void),
}

// Safety: the .NET side takes a GCHandle, which any thread may use; translators must be thread safe.
unsafe impl Send for ManagedTranslator {}
unsafe impl Sync for ManagedTranslator {}

impl ManagedTranslator {
    fn call<'a>(
        &self,
        text: &'a str,
        plural: Option<&'a str>,
        n: u64,
        context: Option<&'a str>,
    ) -> Cow<'a, str> {
        let result = unsafe {
            (self.translate)(
                self.user_data,
                Utf8Str::borrow(text),
                Utf8Str::borrow(plural.unwrap_or("")),
                n,
                plural.is_some(),
                Utf8Str::borrow(context.unwrap_or("")),
                context.is_some(),
            )
        };
        match result.into_string() {
            Some(translated) => Cow::Owned(translated),
            None if n == 1 || plural.is_none() => Cow::Borrowed(text),
            None => Cow::Borrowed(plural.unwrap_or(text)),
        }
    }
}

impl Drop for ManagedTranslator {
    fn drop(&mut self) {
        unsafe { (self.drop_user_data)(self.user_data) }
    }
}

impl i_slint_core::translations::Translator for ManagedTranslator {
    fn translate<'a>(&'a self, string: &'a str, context: Option<&'a str>) -> Cow<'a, str> {
        self.call(string, None, 1, context)
    }

    fn ntranslate<'a>(
        &'a self,
        n: u64,
        singular: &'a str,
        plural: &'a str,
        context: Option<&'a str>,
    ) -> Cow<'a, str> {
        self.call(singular, Some(plural), n, context)
    }
}

/// Translates `@tr()` strings with `translate`, or with the bundled translations again if it's null.
/// Takes ownership of `user_data`, also on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_set_translator(
    translate: Option<TranslateFn>,
    user_data: *mut c_void,
    drop_user_data: unsafe extern "C" fn(*mut c_void),
) -> bool {
    let translator = translate.map(|translate| {
        Box::new(ManagedTranslator { translate, user_data, drop_user_data })
            as Box<dyn i_slint_core::translations::Translator>
    });
    guard_bool(|| {
        i_slint_backend_selector::with_global_context(|ctx| ctx.set_external_translator(translator))
            .map_err(|e| e.to_string())
    })
}
