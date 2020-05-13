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

const XX_CONST: usize = MyStruct2::field_offsets().xx.get_byte_offset();
static D_STATIC: usize = MyStruct::field_offsets().d.get_byte_offset();

#[test]
fn test() {
    assert_eq!(offset_of!(MyStruct, a), MyStruct::field_offsets().a.get_byte_offset());
    assert_eq!(offset_of!(MyStruct, b), MyStruct::field_offsets().b.get_byte_offset());
    assert_eq!(offset_of!(MyStruct, c), MyStruct::field_offsets().c.get_byte_offset());
    assert_eq!(offset_of!(MyStruct, d), MyStruct::field_offsets().d.get_byte_offset());
    assert_eq!(offset_of!(MyStruct2, xx), MyStruct2::field_offsets().xx.get_byte_offset());
    assert_eq!(offset_of!(MyStruct2, v), MyStruct2::field_offsets().v.get_byte_offset());
    assert_eq!(offset_of!(MyStruct2, k), MyStruct2::field_offsets().k.get_byte_offset());

    assert_eq!(XX_CONST, offset_of!(MyStruct2, xx));
    assert_eq!(D_STATIC, offset_of!(MyStruct, d));
}
