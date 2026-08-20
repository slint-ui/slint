# Slint Safety Critical UI Demo

We aim to make Slint suitable in environments that require reliable display of safety-critical UI, such as vehicles of any kind, medical devices, or industrial tools and machines.

This example serves as a starting point for a setup where strict separation of domains into a safety domain and an application domain is implemented either by hardware or system software:

- The application domain is for example a Slint based application running on Linux, rendering into some kind of surface that only indirectly makes it to the physical output screen.
- The safety domain could be implemented by means of hardware or software. This domain is restricted and would be subject to a device specific safety certification. We aim to demonstrate
  that Slint is suitable for this use-case.

The safety domain is assumed to be split into two parts again:

 - A system or hardware specific layer.
 - The Rust-based Slint and application safety layer.

 This directory contains the Slint safety layer scaffolding and interface. The interface to the system layer is based on a few low-level C functions. The application specific
 safety critical UI is implemented in Slint and Rust.

 The reference device used for developing the example is the Toradex NXP i.MX 95 Verdin https://www.toradex.com/computer-on-modules/verdin-arm-family/nxp-imx95-evaluation-kit#explore
 with NXP's SafeAssure framework.

The following video shows this demo in action, with Linux booting underneath a Slint based rectangular overlay.

The Linux based underlay starts the gallery demo, rendering with OpenGL on a Mali GPU with Skia and Slint's LinuxKMS backend.

https://github.com/user-attachments/assets/077790db-b325-49d2-9d10-1e1be7c5a660

The overlay is rendered on the Cortex-M7 running FreeRTOS and NXP's SafeAssure framework, to handle driving the Display Processing Unit (DPU) for blending, and to run the UI event loop.

## Project Layout

The safety scene uses [Slint SC](../../api/slint-sc/),
the safety-critical subset of Slint: `Window` and `Rectangle` only, and no `Timer` or model.
The UI and its logic are independent of the platform they run on:

- [`app/`](./app) — the scene ([`main.slint`](./app/main.slint)) and the event loop `app_main`.
  A backend implements the `Platform` trait (a clock, the display size, touch events, and an RGB8 framebuffer),
  and drives the UI by calling `app_main`.
  The once-per-second color cycle of the three telltales, which full Slint would express with a `Timer`,
  is written in Rust here.
- [`desktop/`](./desktop) — a desktop backend that shows the rendered frames in a Slint window and forwards its input,
  for running the example on a development machine.
- [`ffi/`](./ffi) — a backend over the C system interface, exposing `slint_app_main()` so C firmware can drive the UI.
  This is the safety-domain target.
- [`ffi-simulator/`](./ffi-simulator) — implements the C system interface in Rust
  and drives `ffi`'s `slint_app_main()`, so the C path can be exercised on a development machine.

## Supported Pixel Formats

The firmware backend ([`ffi/`](./ffi)) supports the following pixel formats via Cargo features:

- `pixel-bgra8888` - 32-bit BGRA, 8 bits per channel + alpha
- `pixel-rgb565` - 16-bit RGB, 5-6-5 bit distribution (memory efficient)
- `pixel-rgb888` - 24-bit RGB, 8 bits per channel

## Critical Section Implementation

The callback queue uses the [`critical-section`](https://crates.io/crates/critical-section) crate for ISR-safe access to the static queue. The actual critical section implementation depends on your target platform and is selected via the `SLINT_SAFEUI_CRITICAL_SECTION` CMake variable:

- `cs-cortex-m` (default) — Uses `cortex-m`'s single-core critical section (interrupt disable/enable via `PRIMASK`). Suitable for single-core Cortex-M MCUs.

On a host, the simulator pulls in the `critical-section` crate's built-in `std` implementation,
so it needs none of the above.
The desktop backend does not use the C queue at all.

## Building the Slint SC Compiler

The scene is compiled by the `slint-compiler` binary built with the `slint-sc` feature.
Build it once into the shared target directory before building the app:

```
cargo build -p slint-compiler --no-default-features --features slint-sc
```

The app's `build.rs` finds it there automatically;
a cross build uses a separate target directory,
so pass the host binary through the `SLINT_COMPILER` environment variable.

## Build System Integration

Integration of this example into an existing safety domain build system works by means of CMake. In your existing `CMakeLists.txt` for your target
that produces the final binary, use `FetchContent` to pull in the `SlintSafeUi` target:

```cmake
set(Rust_CARGO_TARGET "thumbv7em-none-eabihf" CACHE STRING "")

set(SLINT_SAFEUI_PANIC_HANDLER ON CACHE BOOL "" FORCE)
set(SLINT_SAFEUI_CRITICAL_SECTION "cs-cortex-m" CACHE STRING "" FORCE)
set(SLINT_SAFEUI_PIXEL_FORMAT "pixel-rgb565" CACHE STRING "" FORCE)
set(SLINT_SAFEUI_WIDTH "640" CACHE STRING "" FORCE)
set(SLINT_SAFEUI_HEIGHT "480" CACHE STRING "" FORCE)

include(FetchContent)
FetchContent_Declare(
    SlintSafeUi
    GIT_REPOSITORY https://github.com/slint-ui/slint.git
    GIT_TAG master
    SOURCE_SUBDIR examples/safe-ui
)
FetchContent_MakeAvailable(SlintSafeUi)
```

Link against it in your firmware target, to ensure linkage and access to the C system interface headers:

```cmake
target_link_libraries(my_firmware PRIVATE SlintSafeUi)
```

## C System Interface

The basic C system interface is documented in [./ffi/src/slint-safeui-platform-interface.h](./ffi/src/slint-safeui-platform-interface.h). This header file is also part of the `INTERFACE`
of the `SlintSafeUi` CMake target. Implement these functions in your firmware.

To run code on the Slint event loop thread from C firmware (including ISR context), use `slint_safeui_invoke_from_event_loop()`. This is ISR-safe: no heap allocation, no blocking, no FPU usage. It queues a function pointer and user data into a static queue under a critical section, then wakes the Slint event loop to execute the callback.

Input events (touch, keyboard, resize) are dispatched from C into the Rust/Slint event loop via the types and function declared in [./ffi/src/slint-safeui-event.h](./ffi/src/slint-safeui-event.h).
All coordinates are in physical pixels; the Rust conversion layer handles the physical-to-logical mapping using the configured scale factor.

Once you've started your UI task, invoke `slint_app_main()` to start the Slint event loop and the UI safety layer.

## Running on the Desktop

For convenience, the [`desktop/`](./desktop) backend runs the example on a development machine.
A Slint window displays the rendered frames and forwards its touch input,
while the safety UI runs on a worker thread:

```
cargo run --manifest-path examples/safe-ui/desktop/Cargo.toml
```

It implements the same `Platform` trait the firmware backend does,
so it exercises the identical UI and event loop without going through the C interface.

To exercise the C interface itself, run the [`ffi-simulator/`](./ffi-simulator) instead.
It implements the C system functions in Rust and drives `ffi`'s `slint_app_main()`,
selecting the pixel format with a Cargo feature:

```
cargo run --manifest-path examples/safe-ui/ffi-simulator/Cargo.toml --features pixel-bgra8888
```

## Known Limitations

- Partial rendering is not implemented. While this is technically possible, we aim to exclude the partial renderer from the safety certification process for now.
