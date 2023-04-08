# Slint UEFI demo

This example demonstrates slint in a UEFI environment.

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
