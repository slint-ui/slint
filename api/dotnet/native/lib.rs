// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! C ABI for the Slint .NET bindings.
//! See `docs/development/dotnet-bindings.md` for the conventions every export follows.

#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;
use std::ffi::c_void;
use std::panic::AssertUnwindSafe;
use std::sync::OnceLock;
use std::thread::ThreadId;

mod brush;
mod instance;
mod interpreter;
mod models;
mod timer;
mod translations;
mod value;
mod window;

/// A borrowed UTF-8 string.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Utf8Str {
    ptr: *const u8,
    len: usize,
}

impl Utf8Str {
    unsafe fn as_str<'a>(self) -> Result<&'a str, String> {
        if self.len == 0 {
            return Ok("");
        }
        let bytes = unsafe { std::slice::from_raw_parts(self.ptr, self.len) };
        std::str::from_utf8(bytes).map_err(|e| e.to_string())
    }

    fn borrow(s: &str) -> Self {
        Self { ptr: s.as_ptr(), len: s.len() }
    }
}

unsafe fn utf8_slice<'a>(strings: *const Utf8Str, count: usize) -> Result<Vec<&'a str>, String> {
    if count == 0 {
        return Ok(Vec::new());
    }
    unsafe { std::slice::from_raw_parts(strings, count) }
        .iter()
        .map(|s| unsafe { s.as_str() })
        .collect()
}

/// A UTF-8 string allocated by Rust, released with `slint_dotnet_string_free`.
/// A null `ptr` stands for no string.
#[repr(C)]
pub struct OwnedString {
    ptr: *mut u8,
    len: usize,
    cap: usize,
}

impl OwnedString {
    fn none() -> Self {
        Self { ptr: std::ptr::null_mut(), len: 0, cap: 0 }
    }

    /// Takes the string back, or returns `None` for no string.
    fn into_string(self) -> Option<String> {
        (!self.ptr.is_null())
            .then(|| unsafe { String::from_raw_parts(self.ptr, self.len, self.cap) })
    }
}

impl From<String> for OwnedString {
    fn from(s: String) -> Self {
        let mut s = std::mem::ManuallyDrop::new(s);
        Self { ptr: s.as_mut_ptr(), len: s.len(), cap: s.capacity() }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_string_free(s: OwnedString) {
    if !s.ptr.is_null() {
        drop(unsafe { String::from_raw_parts(s.ptr, s.len, s.cap) });
    }
}

/// Names with an integer tag each, such as property names with their `ValueType`.
pub struct NameList(Vec<(String, i32)>);

impl NameList {
    fn untagged(names: impl Iterator<Item = String>) -> Box<Self> {
        Box::new(Self(names.map(|n| (n, 0)).collect()))
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_name_list_len(list: &NameList) -> usize {
    list.0.len()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_name_list_get(
    list: &NameList,
    index: usize,
    tag: *mut i32,
) -> Utf8Str {
    let (name, t) = &list.0[index];
    if !tag.is_null() {
        unsafe { *tag = *t };
    }
    Utf8Str::borrow(name)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_name_list_free(list: *mut NameList) {
    if !list.is_null() {
        drop(unsafe { Box::from_raw(list) });
    }
}

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

static SLINT_THREAD: OnceLock<ThreadId> = OnceLock::new();

fn is_slint_thread() -> bool {
    SLINT_THREAD.get() == Some(&std::thread::current().id())
}

/// Claims the Slint thread on first use and creates the platform there,
/// so that `invoke_from_event_loop` works from other threads.
fn check_thread() -> Result<(), String> {
    let current = std::thread::current().id();
    let mut claimed = false;
    if *SLINT_THREAD.get_or_init(|| {
        claimed = true;
        current
    }) != current
    {
        return Err("Slint objects can only be used from the thread that first used Slint".into());
    }
    if claimed {
        // Compiling works without a platform, so a missing backend only fails the calls that need one.
        let _ = i_slint_backend_selector::with_platform(|_| Ok(()));
        #[cfg(feature = "testing")]
        if std::env::var("SLINT_BACKEND").is_ok_and(|b| b == "testing") {
            i_slint_backend_testing::configure_test_fonts();
            i_slint_core::OPERATING_SYSTEM_OVERRIDE
                .set(Some(i_slint_core::items::OperatingSystemType::Windows));
        }
    }
    Ok(())
}

/// Claims the current thread if no thread is the Slint thread yet,
/// for the exports that are callable from any thread.
fn claim_if_unclaimed() {
    if SLINT_THREAD.get().is_none() {
        let _ = check_thread();
    }
}

/// Runs `f` on the Slint thread, turning an error or a panic into the thread's last error and `on_error`.
fn guard<T>(on_error: T, f: impl FnOnce() -> Result<T, String>) -> T {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        check_thread()?;
        f()
    }));
    let message = match result {
        Ok(Ok(value)) => return value,
        Ok(Err(message)) => message,
        Err(panic) => panic
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic in Slint".into()),
    };
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(message));
    on_error
}

fn guard_bool(f: impl FnOnce() -> Result<(), String>) -> bool {
    guard(false, || f().map(|()| true))
}

fn into_raw<T>(value: T) -> *mut T {
    Box::into_raw(Box::new(value))
}

struct SendPtr<T>(*mut T);
// Safety: the pointer is only dereferenced on the Slint thread, see `release`.
unsafe impl<T> Send for SendPtr<T> {}

impl<T> SendPtr<T> {
    unsafe fn into_box(self) -> Box<T> {
        unsafe { Box::from_raw(self.0) }
    }
}

/// Drops a boxed object on the Slint thread.
/// The .NET finalizer thread may release handles, but Slint objects hold `Rc`s.
/// If the event loop never runs again, the object leaks.
unsafe fn release<T: 'static>(ptr: *mut T) {
    if ptr.is_null() {
        return;
    }
    if is_slint_thread() {
        drop(unsafe { Box::from_raw(ptr) });
    } else {
        let ptr = SendPtr(ptr);
        let _ = slint_interpreter::invoke_from_event_loop(move || drop(unsafe { ptr.into_box() }));
    }
}

/// Takes the message of the last failed call on this thread, or no string.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_last_error() -> OwnedString {
    LAST_ERROR.with(|e| e.borrow_mut().take()).map_or_else(OwnedString::none, OwnedString::from)
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_run_event_loop() -> bool {
    guard_bool(|| slint_interpreter::run_event_loop().map_err(|e| e.to_string()))
}

/// Callable from any thread.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_quit_event_loop() -> bool {
    claim_if_unclaimed();
    match slint_interpreter::quit_event_loop() {
        Ok(()) => true,
        Err(e) => {
            LAST_ERROR.with(|l| *l.borrow_mut() = Some(e.to_string()));
            false
        }
    }
}

/// Callable from any thread. `callback` runs once on the Slint thread with `user_data`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_invoke_from_event_loop(
    callback: unsafe extern "C" fn(*mut c_void),
    user_data: *mut c_void,
) -> bool {
    claim_if_unclaimed();
    let user_data = SendPtr(user_data);
    match slint_interpreter::invoke_from_event_loop(move || {
        let user_data = user_data;
        unsafe { callback(user_data.0) }
    }) {
        Ok(()) => true,
        Err(e) => {
            LAST_ERROR.with(|l| *l.borrow_mut() = Some(e.to_string()));
            false
        }
    }
}

/// The optional capabilities compiled into this library, separated by commas.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_build_features() -> OwnedString {
    let mut features = Vec::new();
    if cfg!(feature = "backend-testing") {
        features.push("backend-testing");
    }
    if cfg!(feature = "system-testing") {
        features.push("system-testing");
    }
    if cfg!(feature = "mcp") {
        features.push("mcp");
    }
    if cfg!(feature = "live-preview") {
        features.push("live-preview");
    }
    features.join(",").into()
}
