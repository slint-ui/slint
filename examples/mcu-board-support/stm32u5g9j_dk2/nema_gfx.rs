#[no_mangle]
pub unsafe extern "C" fn nema_reg_read(reg: u32) -> u32 {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn nema_reg_write(reg: u32, value: u32) {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn nema_buffer_flush(bo: *mut nema_gfx_rs::nema_buffer_t) {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn nema_sys_init() -> i32 {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn nema_buffer_map(
    bo: *mut nema_gfx_rs::nema_buffer_t,
) -> *mut core::ffi::c_void {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn nema_host_malloc(size: usize) -> *mut core::ffi::c_void {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn nema_host_free(ptr: *mut core::ffi::c_void) {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn nema_mutex_lock(mutex_id: core::ffi::c_int) -> core::ffi::c_int {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn nema_mutex_unlock(mutex_id: core::ffi::c_int) -> core::ffi::c_int {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn nema_wait_irq_cl(cl_id: core::ffi::c_int) -> core::ffi::c_int {
    todo!()
}

#[no_mangle]
pub unsafe extern "C" fn nema_buffer_create_pool(
    pool: core::ffi::c_int,
    size: core::ffi::c_int,
) -> nema_gfx_rs::nema_buffer_t {
    todo!()
}
