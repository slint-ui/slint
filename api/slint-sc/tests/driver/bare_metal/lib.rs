// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

// cSpell: ignore cpacr endr fpexc orrs vmsr

//! The platform of the test programs on bare-metal targets, which QEMU runs
//! with Arm semihosting: the start-up code, which calls the test program's
//! `main` through [`entry!`], and the `semihosting` crate for the files,
//! the exit status, and the panics.
//!
//! On other targets, the crate is empty.

#![no_std]

#[cfg(target_os = "none")]
pub use semihosting;

#[cfg(target_os = "none")]
use core::arch::global_asm;

/// Make `main` the test program's entry point, which the start-up code calls.
/// The test program forbids `unsafe_code`, which rustc doesn't check in the
/// expansion of a macro from another crate.
#[macro_export]
macro_rules! entry {
    ($main:path) => {
        #[unsafe(no_mangle)]
        extern "C" fn slint_sc_test_main() -> ! {
            if let Err(error) = $main() {
                $crate::semihosting::eprintln!("Error: {error:?}");
                $crate::semihosting::process::exit(1)
            }
            $crate::semihosting::process::exit(0)
        }
    };
}

#[cfg(cortex_m)]
global_asm!(
    ".section .vector_table, \"a\"",
    ".word _stack_top",
    ".word _start",
    ".rept 14",
    ".word fault",
    ".endr",
    ".section .text._start, \"ax\"",
    ".syntax unified",
    ".thumb",
    ".global _start",
    ".thumb_func",
    "_start:",
);

#[cfg(all(cortex_m, target_abi = "eabihf"))]
global_asm!(
    // CPACR: full access to the FPU
    "ldr r0, =0xE000ED88",
    "ldr r1, [r0]",
    "ldr r2, =0xF00000",
    "orrs r1, r2",
    "str r1, [r0]",
    "dsb",
    "isb",
);

#[cfg(cortex_m)]
global_asm!("bl slint_sc_test_main");

#[cfg(all(target_os = "none", target_arch = "arm", not(cortex_m)))]
global_asm!(
    ".section .text._start, \"ax\"",
    ".arm",
    ".global _start",
    "_start:",
    "ldr sp, =_stack_top",
    // CPACR: full access to the FPU, then enable it in FPEXC
    "mrc p15, 0, r0, c1, c0, 2",
    "orr r0, r0, #0xF00000",
    "mcr p15, 0, r0, c1, c0, 2",
    "isb",
    "mov r0, #0x40000000",
    "vmsr fpexc, r0",
    "bl slint_sc_test_main",
);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
global_asm!(
    ".section .text._start, \"ax\"",
    ".global _start",
    "_start:",
    "ldr x0, =_stack_top",
    "mov sp, x0",
    // CPACR_EL1: no trap of the FP and SIMD instructions
    "mov x0, #0x300000",
    "msr cpacr_el1, x0",
    "isb",
    "bl slint_sc_test_main",
);

/// The handler of the M-profile exceptions
#[cfg(cortex_m)]
#[unsafe(no_mangle)]
extern "C" fn fault() -> ! {
    semihosting::eprintln!("fault");
    semihosting::process::exit(1)
}
