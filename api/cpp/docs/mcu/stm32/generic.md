<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Generic Instructions for Slint on STM32 MCUs

The following instructions outline a rough, general path how to get started on an STM32 MCU
with STM32 Cube tooling and Slint. Successful completion requires experience with STM32CubeMX
as well as the peripherals of our board.

1. Make sure to install all the <project:../stm32.md#prerequisites>.
2. Start a new project with STM32CubeMX:
   - Select your base board.
   - Enable all peripherals needed. This includes LTDC and typically the FMC to be able
     to place the framebuffers in RAM.
   - Select CMake in the code generator options.
3. Generate the CMake project and skeleton code.
4. In the STM32 VS Code Extension, choose the command to import a CMake project.
5. In STM32CubeMX select the STM32Cube BSP that matches your board and install it.
6. Copy the BSP drivers into your project's source tree and modify `CMakeLists.txt` to
   add them to the build.
7. Add C++ support to the generated CMake project by adding `CXX` to the `LANGUAGES` option
   of the `project` command.
8. Open a web browser, navigate to <https://github.com/slint-ui/slint/releases/latest>, and download
   and extract `Slint-cpp-VERSION-thumbv7em-none-eabihf.tar.gz`, replace `VERSION` with the version you see.
10. Set `CMAKE_PREFIX_PATH` to the extracted directory.
11. Add a C++ source file to your project, for example `appmain.cpp`, with an `appmain` function and call it
    from the generated `main.c`.
12. Create `app-window.slint` with the following contents:
    ```slint,no-preview
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
13. Adjust your `CMakeLists.txt` for use of Slint. Copy the follow snippets and adjust the target names as needed:
    ```cmake
    # Locate Slint
    find_package(Slint)

    # Compile app-window.slint to app-window.h and app-window.cpp
    slint_target_sources(your-target app-window.slint)

    # Embed images and fonts in the binary
    set_target_properties(your-target PROPERTIES SLINT_EMBED_RESOURCES embed-for-software-renderer)

    # Replace $BSP_NAME with the name of your concrete BSP,
    # for example stm32h735g_discovery.
    target_compile_definitions(your-target PRIVATE
        SLINT_STM32_BSP_NAME=$BSP_NAME
    )

    # Link Slint run-time library
    target_link_libraries(your-target PRIVATE
        Slint::Slint
    )
    ```
14. In your `appmain` function, initialize the screen via `BSP_LCD_InitEx` as well as the touch screen via `BSP_TS_Init`,
    include `#include <slint-stm32.h>` and call `slint_stm32_init(SlintPlatformConfiguration());` to initialize the
    Slint platform integration for STM32. Finally, include the header file for your `.slint` file, instantiate the generated class, and invoke the event loop by calling `->run()` on the instance. Use the following example as reference:
    ```cpp
    #include <slint-stm32.h>
    #include <stdio.h>
    #include <stm32h735g_discovery.h>
    #include <stm32h735g_discovery_lcd.h>
    #include <stm32h735g_discovery_ts.h>

    #include "app-window.h"

    // Called from main()
    extern "C" void appmain() {
        if (BSP_LCD_InitEx(0, LCD_ORIENTATION_LANDSCAPE, LCD_PIXEL_FORMAT_RGB565,
                            LCD_DEFAULT_WIDTH, LCD_DEFAULT_HEIGHT) != 0) {
            Error_Handler();
        }

        BSP_LCD_DisplayOn(0);
        BSP_LCD_SetActiveLayer(0, 0);

        TS_Init_t hTS;
        hTS.Width = LCD_DEFAULT_WIDTH;
        hTS.Height = LCD_DEFAULT_HEIGHT;
        hTS.Orientation = TS_SWAP_XY;
        hTS.Accuracy = 0;
        /* Touchscreen initialization */
        if (BSP_TS_Init(0, &hTS) != 0) {
            Error_Handler();
        }

        slint_stm32_init(SlintPlatformConfiguration());

        auto app_window = AppWindow::create();

        app_window->run();

        return 0;
    }
    ```


