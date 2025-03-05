/* Copyright © 2025 SixtyFPS GmbH
 SPDX-License-Identifier: MIT OR Apache-2.0 */

MEMORY
{
  FLASH    : ORIGIN = 0x08000000, LENGTH = 4096K
  RAM      : ORIGIN = 0x20000000, LENGTH = 3008K
  HSPI_ROM : ORIGIN = 0xA0000000, LENGTH = 65536K
}

SECTIONS {
     .slint_assets : {
       . = ALIGN(4);
      __s_slint_assets = .;
       *(.slint_assets);
       . = ALIGN(4);
       *(.slint_code);
       . = ALIGN(4);
     } > HSPI_ROM

    __e_slint_assets = .;
    __si_slint_assets = LOADADDR(.slint_assets);
}
