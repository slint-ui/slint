// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

const POOL_SIZE: usize = 10 * 1024;

#[repr(C, align(8))]
struct AlignedPool([u8; POOL_SIZE]);

static mut NEMA_POOL: AlignedPool = AlignedPool([0; POOL_SIZE]);

static LAST_COMMAND_LIST_ID: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(-1);

static mut RING_BUFFER: nema_gfx_rs::nema_ringbuffer_t = nema_gfx_rs::nema_ringbuffer_t {
    bo: nema_gfx_rs::nema_buffer_t {
        size: 0,
        fd: 0,
        base_virt: core::ptr::null_mut(),
        base_phys: 0,
    },
    offset: 0,
    last_submission_id: 0,
};

const GPU2D_INTERRUPT_CONTROL_REG: u32 = 0x0F8;
const GPU2D_INTERRUPT_LAST_COMMAND_ID_REG: u32 = 0x148;

const GPU2D_BASE: u32 = /* peripheral base  */
    0x4000_0000 + /* ahb1 peripheral base */ 0x0002_0000 + /* GPU2D offset */ 0x0F000;
const GPU2D_PTR: *mut u32 = GPU2D_BASE as *mut u32;

#[no_mangle]
pub unsafe extern "C" fn nema_reg_read(reg: u32) -> u32 {
    GPU2D_PTR.byte_offset(reg as isize).read()
}

#[no_mangle]
pub unsafe extern "C" fn nema_reg_write(reg: u32, value: u32) {
    GPU2D_PTR.byte_offset(reg as isize).write(value)
}

#[no_mangle]
pub unsafe extern "C" fn nema_sys_init() -> i32 {
    let err = nema_gfx_rs::tsi_malloc_init_pool_aligned(
        0,
        &raw mut NEMA_POOL.0 as _,
        &raw mut NEMA_POOL.0 as _,
        POOL_SIZE as i32,
        1,
        8,
    );
    if err < 0 {
        return err;
    }

    RING_BUFFER.bo = nema_gfx_rs::nema_buffer_create(1024);

    let err = nema_gfx_rs::nema_rb_init(&raw mut RING_BUFFER, /* reset buffer */ 1);
    if err < 0 {
        return err;
    }

    return 0;
}

#[no_mangle]
pub unsafe extern "C" fn nema_buffer_create_pool(
    _pool: core::ffi::c_int,
    size: core::ffi::c_int,
) -> nema_gfx_rs::nema_buffer_t {
    nema_buffer_create(size)
}

#[no_mangle]
pub unsafe extern "C" fn nema_buffer_create(size: core::ffi::c_int) -> nema_gfx_rs::nema_buffer_t {
    let mut buffer = nema_gfx_rs::nema_buffer_t {
        size,
        fd: 0,
        base_virt: nema_gfx_rs::tsi_malloc_pool(0, size),
        base_phys: 0,
    };

    buffer.base_phys = buffer.base_virt as _;

    buffer
}

#[no_mangle]
pub unsafe extern "C" fn nema_buffer_flush(bo: *mut nema_gfx_rs::nema_buffer_t) {
    let mut scb = cortex_m::Peripherals::steal().SCB;
    scb.clean_dcache_by_address((*bo).base_phys as _, (*bo).size as _);
}

#[no_mangle]
pub unsafe extern "C" fn nema_buffer_map(
    bo: *mut nema_gfx_rs::nema_buffer_t,
) -> *mut core::ffi::c_void {
    (*bo).base_virt
}

#[no_mangle]
pub unsafe extern "C" fn nema_buffer_unmap(bo: *mut nema_gfx_rs::nema_buffer_t) {
    //defmt::info!("UNMAP BUFFER");
    let mut scb = cortex_m::Peripherals::steal().SCB;
    scb.clean_dcache_by_address((*bo).base_virt as _, (*bo).size as _);
}

#[no_mangle]
pub unsafe extern "C" fn nema_buffer_destroy(bo: *mut nema_gfx_rs::nema_buffer_t) {
    let bo = *bo;
    nema_gfx_rs::tsi_free(bo.base_virt as _);
}

#[no_mangle]
pub unsafe extern "C" fn nema_host_malloc(size: usize) -> *mut core::ffi::c_void {
    nema_gfx_rs::tsi_malloc_pool(0, size as i32)
}

#[no_mangle]
pub unsafe extern "C" fn nema_host_free(ptr: *mut core::ffi::c_void) {
    nema_gfx_rs::tsi_free(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn nema_mutex_lock(_mutex_id: core::ffi::c_int) -> core::ffi::c_int {
    // TODO?
    return 0;
}

#[no_mangle]
pub unsafe extern "C" fn nema_mutex_unlock(_mutex_id: core::ffi::c_int) -> core::ffi::c_int {
    // TODO?
    return 0;
}

#[no_mangle]
pub unsafe extern "C" fn nema_wait_irq_cl(cl_id: core::ffi::c_int) -> core::ffi::c_int {
    //defmt::info!("waiting for cl_id {}", cl_id);
    while LAST_COMMAND_LIST_ID.load(core::sync::atomic::Ordering::Relaxed) < cl_id {}

    return 0;
}

use embassy_stm32::bind_interrupts;

struct GPU2DIrqHandler {}

impl embassy_stm32::interrupt::typelevel::Handler<embassy_stm32::interrupt::typelevel::GPU2D>
    for GPU2DIrqHandler
{
    unsafe fn on_interrupt() {
        let mut flags = nema_reg_read(GPU2D_INTERRUPT_CONTROL_REG);
        //defmt::info!("GPU2D IRQ flags = {}", flags);

        if flags & 0x1 != /* GPU2D_FLAG_CLC */ 0 {
            // clear command list complete flag
            flags &= !0x1;
            nema_reg_write(GPU2D_INTERRUPT_CONTROL_REG, flags);

            LAST_COMMAND_LIST_ID.store(
                nema_reg_read(GPU2D_INTERRUPT_LAST_COMMAND_ID_REG) as _,
                core::sync::atomic::Ordering::Relaxed,
            );
            // defmt::info!(
            //     "last command id set {}",
            //     LAST_COMMAND_LIST_ID.load(core::sync::atomic::Ordering::Relaxed,)
            // );
        }
    }
}

bind_interrupts!(struct Irqs {
    GPU2D => GPU2DIrqHandler;
});
