<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Troubleshooting

You may run into compile or run-time issues due to Slint's requirements. The following sections
track issues we're aware of and how to solve them.

## Rust Compilation Error During Slint Build

You see the following error:

```
error: the `-Z` flag is only accepted on the nightly channel of Cargo, but this is the `stable` channel
```

Solution: You need to configure your Rust toolchain to use the esp channel. Either set the `RUSTUP_TOOLCHAIN` environment variable to the value `esp` or create a file called `rust-toolchain.toml` in your project directory with the following contents:
```toml
[toolchain]
channel = "esp"
```


## The device crashes at boot or enter a boot loop

One reason could be that you don't have enough ram for the heap or the stack.
Make sure that the stack is big enough (~8KiB), and that all the RAM was made available for the heap allocator.

## Wrong colors shown

If colors look inverted on your display, it may be an incompatibility between how RGB565 colors are ordered in little-endian
and your display expecting a different byte order. Typically, esp32 devices are little ending and display controllers often
expect big-endian or `esp_lcd` configures them accordingly. Therefore, by default Slint converts pixels to big-endian.
If your display controller expects little endian, set the `byte_swap` field in `SlintPlatformConfiguration` to `false`.

## Errors about multiple symbol definitions when linking

You see errors at application link time such as these:

```
compiler_builtins.4c2482f45199cb1e-cgu.05:(.text.__udivdi3+0x0): multiple definition of `__udivdi3'; .../libgcc.a(_udivdi3.o): first defined here
```

Solution: Add `-Wl,--allow-multiple-definition` to your linker flags by using the following cmake command:

```cmake
target_link_options(${COMPONENT_LIB} PUBLIC -Wl,--allow-multiple-definition)
```
