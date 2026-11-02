// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// Adapted from the direct Rust BOX renderer's dial_fill.S.
    .section .iram1,"ax"
    .align 4
    .global box_fill_blocks
    .type box_fill_blocks,@function
box_fill_blocks:
    entry a1, 64
    // Preserve the interrupted task's PIE state, including when called from scanout.
    rsil a8, 3
    rsr.cpenable a9
    movi a10, 8
    or a10, a10, a9
    wsr.cpenable a10
    isync
    addi a11, a1, 31
    movi a10, -16
    and a11, a11, a10
    ee.vst.128.ip q0, a11, 16
    addi a11, a11, -16
    ee.vld.128.ip q0, a3, 16
    loopnez a4, .fill_end
    ee.vst.128.ip q0, a2, 16
.fill_end:
    ee.vld.128.ip q0, a11, 16
    wsr.cpenable a9
    isync
    wsr.ps a8
    rsync
    retw.n
    .size box_fill_blocks, .-box_fill_blocks
