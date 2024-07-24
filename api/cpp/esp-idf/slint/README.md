<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Slint

[![Component Registry](https://components.espressif.com/components/slint/slint/badge.svg)](https://components.espressif.com/components/slint/slint)

Slint is a declarative GUI toolkit to build native user interfaces for desktop and embedded applications written in Rust, C++, or JavaScript.

This component provides the C++ version of [Slint](https://slint.dev/) for the [Espressif IoT Development Framework](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/index.html).

It has been tested on ESP32-S3 devices.

![Screenshot](https://user-images.githubusercontent.com/959326/260754861-e2130cce-9d2b-4925-9536-88293818ac3e.jpeg)

## Getting Started

### Prerequisites

* Install the [Espressif IoT Development Framework](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/index.html) and open a terminal or command prompt with the environment set up.
On Windows, follow the [Using the Command Prompt](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/get-started/windows-setup.html#using-the-command-prompt) instructions, on macOS and Linux, follow the
[Set up the Environment Variables](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/get-started/linux-macos-setup.html#step-4-set-up-the-environment-variables) instructions.
* Make sure that you have [Rust](https://esp-rs.github.io/book/installation/rust.html) installed.
* Install the Rust toolchains for [Espressif SoCs with Xtensa and RISC-V targets](https://esp-rs.github.io/book/installation/riscv-and-xtensa.html).

### Hello World

The following steps will guide from the a bare-bones esp-idf "hello_world" to a GUI with Slint.

1. Start by creating a new project:
```bash
idf.py create-project slint-hello-world
cd slint-hello-world
```
2. Select your chipset with `idf.py set-target`, for example if you're using an `ESP32S3` chipset, run
```bash
idf.py set-target esp32s3
```
3. Add a [Board Support Package](https://github.com/espressif/esp-bsp#esp-bsp-espressifs-board-support-packages) that matches your device as a dependency. For example, if you're using an ESP-BOX, run
```bash
idf.py add-dependency esp-box
```
4. Add Slint as a dependency:
```bash
idf.py add-dependency slint/slint
```
5. Ensure that Espressif's Rust toolchain is selected for building. Either set the `RUSTUP_TOOLCHAIN` environment variable to the value `esp` or create a file called `rust-toolchain.toml` in your project directory with the following contents:
```toml
[toolchain]
channel = "esp"
```
6. Remove `main/slint-hello-world.c`.
7. Create a new file `main/slint-hello-world.cpp` with the following contents:
```cpp
#include <stdio.h>
#include <esp_err.h>
#include <bsp/esp-bsp.h>
#include <bsp/touch.h>
#include <bsp/display.h>
#include <slint-esp.h>

#if defined(BSP_LCD_DRAW_BUFF_SIZE)
#    define DRAW_BUF_SIZE BSP_LCD_DRAW_BUFF_SIZE
#else
#    define DRAW_BUF_SIZE (BSP_LCD_H_RES * CONFIG_BSP_LCD_DRAW_BUF_HEIGHT)
#endif

#include "appwindow.h"

extern "C" void app_main(void)
{
    /* Initialize display  */
    esp_lcd_panel_io_handle_t io_handle = NULL;
    esp_lcd_panel_handle_t panel_handle = NULL;
    const bsp_display_config_t bsp_disp_cfg = {
        .max_transfer_sz = DRAW_BUF_SIZE * sizeof(uint16_t),
    };
    bsp_display_new(&bsp_disp_cfg, &panel_handle, &io_handle);

     /* Set display brightness to 100% */
    bsp_display_backlight_on();

    /* Initialize touch */
    esp_lcd_touch_handle_t touch_handle = NULL;
    const bsp_touch_config_t bsp_touch_cfg = {};
    bsp_touch_new(&bsp_touch_cfg, &touch_handle);

    /* Allocate a drawing buffer */
    static std::vector<slint::platform::Rgb565Pixel> buffer(BSP_LCD_H_RES * BSP_LCD_V_RES);

    /* Initialize Slint's ESP platform support*/
    slint_esp_init(SlintPlatformConfiguration {
            .size = slint::PhysicalSize({ BSP_LCD_H_RES, BSP_LCD_V_RES }),
            .panel_handle = panel_handle,
            .touch_handle = touch_handle,
            .buffer1 = buffer,
            .color_swap_16 = true });

    /* Instantiate the UI */
    auto ui = AppWindow::create();
    /* Show it on the screen and run the event loop */
    ui->run();
}
```
8. Create `main/appwindow.slint` with the following contents:
```
import { VerticalBox, AboutSlint } from "std-widgets.slint";
export component AppWindow inherits Window {
    VerticalBox {
        AboutSlint {}
        Text {
            text: "Hello World";
            font-size: 18px;
            horizontal-alignment: center;
        }
    }
}
```
9. Edit `main/CMakeLists.txt` to adjust for the new `slint-hello-world.cpp`, add `slint` as required component,
   and instruction the build system to compile `appwindow.slint` to `appwindow.h`. The file should look like this:
```cmake
idf_component_register(SRCS "slint-hello-world.cpp" INCLUDE_DIRS "." REQUIRES slint)
slint_target_sources(${COMPONENT_LIB} appwindow.slint)
```
10. Open the configuration editor with `idf.py menuconfig`:
    * Change the stack size under `Component config --> ESP System Settings --> Main task stack size` to at least `8192`. You may need to tweak this value in the future if you run into stack overflows.
    * You may need additional device-specific settings. For example if your device has external SPI RAM,
       you may need to enable that. For details for ESP32-S3 based devices see how to [Configure the PSRAM](https://docs.espressif.com/projects/esp-idf/en/latest/esp32s3/api-guides/flash_psram_config.html#configure-the-psram).
    * Quit the editor with `Q` and save the configuration.

    Alternatively, check in a default sdkconfig tweaked from your board that adds the right amount of ram, flash, and use `CONFIG_MAIN_TASK_STACK_SIZE=8192`

11.  Build the project with `idf.py build`.
12.  Connect your device, then flash and run it with `idf.py flash monitor`.
13.  Observe Slint rendering "Hello World" on the screen 🎉.

Congratulations, you're all set up to develop with Slint. For more information, check out our [online documentation](https://slint.dev/docs).

If you have feedback or questions, feel free to reach out to the Slint community:

-   [Chat with us](https://chat.slint.dev/) on Mattermost.
-   [Ask questions](https://github.com/slint-ui/slint/discussions) on GitHub
-   Contact us on [Twitter](https://twitter.com/slint_ui) or [Mastodon](https://fosstodon.org/@slint)
-   [Report a bug](https://github.com/slint-ui/slint/issues) on Github

## Troubleshooting

You may run into compile or run-time issues due to Slint's requirements. The following sections
track issues we're aware of and how to solve them.

### Rust Compilation Error During Slint Build

You see the following error:

```
error: the `-Z` flag is only accepted on the nightly channel of Cargo, but this is the `stable` channel
```

Solution: You need to configure your Rust toolchain to use the esp channel. Either set the `RUSTUP_TOOLCHAIN` environment variable to the value `esp` or create a file called `rust-toolchain.toml` in your project directory with the following contents:
```toml
[toolchain]
channel = "esp"
```

### Error compiling Slint for ESP-IDF on Windows

You see errors about `slint-compiler`:

```
Error copying file (if different) from "C:/path/build/./cargo/build/x86_64-pc-windows-msvc/release/slint-compiler" to "C:/path/build/_deps/slint-build".
```

Solution: At the moment development development on Windows while targeting ESP-IDF is not supported. We're working on a fix and in the meantime we recommend
using [Windows Subsystem for Linux](https://learn.microsoft.com/en-us/windows/wsl/).

### The device crashes at boot or enter a boot loop

One reason could be that you don't have enough ram for the heap or the stack.
Make sure that the stack is big enough (~8KiB), and that all the RAM was made available for the heap allocator.

### Wrong colors shown

If colors look inverted on your display, it may be an incompatibility between how RGB565 colors are ordered in little-endian
and your display expecting a different byte order. Typically, esp32 devices are little ending and display controllers often
expect big-endian or `esp_lcd` configures them accordingly. Therefore, by default Slint converts pixels to big-endian.
If your display controller expects little endian, set the `color_swap_16` field in `SlintPlatformConfiguration` to `false`.

## License

You can use Slint under ***any*** of the following licenses, at your choice:

1. [Royalty-free license](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md),
2. [GNU GPLv3](https://github.com/slint-ui/slint/blob/master/LICENSES/GPL-3.0-only.txt),
3. [Paid license](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Software-3.0.md).

See also the [Licensing FAQ](https://github.com/slint-ui/slint/blob/master/FAQ.md#licensing).

## Links

[Website](https://slint.dev) · [GitHub](https://github.com/slint-ui/slint) · [Docs](https://slint.dev/docs/cpp)
