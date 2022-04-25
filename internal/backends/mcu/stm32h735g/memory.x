/* Copyright © 2021 rp-rs organization
 SPDX-License-Identifier: MIT OR Apache-2.0 */

MEMORY
{
  FLASH  : ORIGIN = 0x08000000, LENGTH = 1M
  RAM    : ORIGIN = 0x24000000, LENGTH = 320K
  SDRAM  : ORIGIN = 0x70000000, LENGTH = 16384K
}

_stack_start = ORIGIN(RAM) + LENGTH(RAM);

SECTIONS {
     .frame_buffer (NOLOAD) : {
       *(.frame_buffer);
       . = ALIGN(4);
     } > SDRAM
}
