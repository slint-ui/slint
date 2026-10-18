/* Copyright © 2021 rp-rs organization
 SPDX-License-Identifier: MIT OR Apache-2.0
 Copied from https://github.com/rp-rs/rp-hal/blob/main/rp235x-hal-examples/memory.x
*/

MEMORY {
    /*
     * The RP2350 has either external or internal flash.
     *
     * Waveshare RP2350-Touch-LCD-2.8 has 16 MiB flash.
     */
    FLASH : ORIGIN = 0x10000000, LENGTH = 2048K
    ASSETS : ORIGIN = 0x10200000, LENGTH = 14336K
    /*
     * RAM consists of 8 banks, SRAM0-SRAM7, with a striped mapping.
     * This is usually good for performance, as it distributes load on
     * those banks evenly.
     */
    RAM : ORIGIN = 0x20000000, LENGTH = 512K
    /*
     * RAM banks 8 and 9 use a direct mapping. They can be used to have
     * memory areas dedicated for some specific job, improving predictability
     * of access times.
     * Example: Separate stacks for core0 and core1.
     */
    CORE1_STACK : ORIGIN = 0x20080000, LENGTH = 8K
}

SECTIONS {
    .core1_stack (NOLOAD) : ALIGN(32)
    {
        KEEP(*(.core1_stack));
    } > CORE1_STACK
} INSERT AFTER .uninit;

__sheap = __euninit;
_stack_end = __euninit;
ASSERT(_stack_start >= _stack_end + 24K, "Pico printer requires at least 24 KiB of main-core stack");
ASSERT(SIZEOF(.core1_stack) == 8K, "Pico core 1 stack must occupy its dedicated 8 KiB SRAM banks");

SECTIONS {
    /* ### Boot ROM info
     *
     * Goes after .vector_table, to keep it in the first 4K of flash
     * where the Boot ROM (and picotool) can find it
     */
    .start_block : ALIGN(4)
    {
        __start_block_addr = .;
        KEEP(*(.start_block));
        KEEP(*(.boot_info));
    } > FLASH

} INSERT AFTER .vector_table;

/* move .text to start /after/ the boot info */
_stext = ALIGN(ADDR(.start_block) + SIZEOF(.start_block), 8);

SECTIONS {
    /* ### Picotool 'Binary Info' Entries
     *
     * Picotool looks through this block (as we have pointers to it in our
     * header) to find interesting information.
     */
    .bi_entries : ALIGN(4)
    {
        /* We put this in the header */
        __bi_entries_start = .;
        /* Here are the entries */
        KEEP(*(.bi_entries));
        /* Keep this block a nice round size */
        . = ALIGN(4);
        /* We put this in the header */
        __bi_entries_end = .;
    } > FLASH
} INSERT AFTER .text;

SECTIONS {
    /* ### Boot ROM extra info
     *
     * Goes after everything in our program, so it can contain a signature.
     */
    .end_block : ALIGN(4)
    {
        __end_block_addr = .;
        KEEP(*(.end_block));
    } > FLASH

} INSERT AFTER .uninit;

PROVIDE(start_to_end = __end_block_addr - __start_block_addr);
PROVIDE(end_to_start = __start_block_addr - __end_block_addr);

/* Stable artwork precedes other read-only data, independently of executable growth. */
SECTIONS {
    .assets : ALIGN(4096)
    {
        __assets_start = .;
        *(SORT_BY_NAME(.pico_assets.*));
        *(SORT_BY_NAME(.rodata.*SLINT_EMBEDDED_RESOURCE_*_DATA*));
        *(SORT_BY_NAME(.rodata .rodata.*));
        . = ALIGN(4);
        __assets_end = .;
    } > ASSETS
} INSERT BEFORE .rodata;

__flash_binary_end = MAX(__assets_end, LOADADDR(.data) + SIZEOF(.data));
