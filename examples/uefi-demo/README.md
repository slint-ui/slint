<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Slint UEFI demo

This example demonstrates Slint in a UEFI environment.

![Screenshot](https://user-images.githubusercontent.com/1486/231705364-8c490e25-48cf-4626-a34b-2bf7239c1245.jpg)
![img.png](https://github.com/slint-ui/slint/assets/12370628/d329f6ee-184f-4c62-8b36-e32123211685)
the red rectangle is mouse. Here's how it works:
![uefi_demo_run_at_vm](https://github.com/slint-ui/slint/assets/12370628/ae534a8e-a138-4333-8813-4b4199d5e806)

To build this example a suitable UEFI rust target must be installed first:

```shell
rustup target install x86_64-unknown-uefi
```

To build, simply pass the `--package` and `--target` arguments to cargo:

```shell
cargo build --package uefi-demo --target x86_64-unknown-uefi
```

The produced UEFI binary can then either be tested on real hardware by booting
it like any other bootloader or directly with QEMU (the firmware location
varies by distro):

```shell
qemu-system-x86_64 -serial stdio -bios /usr/share/edk2-ovmf/x64/OVMF.fd -kernel target/x86_64-unknown-uefi/debug/uefi-demo.efi
```

**NOTE:** the OVMF are not support mouse moving. please run it at VM or your PC if you want to use mouse.
