/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

#![allow(improper_ctypes_definitions)]

use std::rc::Rc;
use vtable::*;
#[vtable]
struct FooVTable {
    drop_in_place: fn(VRefMut<FooVTable>) -> Layout,
    dealloc: fn(&FooVTable, ptr: *mut u8, layout: Layout),
    rc_string: fn(VRef<FooVTable>) -> Rc<String>,
}

#[derive(Debug, const_field_offset::FieldOffsets)]
#[repr(C)]
struct SomeStruct {
    e: u8,
    x: String,
    foo: Rc<String>,
}
impl Foo for SomeStruct {
    fn rc_string(&self) -> Rc<String> {
        self.foo.clone()
    }
}

FooVTable_static!(static SOME_STRUCT_TYPE for SomeStruct);

#[test]
fn rc_test() {
    let string = Rc::new("hello".to_string());
    let rc = VRc::new(SomeStruct { e: 42, x: "44".into(), foo: string.clone() });
    let string_copy = rc.borrow().rc_string();
    assert!(Rc::ptr_eq(&string, &string_copy));
    drop(string_copy);
    let w = rc.downgrade();
    {
        let rc2 = w.upgrade().unwrap();
        let string_copy = rc2.borrow().rc_string();
        assert!(Rc::ptr_eq(&string, &string_copy));
    }
    drop(rc);
    assert_eq!(Rc::strong_count(&string), 1);
    assert!(w.upgrade().is_none());
}
