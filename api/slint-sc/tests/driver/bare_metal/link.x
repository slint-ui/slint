/* Copyright © SixtyFPS GmbH <info@slint.dev>
 SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0 */

/* The layout of a test program on a bare-metal target. The driver defines
   CODE and RAM, addresses of memory in the QEMU machine. QEMU zero-fills
   .bss when it loads the program. */
ENTRY(_start)
SECTIONS
{
    . = CODE;
    .text : { KEEP(*(.vector_table)) *(.text .text.*) }
    .rodata : { *(.rodata .rodata.*) }
    . = RAM;
    .data : { *(.data .data.*) }
    .bss : { *(.bss .bss.*) }
    . = ALIGN(16) + 0x100000;
    _stack_top = .;
}
