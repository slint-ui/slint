<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Slint UEFI demo

This example demonstrates Slint in a UEFI environment.

![Screenshot](https://user-images.githubusercontent.com/1486/231705364-8c490e25-48cf-4626-a34b-2bf7239c1245.jpg)

To build this example a suitable UEFI rust target must be installed first:

```
rustup target install x86_64-unknown-uefi
```

To build, simply pass the `--package` and `--target` arguments to cargo:

```
cargo build --package uefi-demo --target x86_64-unknown-uefi
```

The produced UEFI binary can then either be tested on real hardware by booting
it like any other bootloader or directly with QEMU (the firmware location
varies by distro):

```
qemu-system-x86_64 -bios /usr/share/edk2-ovmf/x64/OVMF.fd -kernel target/x86_64-unknown-uefi/debug/uefi-demo.efi
```
