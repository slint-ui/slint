use super::DataTransfer;

#[repr(C)]
pub struct DataTransferOpaque {
    _unused_0: *mut core::ffi::c_void,
    _unused_1: *mut core::ffi::c_void,
    _unused_2: *mut core::ffi::c_void,
}

const _: () = {
    assert!(core::mem::align_of::<DataTransfer>() == core::mem::align_of::<DataTransferOpaque>());
    assert!(core::mem::size_of::<DataTransfer>() == core::mem::size_of::<DataTransferOpaque>());
};
