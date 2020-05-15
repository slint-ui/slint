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
}

#[derive(Debug)]
struct SomeStruct(u32);
impl Hello for SomeStruct {
    fn foo(&self, xx: u32) -> u32 {
        println!("calling foo {} + {}", self.0, xx);
        self.0 + xx
    }

    fn foo_mut(&mut self, xx: u32) -> u32 {
        println!("calling foo_mut {} + {}", self.0, xx);
        self.0 += xx;
        self.0
    }

    fn construct(init: u32) -> Self {
        println!("calling Construct {}", init);
        Self(init)
    }

    fn assoc() -> isize {
        32
    }
}
impl HelloConsts for SomeStruct {
    const CONSTANT: usize = 88;
}

static SOME_STRUCT_TYPE: HelloVTable = HelloVTable_static!(SomeStruct);

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
}
