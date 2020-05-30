use vtable::*;
#[vtable]
/// This is the actual doc
struct HelloVTable {
    foo: fn(VRef<'_, HelloVTable>, u32) -> u32,
    foo_mut: fn(VRefMut<'_, HelloVTable>, u32) -> u32,
    construct: fn(*const HelloVTable, u32) -> VBox<HelloVTable>,
    assoc: fn(*const HelloVTable) -> isize,

    drop: fn(VRefMut<'_, HelloVTable>),

    CONSTANT: usize,

    #[offset(u32)]
    SOME_OFFSET: usize,
}

#[derive(Debug, const_field_offset::FieldOffsets)]
#[repr(C)]
struct SomeStruct {
    e: u8,
    x: u32,
}
impl Hello for SomeStruct {
    fn foo(&self, xx: u32) -> u32 {
        self.x + xx
    }

    fn foo_mut(&mut self, xx: u32) -> u32 {
        self.x += xx;
        self.x
    }

    fn construct(init: u32) -> Self {
        Self { e: 3, x: init }
    }

    fn assoc() -> isize {
        32
    }
}
impl HelloConsts for SomeStruct {
    const CONSTANT: usize = 88;
    const SOME_OFFSET: const_field_offset::FieldOffset<SomeStruct, u32> =
        SomeStruct::field_offsets().x;
}

HelloVTable_static!(static SOME_STRUCT_TYPE for SomeStruct);

#[repr(C)]
#[derive(const_field_offset::FieldOffsets)]
struct SomeStructContainer {
    e: u8,
    s: SomeStruct,
}

#[derive(Debug, const_field_offset::FieldOffsets, Default)]
#[repr(C)]
struct AnotherStruct {
    s: String,
    foo: u32,
}
impl Hello for AnotherStruct {
    fn foo(&self, xx: u32) -> u32 {
        self.s.len() as u32 + xx
    }

    fn foo_mut(&mut self, xx: u32) -> u32 {
        self.foo(xx)
    }

    fn construct(init: u32) -> Self {
        Self { s: "123".into(), foo: init }
    }

    fn assoc() -> isize {
        999
    }
}
impl HelloConsts for AnotherStruct {
    const CONSTANT: usize = 99;
    const SOME_OFFSET: const_field_offset::FieldOffset<AnotherStruct, u32> =
        AnotherStruct::field_offsets().foo;
}

HelloVTable_static!(static ANOTHERSTRUCT_VTABLE for AnotherStruct);

#[test]
fn test() {
    let vt = &SOME_STRUCT_TYPE;
    assert_eq!(vt.assoc(), 32);
    assert_eq!(vt.CONSTANT, 88);
    let mut bx = vt.construct(89);
    assert_eq!(bx.foo(1), 90);
    assert_eq!(bx.foo_mut(6), 95);
    assert_eq!(bx.foo(2), 97);
    assert_eq!(bx.get_vtable().CONSTANT, 88);

    let bx2 = VBox::<HelloVTable>::new(SomeStruct { e: 4, x: 23 });
    assert_eq!(bx2.foo(3), 26);
    assert_eq!(bx2.get_vtable().CONSTANT, 88);
    assert_eq!(*bx2.SOME_OFFSET(), 23);

    let mut hello = SomeStruct { e: 4, x: 44 };
    {
        let xref = VRef::<HelloVTable>::new(&hello);
        assert_eq!(xref.foo(0), 44);
    }
    {
        let mut xref = VRefMut::<HelloVTable>::new(&mut hello);
        assert_eq!(xref.foo_mut(2), 46);
        assert_eq!(*xref.SOME_OFFSET(), 46);
        *xref.SOME_OFFSET_mut() = 3;
        let xref2 = xref.borrow();
        assert_eq!(xref2.foo(1), 4);
    }

    let vo =
        VOffset::<SomeStructContainer, HelloVTable>::new(SomeStructContainer::field_offsets().s);
    let mut ssc = SomeStructContainer { e: 4, s: SomeStruct { e: 5, x: 32 } };
    assert_eq!(vo.apply(&ssc).foo(4), 32 + 4);
    assert_eq!(vo.apply_mut(&mut ssc).foo_mut(4), 32 + 4);
    assert_eq!(*vo.apply(&ssc).SOME_OFFSET(), 32 + 4);
}

#[test]
fn test2() {
    let mut ss = SomeStruct::construct(44);
    let mut vrss = VRefMut::<HelloVTable>::new(&mut ss);
    assert_eq!(vrss.downcast::<SomeStruct>().unwrap().foo_mut(4), 44 + 4);
    assert!(vrss.downcast::<AnotherStruct>().is_none());

    let as_ = AnotherStruct::default();
    let vras = VRef::<HelloVTable>::new(&as_);
    assert_eq!(vras.downcast::<AnotherStruct>().unwrap().foo(4), 4);
    assert!(vras.downcast::<SomeStruct>().is_none());
}
