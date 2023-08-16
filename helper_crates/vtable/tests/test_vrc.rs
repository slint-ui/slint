// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

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
#[pin]
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
    let string_copy = VRc::borrow(&rc).rc_string();
    assert!(Rc::ptr_eq(&string, &string_copy));
    assert_eq!(VRc::strong_count(&rc), 1);
    assert_eq!(rc.e, 42);
    drop(string_copy);
    let w = VRc::downgrade(&rc);
    assert_eq!(VRc::strong_count(&rc), 1);
    {
        let rc2 = w.upgrade().unwrap();
        let string_copy = VRc::borrow(&rc2).rc_string();
        assert!(Rc::ptr_eq(&string, &string_copy));
        assert_eq!(VRc::strong_count(&rc), 2);
        assert!(VRc::ptr_eq(&rc, &rc2));
        // one in `string`, one in `string_copy`, one in the shared region.
        assert_eq!(Rc::strong_count(&string), 3);
        assert_eq!(rc2.e, 42);
    }
    assert_eq!(VRc::strong_count(&rc), 1);
    drop(rc);
    assert_eq!(Rc::strong_count(&string), 1);
    assert!(w.upgrade().is_none());

    let rc = VRc::new(SomeStruct { e: 55, x: "_".into(), foo: string.clone() });
    assert!(VRc::ptr_eq(&rc, &rc.clone()));
    assert_eq!(Rc::strong_count(&string), 2);
    assert_eq!(VRc::strong_count(&rc), 1);
    assert_eq!(rc.e, 55);
    drop(rc);
    assert_eq!(Rc::strong_count(&string), 1);
}

#[test]
fn rc_dyn_test() {
    let string = Rc::new("hello".to_string());
    let origin = VRc::new(SomeStruct { e: 42, x: "44".into(), foo: string.clone() });
    let origin_weak = VRc::downgrade(&origin);
    let rc: VRc<FooVTable> = VRc::into_dyn(origin);
    let string_copy = VRc::borrow(&rc).rc_string();
    assert!(Rc::ptr_eq(&string, &string_copy));
    assert_eq!(VRc::strong_count(&rc), 1);
    drop(string_copy);
    let w = VRc::downgrade(&rc);
    assert_eq!(VRc::strong_count(&rc), 1);
    {
        let rc2 = w.upgrade().unwrap();
        let string_copy = VRc::borrow(&rc2).rc_string();
        assert!(Rc::ptr_eq(&string, &string_copy));
        assert_eq!(VRc::strong_count(&rc), 2);
        assert!(VRc::ptr_eq(&rc, &rc2));
        // one in `string`, one in `string_copy`, one in the shared region.
        assert_eq!(Rc::strong_count(&string), 3);
    }
    assert_eq!(VRc::strong_count(&rc), 1);
    {
        let rc_origin = origin_weak.upgrade().unwrap();
        assert_eq!(rc_origin.e, 42);
        assert!(VRc::ptr_eq(&VRc::into_dyn(rc_origin), &rc));
    }
    drop(rc);
    assert_eq!(Rc::strong_count(&string), 1);
    assert!(w.upgrade().is_none());
    assert!(origin_weak.upgrade().is_none());
    assert!(origin_weak.into_dyn().upgrade().is_none());

    let rc: VRc<FooVTable> =
        VRc::into_dyn(VRc::new(SomeStruct { e: 55, x: "_".into(), foo: string.clone() }));
    assert!(VRc::ptr_eq(&rc, &rc.clone()));
    assert_eq!(Rc::strong_count(&string), 2);
    assert_eq!(VRc::strong_count(&rc), 1);
    drop(rc);
    assert_eq!(Rc::strong_count(&string), 1);
}

#[derive(Debug, const_field_offset::FieldOffsets)]
#[repr(C)]
struct SyncStruct {
    e: std::sync::Mutex<String>,
}
impl Foo for SyncStruct {
    fn rc_string(&self) -> Rc<String> {
        Rc::new(self.e.lock().unwrap().clone())
    }
}

FooVTable_static!(static SYNC_STRUCT_TYPE for SyncStruct);
#[test]
fn rc_test_threading() {
    let rc = VRc::new(SyncStruct { e: std::sync::Mutex::new("44".into()) });
    let weak = VRc::downgrade(&rc);
    assert_eq!(*rc.rc_string(), "44");
    let mut handles = Vec::new();
    for _ in 0..10 {
        let rc = rc.clone();
        handles.push(std::thread::spawn(move || {
            let _w = VRc::downgrade(&rc);
            for _ in 0..10 {
                let _clone = rc.clone();
                let weak = VRc::downgrade(&rc);
                let rc2 = weak.upgrade().unwrap();
                let mut lock = rc2.e.lock().unwrap();
                let v: u32 = lock.parse().unwrap();
                *lock = (v + 1).to_string();
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(*rc.rc_string(), "144");
    let h = std::thread::spawn(move || drop(rc));
    drop(weak);
    h.join().unwrap();
}

#[vtable]
struct AppVTable {
    drop_in_place: fn(VRefMut<AppVTable>) -> Layout,
    dealloc: fn(&AppVTable, ptr: *mut u8, layout: Layout),
}

#[derive(Debug, const_field_offset::FieldOffsets)]
#[repr(C)]
#[pin]
struct AppStruct {
    some: SomeStruct,
    another_struct: SomeStruct,
}

impl AppStruct {
    fn new() -> VRc<AppVTable, Self> {
        let string = Rc::new("hello".to_string());
        let self_ = Self {
            some: SomeStruct { e: 55, x: "_".into(), foo: string.clone() },
            another_struct: SomeStruct { e: 100, x: "_".into(), foo: string.clone() },
        };
        VRc::new(self_)
    }
}

impl App for AppStruct {}

AppVTable_static!(static APP_STRUCT_TYPE for AppStruct);

#[test]
fn rc_map_test() {
    fn get_struct_value(instance: &VRcMapped<AppVTable, SomeStruct>) -> u8 {
        let field_ref = SomeStruct::FIELD_OFFSETS.e.apply_pin(instance.as_pin_ref());
        *field_ref
    }

    let app_rc = AppStruct::new();

    let some_struct_ref =
        VRc::map(app_rc.clone(), |app| AppStruct::FIELD_OFFSETS.some.apply_pin(app));

    // check clone() compiles
    {
        let _some_clone = some_struct_ref.clone();
    }

    let other_struct_ref =
        VRc::map(app_rc.clone(), |app| AppStruct::FIELD_OFFSETS.another_struct.apply_pin(app));

    let weak_struct_ref = VRcMapped::downgrade(&some_struct_ref);

    {
        let _weak_clone = weak_struct_ref.clone();
    }

    {
        let strong_struct_ref = weak_struct_ref.upgrade().unwrap();
        assert_eq!(get_struct_value(&strong_struct_ref), 55);
    }

    {
        assert_eq!(get_struct_value(&other_struct_ref), 100);
    }

    drop(app_rc);

    {
        let strong_struct_ref = weak_struct_ref.upgrade().unwrap();
        let e_field = SomeStruct::FIELD_OFFSETS.e.apply_pin(strong_struct_ref.as_pin_ref());
        assert_eq!(*e_field, 55);
    }

    let double_map =
        VRcMapped::map(other_struct_ref, |some| SomeStruct::FIELD_OFFSETS.e.apply_pin(some));
    let double_map_weak = VRcMapped::downgrade(&double_map);
    drop(double_map);
    assert_eq!(*double_map_weak.upgrade().unwrap(), 100);

    drop(some_struct_ref);

    assert!(weak_struct_ref.upgrade().is_none());
    assert!(double_map_weak.upgrade().is_none());

    let def = VWeakMapped::<AppVTable, SomeStruct>::default();
    assert!(def.upgrade().is_none());
}

#[test]
fn rc_map_dyn_test() {
    fn get_struct_value(instance: &VRcMapped<AppVTable, SomeStruct>) -> u8 {
        let field_ref = SomeStruct::FIELD_OFFSETS.e.apply_pin(instance.as_pin_ref());
        *field_ref
    }

    let app_rc = AppStruct::new();
    let app_dyn = VRc::into_dyn(app_rc);

    let some_struct_ref = VRc::map_dyn(app_dyn.clone(), |app_dyn| {
        let app_ref = VRef::downcast_pin(app_dyn).unwrap();
        AppStruct::FIELD_OFFSETS.some.apply_pin(app_ref)
    });

    assert_eq!(get_struct_value(&some_struct_ref), 55);
}

#[test]
fn rc_map_origin() {
    let app_rc = AppStruct::new();

    let some_struct_ref =
        VRc::map(app_rc.clone(), |app| AppStruct::FIELD_OFFSETS.some.apply_pin(app));

    drop(app_rc);

    let strong_origin = VRcMapped::origin(&some_struct_ref);

    drop(some_struct_ref);

    let weak_origin = VRc::downgrade(&strong_origin);

    assert_eq!(VRc::strong_count(&strong_origin), 1);

    drop(strong_origin);

    assert!(weak_origin.upgrade().is_none());
}

#[test]
fn ptr_eq() {
    let string = Rc::new("hello".to_string());

    let vrc1 = VRc::new(SomeStruct { e: 42, x: "42".into(), foo: string.clone() });
    let vweak1 = VRc::downgrade(&vrc1);

    let vrc2 = VRc::new(SomeStruct { e: 23, x: "23".into(), foo: string });
    let vweak2 = VRc::downgrade(&vrc2);

    let vweak1clone = vweak1.clone();
    let vrc2clone = vrc2.clone();
    let vweak2clone = VRc::downgrade(&vrc2clone);

    assert!(VRc::ptr_eq(&vrc2, &vrc2clone));

    assert!(vtable::VWeak::ptr_eq(&vweak1, &vweak1));
    assert!(vtable::VWeak::ptr_eq(&vweak1clone, &vweak1clone));
    assert!(vtable::VWeak::ptr_eq(&vweak1clone, &vweak1));
    assert!(vtable::VWeak::ptr_eq(&vweak1, &vweak1clone));
    assert!(vtable::VWeak::ptr_eq(&vweak2clone, &vweak2));
    assert!(vtable::VWeak::ptr_eq(&vweak2, &vweak2clone));
    assert!(vtable::VWeak::ptr_eq(&vweak2, &vweak2));
    assert!(vtable::VWeak::ptr_eq(&vweak2clone, &vweak2clone));

    assert!(!vtable::VWeak::ptr_eq(&vweak1clone, &vweak2));
    assert!(!vtable::VWeak::ptr_eq(&vweak1clone, &vweak2clone));
    assert!(!vtable::VWeak::ptr_eq(&vweak1, &vweak2));
    assert!(!vtable::VWeak::ptr_eq(&vweak1, &vweak2clone));
    assert!(!vtable::VWeak::ptr_eq(&vweak2clone, &vweak1));
    assert!(!vtable::VWeak::ptr_eq(&vweak2clone, &vweak1clone));
    assert!(!vtable::VWeak::ptr_eq(&vweak2, &vweak1));
    assert!(!vtable::VWeak::ptr_eq(&vweak2, &vweak1clone));
}
