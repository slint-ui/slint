use const_field_offset::FieldOffsets;
use memoffset::offset_of;

#[derive(FieldOffsets)]
#[repr(C)]
struct MyStruct {
    a: u8,
    b: u16,
    c: u8,
    d: u16,
}

#[derive(FieldOffsets)]
#[repr(C)]
struct MyStruct2 {
    k: core::cell::Cell<isize>,
    xx: MyStruct,
    v: u32,
}

const XX_CONST: usize = MyStruct2::field_offsets().xx;
static D_STATIC: usize = MyStruct::field_offsets().d;

#[test]
fn test() {
    assert_eq!(offset_of!(MyStruct, a), MyStruct::field_offsets().a);
    assert_eq!(offset_of!(MyStruct, b), MyStruct::field_offsets().b);
    assert_eq!(offset_of!(MyStruct, c), MyStruct::field_offsets().c);
    assert_eq!(offset_of!(MyStruct, d), MyStruct::field_offsets().d);
    assert_eq!(offset_of!(MyStruct2, xx), MyStruct2::field_offsets().xx);
    assert_eq!(offset_of!(MyStruct2, v), MyStruct2::field_offsets().v);
    assert_eq!(offset_of!(MyStruct2, k), MyStruct2::field_offsets().k);

    assert_eq!(XX_CONST, offset_of!(MyStruct2, xx));
    assert_eq!(D_STATIC, offset_of!(MyStruct, d));
}
