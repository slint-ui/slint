use core::mem::MaybeUninit;
use servo_arc::ThinArc;
use std::{fmt::Debug, ops::Deref};

/// The string type suitable for properties. It is shared meaning passing copies
/// around will not allocate, and that different properties with the same string
/// can share the same buffer.
/// It is also ffi-friendly as the buffer always ends with `'\0'`
/// Internally, this is an implicitly shared type to a null terminated string
#[derive(Clone)]
pub struct SharedString {
    /// Invariant: The usize header is the `len` of the vector, the contained buffer is [MaybeUninit<u8>]
    /// buffer[0..=len] is initialized and valid utf8, and buffer[len] is '\0'
    inner: ThinArc<usize, MaybeUninit<u8>>,
}

impl SharedString {
    fn as_ptr(&self) -> *const u8 {
        self.inner.slice.as_ptr() as *const u8
    }

    pub fn len(&self) -> usize {
        self.inner.header.header
    }

    pub fn as_str(&self) -> &str {
        unsafe {
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(self.as_ptr(), self.len()))
        }
    }
}

impl Deref for SharedString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Default for SharedString {
    fn default() -> Self {
        // Unfortunately, the Arc constructor is not const, so we must use a Lazy static for that
        static NULL: once_cell::sync::Lazy<ThinArc<usize, MaybeUninit<u8>>> =
            once_cell::sync::Lazy::new(|| {
                servo_arc::Arc::into_thin(servo_arc::Arc::from_header_and_iter(
                    servo_arc::HeaderWithLength::new(0, core::mem::align_of::<usize>()),
                    [MaybeUninit::new(0); core::mem::align_of::<usize>()].iter().cloned(),
                ))
            });

        SharedString { inner: NULL.clone() }
    }
}

impl From<&str> for SharedString {
    fn from(value: &str) -> Self {
        struct AddNullIter<'a> {
            pos: usize,
            str: &'a [u8],
        }

        impl<'a> Iterator for AddNullIter<'a> {
            type Item = MaybeUninit<u8>;
            fn next(&mut self) -> Option<MaybeUninit<u8>> {
                let pos = self.pos;
                self.pos += 1;
                let align = core::mem::align_of::<usize>();
                if pos < self.str.len() {
                    Some(MaybeUninit::new(self.str[pos]))
                } else if pos < (self.str.len() + align) & !(align - 1) {
                    Some(MaybeUninit::new(0))
                } else {
                    None
                }
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                let l = self.str.len() + 1;
                // add some padding at the end since the sice of the inner will anyway have to be padded
                let align = core::mem::align_of::<usize>();
                let l = (l + align - 1) & !(align - 1);
                let l = l - self.pos;
                (l, Some(l))
            }
        }
        impl<'a> core::iter::ExactSizeIterator for AddNullIter<'a> {}

        let iter = AddNullIter { str: value.as_bytes(), pos: 0 };

        SharedString {
            inner: servo_arc::Arc::into_thin(servo_arc::Arc::from_header_and_iter(
                servo_arc::HeaderWithLength::new(value.len(), iter.size_hint().0),
                iter,
            )),
        }
    }
}

impl Debug for SharedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}

/// for cbingen.
#[allow(non_camel_case_types)]
type c_char = u8;

#[no_mangle]
pub extern "C" fn sixtyfps_shared_string_bytes(ss: &SharedString) -> *const c_char {
    ss.as_ptr()
}

#[no_mangle]
/// Destroy the shared string
pub unsafe extern "C" fn sixtyfps_shared_string_drop(ss: *const SharedString) {
    core::ptr::read(ss);
}

#[no_mangle]
/// Increment the reference count of the string.
/// the resulting structure must be passed to sixtyfps_shared_string_drop
pub unsafe extern "C" fn sixtyfps_shared_string_clone(out: *mut SharedString, ss: &SharedString) {
    core::ptr::write(out, ss.clone())
}

#[no_mangle]
/// Safety: bytes must be a valid utf-8 string of size len wihout null inside.
/// the resulting structure must be passed to sixtyfps_shared_string_drop
pub unsafe extern "C" fn sixtyfps_shared_string_from_bytes(
    out: *mut SharedString,
    bytes: *const c_char,
    len: usize,
) {
    let str = core::str::from_utf8_unchecked(core::slice::from_raw_parts(bytes, len));
    core::ptr::write(out, SharedString::from(str));
}
