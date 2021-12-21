// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

//! module for the SharedString and related things

#![allow(unsafe_code)]
#![warn(missing_docs)]

use crate::SharedVector;
use alloc::string::String;
use core::fmt::{Debug, Display};
use core::iter::FromIterator;
use core::ops::Deref;

/// A string type used by the SixtyFPS run-time.
///
/// SharedString uses implicit data sharing to make it efficient to pass around copies. When
/// cloning, a reference to the data is cloned, not the data itself. The data itself is only copied
/// when modifying it, for example using [push_str](SharedString::push_str). This is also called copy-on-write.
///
/// Under the hood the string data is UTF-8 encoded and it is always terminated with a null character.
///
/// `SharedString` implements [`Deref<Target=str>`] so it can be easily passed to any function taking a `&str`.
/// It also implement `From` such that it an easily be converted to and from the typical rust String type with `.into()`
#[derive(Clone, Default)]
#[repr(C)]
pub struct SharedString {
    // Invariant: valid utf-8, `\0` terminated
    inner: SharedVector<u8>,
}

impl SharedString {
    fn as_ptr(&self) -> *const u8 {
        self.inner.as_ptr()
    }

    /// Size of the string, in bytes. This excludes the terminating null character.
    pub fn len(&self) -> usize {
        self.inner.len().saturating_sub(1)
    }

    /// Return true if the String is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return a slice to the string
    pub fn as_str(&self) -> &str {
        // Safety: self.as_ptr is a pointer from the inner which has utf-8
        unsafe {
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(self.as_ptr(), self.len()))
        }
    }

    /// Append a string to this string
    ///
    /// ```
    /// # use sixtyfps_corelib::SharedString;
    /// let mut hello = SharedString::from("Hello");
    /// hello.push_str(", ");
    /// hello.push_str("World");
    /// hello.push_str("!");
    /// assert_eq!(hello, "Hello, World!");
    /// ```
    pub fn push_str(&mut self, x: &str) {
        let mut iter = x.as_bytes().iter().copied();
        if self.inner.is_empty() {
            self.inner.extend(iter.chain(core::iter::once(0)));
            return;
        } else if let Some(first) = iter.next() {
            // We skip the `first` from `iter` because we will write it at the
            // location of the previous `\0`, after extend did the re-alloc of the
            // right size
            let prev_len = self.len();
            self.inner.extend(iter.chain(core::iter::once(0)));
            self.inner.make_mut_slice()[prev_len] = first;
        }
    }
}

impl Deref for SharedString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl From<&str> for SharedString {
    fn from(value: &str) -> Self {
        SharedString {
            inner: SharedVector::from_iter(
                value.as_bytes().iter().cloned().chain(core::iter::once(0)),
            ),
        }
    }
}

impl Debug for SharedString {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}

impl Display for SharedString {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(self.as_str(), f)
    }
}

impl AsRef<str> for SharedString {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(feature = "std")]
impl AsRef<std::ffi::CStr> for SharedString {
    #[inline]
    fn as_ref(&self) -> &std::ffi::CStr {
        if self.inner.is_empty() {
            return Default::default();
        }
        // Safety: we ensure that there is always a terminated \0
        debug_assert_eq!(self.inner.as_slice()[self.inner.len() - 1], 0);
        unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(self.inner.as_slice()) }
    }
}

impl AsRef<[u8]> for SharedString {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_str().as_bytes()
    }
}

impl<T> PartialEq<T> for SharedString
where
    T: ?Sized + AsRef<str>,
{
    fn eq(&self, other: &T) -> bool {
        self.as_str() == other.as_ref()
    }
}
impl Eq for SharedString {}

impl<T> PartialOrd<T> for SharedString
where
    T: ?Sized + AsRef<str>,
{
    fn partial_cmp(&self, other: &T) -> Option<core::cmp::Ordering> {
        PartialOrd::partial_cmp(self.as_str(), other.as_ref())
    }
}
impl Ord for SharedString {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        Ord::cmp(self.as_str(), other.as_str())
    }
}

impl From<String> for SharedString {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

impl From<&String> for SharedString {
    fn from(s: &String) -> Self {
        s.as_str().into()
    }
}

impl From<SharedString> for String {
    fn from(s: SharedString) -> String {
        s.as_str().into()
    }
}

impl From<&SharedString> for String {
    fn from(s: &SharedString) -> String {
        s.as_str().into()
    }
}

impl core::ops::AddAssign<&str> for SharedString {
    fn add_assign(&mut self, other: &str) {
        self.push_str(other);
    }
}

impl core::ops::Add<&str> for SharedString {
    type Output = SharedString;
    fn add(mut self, other: &str) -> SharedString {
        self.push_str(other);
        self
    }
}

impl core::hash::Hash for SharedString {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

#[test]
fn simple_test() {
    let x = SharedString::from("hello world!");
    assert_eq!(x, "hello world!");
    assert_ne!(x, "hello world?");
    assert_eq!(x, x.clone());
    assert_eq!("hello world!", x.as_str());
    let string = String::from("hello world!");
    assert_eq!(x, string);
    assert_eq!(x.to_string(), string);
    let def = SharedString::default();
    assert_eq!(def, SharedString::default());
    assert_ne!(def, x);
    assert_eq!(
        (&x as &dyn AsRef<std::ffi::CStr>).as_ref(),
        &*std::ffi::CString::new("hello world!").unwrap()
    );
}

#[test]
fn threading() {
    let shared_cst = SharedString::from("Hello there!");
    let shared_mtx = std::sync::Arc::new(std::sync::Mutex::new(SharedString::from("Shared:")));
    let mut handles = vec![];
    for _ in 0..20 {
        let cst = shared_cst.clone();
        let mtx = shared_mtx.clone();
        handles.push(std::thread::spawn(move || {
            assert_eq!(cst, "Hello there!");
            let mut cst2 = cst.clone();
            cst2.push_str(" ... or not?");
            assert_eq!(cst2, "Hello there! ... or not?");
            assert_eq!(cst.clone(), "Hello there!");

            let shared = {
                let mut lock = mtx.lock().unwrap();
                assert!(lock.starts_with("Shared:"));
                lock.push_str("!");
                lock.clone()
            };
            assert!(shared.clone().starts_with("Shared:"));
        }));
    }
    for j in handles {
        j.join().unwrap();
    }
    assert_eq!(shared_cst.clone(), "Hello there!");
    assert_eq!(shared_mtx.lock().unwrap().as_str(), "Shared:!!!!!!!!!!!!!!!!!!!!");
    // 20x"!"
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    use super::*;

    /// for cbindgen.
    #[allow(non_camel_case_types)]
    type c_char = u8;

    #[no_mangle]
    /// Returns a nul-terminated pointer for this string.
    /// The returned value is owned by the string, and should not be used after any
    /// mutable function have been called on the string, and must not be freed.
    pub extern "C" fn sixtyfps_shared_string_bytes(ss: &SharedString) -> *const c_char {
        if ss.is_empty() {
            "\0".as_ptr()
        } else {
            ss.as_ptr()
        }
    }

    #[no_mangle]
    /// Destroy the shared string
    pub unsafe extern "C" fn sixtyfps_shared_string_drop(ss: *const SharedString) {
        core::ptr::read(ss);
    }

    #[no_mangle]
    /// Increment the reference count of the string.
    /// The resulting structure must be passed to sixtyfps_shared_string_drop
    pub unsafe extern "C" fn sixtyfps_shared_string_clone(
        out: *mut SharedString,
        ss: &SharedString,
    ) {
        core::ptr::write(out, ss.clone())
    }

    #[no_mangle]
    /// Safety: bytes must be a valid utf-8 string of size len without null inside.
    /// The resulting structure must be passed to sixtyfps_shared_string_drop
    pub unsafe extern "C" fn sixtyfps_shared_string_from_bytes(
        out: *mut SharedString,
        bytes: *const c_char,
        len: usize,
    ) {
        let str = core::str::from_utf8(core::slice::from_raw_parts(bytes, len)).unwrap();
        core::ptr::write(out, SharedString::from(str));
    }

    /// Create a string from a number.
    /// The resulting structure must be passed to sixtyfps_shared_string_drop
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_shared_string_from_number(out: *mut SharedString, n: f64) {
        // TODO: implement Write for SharedString so this can be done without allocation
        let str = format!("{}", n);
        core::ptr::write(out, SharedString::from(str.as_str()));
    }

    #[test]
    fn test_sixtyfps_shared_string_from_number() {
        unsafe {
            let mut s = core::mem::MaybeUninit::uninit();
            sixtyfps_shared_string_from_number(s.as_mut_ptr(), 45.);
            assert_eq!(s.assume_init(), "45");

            let mut s = core::mem::MaybeUninit::uninit();
            sixtyfps_shared_string_from_number(s.as_mut_ptr(), 45.12);
            assert_eq!(s.assume_init(), "45.12");

            let mut s = core::mem::MaybeUninit::uninit();
            sixtyfps_shared_string_from_number(s.as_mut_ptr(), -1325466.);
            assert_eq!(s.assume_init(), "-1325466");

            let mut s = core::mem::MaybeUninit::uninit();
            sixtyfps_shared_string_from_number(s.as_mut_ptr(), 0.);
            assert_eq!(s.assume_init(), "0");
        }
    }

    /// Append some bytes to an existing shared string
    ///
    /// bytes must be a valid utf8 array of size `len`, without null bytes inside
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_shared_string_append(
        self_: &mut SharedString,
        bytes: *const c_char,
        len: usize,
    ) {
        let str = core::str::from_utf8(core::slice::from_raw_parts(bytes, len)).unwrap();
        self_.push_str(str);
    }
    #[test]
    fn test_sixtyfps_shared_string_append() {
        let mut s = SharedString::default();
        let mut append = |x: &str| unsafe {
            sixtyfps_shared_string_append(&mut s, x.as_bytes().as_ptr(), x.len());
        };
        append("Hello");
        append(", ");
        append("world");
        append("");
        append("!");
        assert_eq!(s.as_str(), "Hello, world!");
    }
}
