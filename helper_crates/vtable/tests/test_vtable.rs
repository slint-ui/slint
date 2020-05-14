use vtable::*;
#[vtable]
/// This is the actual doc
struct HelloVTable {
    foo: fn(VRef<'_, HelloVTable>, u32) -> u32,
    foo_mut: fn(VRefMut<'_, HelloVTable>, u32) -> u32,
    construct: fn(*const HelloVTable, u32) -> VBox<HelloVTable>,
    assoc: fn(*const HelloVTable) -> isize,

    drop: fn(VRefMut<'_, HelloVTable>),
}

#[derive(Debug)]
struct SomeStruct(u32);
impl Hello for SomeStruct {
    fn foo(&self, xx: u32) -> u32 {
        println!("calling foo {} + {}", self.0, xx);
        self.0 + xx
    }

    fn foo_mut(&mut self, xx: u32) -> u32 {
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

#[test]
fn test() {
    //let vt = HelloVTable::new::<SomeStruct>();
    let mut vt = HelloVTable::new::<SomeStruct>();
    let vt = unsafe { HelloType::from_raw(std::ptr::NonNull::from(&mut vt)) };
    assert_eq!(vt.assoc(), 32);
    let mut bx = vt.construct(89);
    assert_eq!(bx.foo(1), 90);
    assert_eq!(bx.foo_mut(1), 91);
    assert_eq!(bx.foo(1), 92);
}


