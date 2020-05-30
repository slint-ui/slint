mod m1 {

    /**
    ```
    use vtable::*;
    #[vtable]
    struct MyVTable {
        foo: fn(VRef<'_, MyVTable>) -> u32,
        create: fn(&MyVTable)->VBox<MyVTable>,
        drop: fn(VRefMut<'_, MyVTable>),
    }
    struct S(u32);
    impl My for S {
        fn foo(&self) -> u32 { self.0 }
        fn create() -> Self { S(55) }
    }
    struct R(u8);
    impl My for R {
        fn foo(&self) -> u32 { (self.0 + 3) as _ }
        fn create() -> Self { R(8) }
    }
    MyVTable_static!(static S_VT for S);
    MyVTable_static!(static R_VT for R);
    let x = S_VT.create();
    ```
    */
    #[cfg(doctest)]
    const _: u32 = 0;

    /**
    Test that one cannot call a function of the vtable with the wrong type
    ```compile_fail
    use vtable::*;
    #[vtable]
    struct MyVTable {
        foo: fn(VRef<'_, MyVTable>) -> u32,
        create: fn(&MyVTable)->VBox<MyVTable>,
        drop: fn(VRefMut<'_, MyVTable>),
    }
    struct S(u32);
    impl My for S {
        fn foo(&self) -> u32 { self.0 }
        fn create() -> Self { S(55) }
    }
    struct R(u8);
    impl My for R {
        fn foo(&self) -> u32 { (self.0 + 3) as _ }
        fn create() -> Self { R(8) }
    }
    MyVTable_static!(static S_VT for S);
    MyVTable_static!(static R_VT for R);
    let x = S_VT.create();
    //unsafe     // must compile when unsafe
    { (R_VT.foo)(x.borrow()); }
    ```
    */
    #[cfg(doctest)]
    const _: u32 = 0;
}

mod test_vrefmut {
    /**
    VRefMut cannot be cloned
    ```compile_fail
    use vtable::*;
    #[vtable]
    struct MyVTable { }
    fn xx(x : VRefMut<'a, MyVTable>) {
        let x2 = x.clone()
    }
    ```
    */
    #[cfg(doctest)]
    const _1: u32 = 0;

    /**
    VRefMut's dereference cannot be copied
    ```compile_fail
    use vtable::*;
    #[vtable]
    struct MyVTable { }
    fn xx(x : VRefMut<'a, MyVTable>) {
        let x2 = *x;
    }
    ```
    */
    #[cfg(doctest)]
    const _2: u32 = 0;
}
