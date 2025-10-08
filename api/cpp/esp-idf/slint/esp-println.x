/* Copyright © SixtyFPS GmbH <info@slint.dev>
 SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 */

/*
Linker script needed to ensure that the meta-data from esp-println is included in the final binary. We use esp-println
and esp-backtrace in the C++ build, where this linker section isn't automatically included. For more details, see
https://github.com/esp-rs/rust/issues/266#issuecomment-3361411040
*/

SECTIONS {
  .espressif.metadata 0 (INFO) :
  {
    KEEP(*(.espressif.metadata));
  }
}
