use vtable::vtable;
#[vtable]
/// This is the actual doc
struct HelloVTable {
    foo: fn(*const HelloVTable, *mut HelloImpl, u32) -> u32,

    construct: fn(*const HelloVTable, u32) -> Box<HelloImpl>,

    assoc: fn(*const HelloVTable) -> isize,

    drop: fn(*const HelloVTable, *mut HelloImpl),
}

struct SomeStruct(u32);
impl Hello for SomeStruct {
    fn foo(&mut self, xx: u32) -> u32 {
        self.0 + xx
    }

    fn construct(init: u32) -> Self {
        Self(init)
    }

    fn assoc() -> isize {
        32
    }
}

#[test]
fn test() {
    let vt = HelloVTable::new::<SomeStruct>();
    assert_eq!(vt.assoc(), 32);
    let mut bx = vt.construct(89);
    assert_eq!(bx.foo(1), 90);
}
