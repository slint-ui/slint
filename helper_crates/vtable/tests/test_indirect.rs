// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `#[vtable(vtable = ...)]`: the function pointers live in a table the vtable points at
//! rather than in the vtable itself, so several vtables can share one copy of them.

#![no_std]
extern crate alloc;
use alloc::boxed::Box;
use core::pin::Pin;
use vtable::*;

#[vtable(vtable = AnimalVTable)]
#[repr(C)]
struct AnimalFns {
    make_noise: fn(VRef<'_, AnimalVTable>, i32) -> i32,
    grow: extern "C" fn(VRefMut<'_, AnimalVTable>, i32) -> i32,
    name: extern "C" fn(Pin<VRef<'_, AnimalVTable>>) -> &'_ str,
}

/// Hand-written and deliberately small: only the entries that cannot be shared.
#[repr(C)]
struct AnimalVTable {
    fns: *const AnimalFns,
    drop_in_place: unsafe fn(VRefMut<'_, AnimalVTable>) -> vtable::Layout,
    dealloc: unsafe fn(&AnimalVTable, ptr: *mut u8, layout: vtable::Layout),
    /// Data rather than code: what distinguishes one vtable from the next here.
    legs: u32,
}

// Safety: the vtable holds function pointers and a shared function table that outlives it.
unsafe impl Sync for AnimalVTable {}

impl AnimalVTable {
    const fn new<T: Animal>(legs: u32) -> Self {
        unsafe fn drop_in_place<T>(x: VRefMut<'_, AnimalVTable>) -> vtable::Layout {
            unsafe { core::ptr::drop_in_place(x.as_ptr() as *mut T) };
            core::alloc::Layout::new::<T>().into()
        }
        unsafe fn dealloc(_: &AnimalVTable, ptr: *mut u8, layout: vtable::Layout) {
            unsafe { alloc::alloc::dealloc(ptr, layout.try_into().unwrap()) }
        }
        Self {
            fns: &const { AnimalFns::new::<T>() },
            drop_in_place: drop_in_place::<T>,
            dealloc,
            legs,
        }
    }
}

unsafe impl VTableMetaDropInPlace for AnimalVTable {
    unsafe fn drop_in_place(vtable: &AnimalVTable, ptr: *mut u8) -> vtable::Layout {
        unsafe {
            (vtable.drop_in_place)(VRefMut::from_raw(
                core::ptr::NonNull::from(vtable),
                core::ptr::NonNull::new_unchecked(ptr).cast(),
            ))
        }
    }
    unsafe fn dealloc(vtable: &AnimalVTable, ptr: *mut u8, layout: vtable::Layout) {
        unsafe { (vtable.dealloc)(vtable, ptr, layout) }
    }
}

struct Dog {
    strength: i32,
}
impl Animal for Dog {
    fn make_noise(&self, intensity: i32) -> i32 {
        self.strength * intensity
    }
    fn grow(&mut self, by: i32) -> i32 {
        self.strength += by;
        self.strength
    }
    fn name(self: Pin<&Self>) -> &str {
        "dog"
    }
}

static DOG_VT: AnimalVTable = AnimalVTable::new::<Dog>(4);
static PUPPY_VT: AnimalVTable = AnimalVTable::new::<Dog>(3);

#[test]
fn dispatch_goes_through_the_shared_table() {
    let mut dog = Dog { strength: 100 };
    let mut vref = unsafe {
        VRefMut::<AnimalVTable>::from_raw(
            core::ptr::NonNull::from(&DOG_VT),
            core::ptr::NonNull::from(&mut dog).cast(),
        )
    };
    assert_eq!(vref.make_noise(2), 200);
    assert_eq!(vref.grow(5), 105);
    assert_eq!(vref.borrow().get_vtable().legs, 4);
    assert_eq!(unsafe { Pin::new_unchecked(vref.borrow()) }.as_ref().name(), "dog");
}

#[test]
fn the_function_table_is_shared_between_vtables() {
    // The whole point: two vtables that differ only in their data still name one table.
    assert!(core::ptr::eq(DOG_VT.fns, PUPPY_VT.fns));
    assert_eq!(PUPPY_VT.legs, 3);
}

#[test]
fn the_vtable_stays_small() {
    // fns + drop_in_place + dealloc + legs, rather than one pointer per method: the
    // vtable does not grow when the trait does.
    #[repr(C)]
    struct Expected {
        fns: *const u8,
        drop_in_place: *const u8,
        dealloc: *const u8,
        legs: u32,
    }
    assert_eq!(core::mem::size_of::<AnimalVTable>(), core::mem::size_of::<Expected>());
}

#[test]
fn vrc_drops_through_the_hand_written_entries() {
    let rc: VRc<AnimalVTable, Dog> = unsafe { VRc::new_with_vtable(&DOG_VT, Dog { strength: 7 }) };
    assert_eq!(VRc::borrow(&rc).make_noise(3), 21);
    let weak = VRc::downgrade(&rc);
    drop(rc);
    assert!(weak.upgrade().is_none());
    let _ = Box::new(0); // keep `alloc` used in every configuration
}
