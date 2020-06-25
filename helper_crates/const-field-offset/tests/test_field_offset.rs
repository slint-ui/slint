use const_field_offset::*;
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

    assert_eq!(offset_of!(MyStruct, a), MyStruct_field_offsets::a.get_byte_offset());
    assert_eq!(offset_of!(MyStruct, b), MyStruct_field_offsets::b.get_byte_offset());
    assert_eq!(offset_of!(MyStruct, c), MyStruct_field_offsets::c.get_byte_offset());
    assert_eq!(offset_of!(MyStruct, d), MyStruct_field_offsets::d.get_byte_offset());
    assert_eq!(offset_of!(MyStruct2, xx), MyStruct2_field_offsets::xx.get_byte_offset());
    assert_eq!(offset_of!(MyStruct2, v), MyStruct2_field_offsets::v.get_byte_offset());
    assert_eq!(offset_of!(MyStruct2, k), MyStruct2_field_offsets::k.get_byte_offset());
}

#[derive(FieldOffsets)]
#[repr(C)]
#[pin]
struct MyStructPin {
    phantom: core::marker::PhantomPinned,
    a: u8,
    b: u16,
    c: u8,
    d: u16,
}

#[derive(FieldOffsets)]
#[repr(C)]
#[pin]
struct MyStruct2Pin {
    phantom: core::marker::PhantomPinned,
    k: core::cell::Cell<isize>,
    xx: MyStruct,
    v: u32,
}

const XX_CONST_PIN: usize = MyStruct2Pin::field_offsets().xx.get_byte_offset();
static D_STATIC_PIN: usize = MyStructPin::field_offsets().d.get_byte_offset();

#[test]
fn test_pin() {
    assert_eq!(offset_of!(MyStructPin, a), MyStructPin::field_offsets().a.get_byte_offset());
    assert_eq!(offset_of!(MyStructPin, b), MyStructPin::field_offsets().b.get_byte_offset());
    assert_eq!(offset_of!(MyStructPin, c), MyStructPin::field_offsets().c.get_byte_offset());
    assert_eq!(offset_of!(MyStructPin, d), MyStructPin::field_offsets().d.get_byte_offset());
    assert_eq!(offset_of!(MyStruct2Pin, xx), MyStruct2Pin::field_offsets().xx.get_byte_offset());
    assert_eq!(offset_of!(MyStruct2Pin, v), MyStruct2Pin::field_offsets().v.get_byte_offset());
    assert_eq!(offset_of!(MyStruct2Pin, k), MyStruct2Pin::field_offsets().k.get_byte_offset());

    assert_eq!(XX_CONST_PIN, offset_of!(MyStruct2Pin, xx));
    assert_eq!(D_STATIC_PIN, offset_of!(MyStructPin, d));
}
