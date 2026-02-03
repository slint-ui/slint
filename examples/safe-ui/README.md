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

The overlay is rendered on the Cortex-M7 running FreeRTOS and NXP's SafeAssure framework, to handle driving the Display Processing Unit (DPU) for blending, and to run Slint's event loop.
The Slint scene rendered can be found in [./ui/app-window.slint](./ui/app-window.slint).
The application entry point is [./core/src/lib.rs](./core/src/lib.rs);

## Supported Pixel Formats

The SafeUI core supports the following pixel formats via Cargo features:

- `pixel-bgra8888` (default) - 32-bit BGRA, 8 bits per channel + alpha
- `pixel-rgb565` - 16-bit RGB, 5-6-5 bit distribution (memory efficient)
- `pixel-rgb888` - 24-bit RGB, 8 bits per channel

## Build System Integration

Integration of this example into an existing safety domain build system works by means of CMake. In your existing `CMakeLists.txt` for your target
that produces the final binary, use `FetchContent` to pull in the `SlintSafeUi` target:

```cmake
set(Rust_CARGO_TARGET "thumbv7em-none-eabihf" CACHE STRING "")

set(SLINT_SAFEUI_PANIC_HANDLER ON CACHE BOOL "" FORCE)
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

The basic C system interface is documented in [./core/src/slint-safeui-platform-interface.h](./core/src/slint-safeui-platform-interface.h). This header file is also part of the `INTERFACE`
of the `SlintSafeUi` CMake target. Implement these functions in your firmware.

Once you've started your UI task, invoke `slint_app_main()` to start the Slint event loop and the UI safety layer.

## Simulation

For convenience, this example provides a "simulator" binary target in [./simulator/src/main.rs](./simulator/src/main.rs), so that you can just run this on a desktop system passing the desired pixel format as cargo feature, e.g with

```
cargo run -p slint-safeui-simulator --features pixel-bgra8888
```

The "simulator" implements the same C system interface and runs the Slint UI safety layer example in a secondary thread.

## Known Limitations

- Partial rendering is not implemented. While this is technically possible, we aim to exclude the partial renderer from the safety certification process for now.
- The pixel format is hard-coded to BGRA8888. This is relatively easy to change, if necessary.
- `slint::invoke_from_event_loop()` (and `slint_safeui_platform_wake` in the interface) isn't fully implemented yet. This is partly due to missing abstractions
  (mutexes) as well as missing support to distinguish between waking up from an interrupt handler vs. being invoked from another task (`vTaskNotifyGiveFromISR()` vs `xTaskNotifyGive()`)
