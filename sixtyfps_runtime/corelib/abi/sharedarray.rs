//! module for the SharedArray and related things
use core::mem::MaybeUninit;
use servo_arc::ThinArc;
use std::{fmt::Debug, fmt::Display, ops::Deref};

#[derive(Clone)]
#[repr(C)]
/// SharedArray holds a reference-counted read-only copy of [T].
pub struct SharedArray<T: 'static> {
    /// Invariant: The usize header is the `len` of the vector, the contained buffer is [T]
    inner: ThinArc<usize, MaybeUninit<T>>,
}

struct PaddingFillingIter<'a, U> {
    iter: &'a mut dyn Iterator<Item = MaybeUninit<U>>,
    pos: usize,
    len: usize,
}

impl<'a, U> PaddingFillingIter<'a, U> {
    fn new(len: usize, iter: &'a mut dyn Iterator<Item = MaybeUninit<U>>) -> Self {
        Self { iter, pos: 0, len }
    }

    fn padded_length(&self) -> usize {
        // add some padding at the end since the size of the inner will anyway have to be padded
        let align = core::mem::align_of::<usize>() / core::mem::size_of::<U>();
        if self.len > 0 {
            if align > 0 {
                (self.len + align - 1) & !(align - 1)
            } else {
                self.len
            }
        } else {
            align
        }
    }
}

impl<'a, U: Clone> Iterator for PaddingFillingIter<'a, U> {
    type Item = MaybeUninit<U>;
    fn next(&mut self) -> Option<MaybeUninit<U>> {
        let pos = self.pos;
        self.pos += 1;
        if pos < self.len {
            self.iter.next()
        } else if pos < self.padded_length() {
            Some(MaybeUninit::uninit())
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let l = self.padded_length() - self.pos;
        (l, Some(l))
    }
}
impl<'a, U: Clone> core::iter::ExactSizeIterator for PaddingFillingIter<'a, U> {}

impl<T: Clone> SharedArray<T> {
    fn as_ptr(&self) -> *const T {
        self.inner.slice.as_ptr() as *const T
    }

    /// Size of the string, in bytes
    pub fn len(&self) -> usize {
        self.inner.header.header
    }

    /// Return a slice to the array
    pub fn as_slice(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.as_ptr(), self.len()) }
    }

    /// Constructs a new SharedArray from the given iterator.
    pub fn from_iter(iter: impl Iterator<Item = T> + ExactSizeIterator) -> Self {
        let len = iter.len();
        let item_iter = &mut iter.map(|item| MaybeUninit::new(item));
        let iter = PaddingFillingIter::new(len, item_iter);

        SharedArray {
            inner: servo_arc::Arc::into_thin(servo_arc::Arc::from_header_and_iter(
                servo_arc::HeaderWithLength::new(len, iter.padded_length()),
                iter,
            )),
        }
    }

    /// Constructs a new SharedArray from the given slice.
    pub fn from(slice: &[T]) -> Self {
        SharedArray::from_iter(slice.iter().cloned())
    }
}

impl<T: Clone> Deref for SharedArray<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}
trait StaticNull: Sized + 'static {
    const NULL: once_cell::sync::Lazy<ThinArc<usize, MaybeUninit<Self>>>;
}
impl<T: Clone + Copy + Default + Sized + 'static> StaticNull for T {
    const NULL: once_cell::sync::Lazy<ThinArc<usize, MaybeUninit<T>>> =
        once_cell::sync::Lazy::new(|| {
            let len = 0;
            let null_iter = &mut std::iter::empty();
            let iter = PaddingFillingIter::new(len, null_iter);

            servo_arc::Arc::into_thin(servo_arc::Arc::from_header_and_iter(
                servo_arc::HeaderWithLength::new(len, iter.padded_length()),
                iter,
            ))
        });
}

impl<T: Clone + Copy + Default> Default for SharedArray<T> {
    fn default() -> Self {
        SharedArray { inner: StaticNull::NULL.clone() }
    }
}

impl<T: Clone + Debug> Debug for SharedArray<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl<T: Clone + Debug> Display for SharedArray<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl<T: Clone> AsRef<[T]> for SharedArray<T> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, U> PartialEq<U> for SharedArray<T>
where
    U: ?Sized + AsRef<[T]>,
    T: Clone + PartialEq,
{
    fn eq(&self, other: &U) -> bool {
        self.as_slice() == other.as_ref()
    }
}

impl<T: Clone + PartialEq> Eq for SharedArray<T> {}

#[test]
fn simple_test() {
    let x: SharedArray<i32> = SharedArray::from(&[1, 2, 3]);
    let y: SharedArray<i32> = SharedArray::from(&[3, 2, 1]);
    assert_eq!(x, x.clone());
    assert_ne!(x, y);
    let z: [i32; 3] = [1, 2, 3];
    assert_eq!(z, x.as_slice());
    let vec: Vec<i32> = vec![1, 2, 3];
    assert_eq!(x, vec);
    let def: SharedArray<i32> = Default::default();
    assert_eq!(def, SharedArray::<i32>::default());
    assert_ne!(def, x);
}

#[no_mangle]
/// This function is used for the low-level C++ interface to allocate the backing vector for an empty shared array.
pub unsafe extern "C" fn sixtyfps_shared_array_new_null(out: *mut SharedArray<u8>) {
    core::ptr::write(out, SharedArray::<u8>::default());
}

#[no_mangle]
/// This function is used for the low-level C++ interface to clone a shared array by increasing its reference count.
pub unsafe extern "C" fn sixtyfps_shared_array_clone(
    out: *mut SharedArray<u8>,
    source: &SharedArray<u8>,
) {
    core::ptr::write(out, source.clone());
}

#[no_mangle]
/// This function is used for the low-level C++ interface to decrease the reference count of a shared array.
pub unsafe extern "C" fn sixtyfps_shared_array_drop(out: *mut SharedArray<u8>) {
    // ?? This won't call drop on the right type...
    core::ptr::read(out);
}

/// Somehow this is required for the extern "C" things to be exported in a dependent dynlib
#[doc(hidden)]
pub fn dummy() {
    #[derive(Clone)]
    struct Foo;
    foo(Foo);
    fn foo(f: impl Clone) {
        let _ = f.clone();
    }
}
