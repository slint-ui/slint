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
        Self { x: init }
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

HelloVTable_static!(SomeStruct);
static SOME_STRUCT_TYPE: HelloVTable = SomeStruct::VTABLE;

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

    let bx2 = VBox::<HelloVTable>::new(SomeStruct { x: 23 });
    assert_eq!(bx2.foo(3), 26);
    assert_eq!(bx2.get_vtable().CONSTANT, 88);
    assert_eq!(*bx2.SOME_OFFSET(), 23);

    let mut hello = SomeStruct { x: 44 };
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
}
