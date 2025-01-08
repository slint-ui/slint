<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Espressif's IoT Development Framework

Slint provides a [component](https://components.espressif.com/components/slint/slint) for the [Espressif IoT Development Framework](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/index.html).

It has been tested on ESP32-S3 devices.

## Prerequisites

* Install the [Espressif IoT Development Framework](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/index.html) and open a terminal or command prompt with the environment set up.
On Windows, follow the [Using the Command Prompt](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/get-started/windows-setup.html#using-the-command-prompt) instructions, on macOS and Linux, follow the
[Set up the Environment Variables](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/get-started/linux-macos-setup.html#step-4-set-up-the-environment-variables) instructions.

By default, Slint will use pre-compiled binaries. If for some reason there are no binaries available, the build will fall back to compiling Slint from source and you need to have [Rust installed](https://esp-rs.github.io/book/installation/rust.html installed) as well as the [Rust toolchains for Espressif SoCs with Xtensa and RISC-V targets](https://esp-rs.github.io/book/installation/riscv-and-xtensa.html).

## First Steps

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
5. Remove `main/slint-hello-world.c`.
6. Create a new file `main/slint-hello-world.cpp` with the following contents:
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

#include "app-window.h"

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
            .byte_swap = true });

    /* Instantiate the UI */
    auto ui = AppWindow::create();
    /* Show it on the screen and run the event loop */
    ui->run();
}
```
7. Create `main/app-window.slint` with the following contents:
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
8. Edit `main/CMakeLists.txt` to adjust for the new `slint-hello-world.cpp`, add `slint` as required component,
   and instruction the build system to compile `app-window.slint` to `app-window.h`. The file should look like this:
```cmake
idf_component_register(SRCS "slint-hello-world.cpp" INCLUDE_DIRS "." REQUIRES slint)
slint_target_sources(${COMPONENT_LIB} app-window.slint)
```
9. Open the configuration editor with `idf.py menuconfig`:
    * Change the stack size under `Component config --> ESP System Settings --> Main task stack size` to at least `8192`. You may need to tweak this value in the future if you run into stack overflows.
    * You may need additional device-specific settings. For example if your device has external SPI RAM,
       you may need to enable that. For details for ESP32-S3 based devices see how to [Configure the PSRAM](https://docs.espressif.com/projects/esp-idf/en/latest/esp32s3/api-guides/flash_psram_config.html#configure-the-psram).
    * Quit the editor with `Q` and save the configuration.

    Alternatively, check in a default sdkconfig tweaked from your board that adds the right amount of ram, flash, and use `CONFIG_MAIN_TASK_STACK_SIZE=8192`

10.  Build the project with `idf.py build`.
11.  Connect your device, then flash and run it with `idf.py flash monitor`.
12.  Observe Slint rendering "Hello World" on the screen 🎉.

Congratulations, you're all set up to develop with Slint.

## Next Steps

 - For more details about the Slint language, check out the [Slint Language Documentation](slint-reference:).
 - Learn about the [](../types.md) between Slint and C++.
 - Study the [](../api/library_root).

```{toctree}
:maxdepth: 2
:hidden:
:caption: Espressif's IoT Development Framework

esp-idf/troubleshoot.md
```
