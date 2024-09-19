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
3. General the CMake project and skeleton code.
4. In the STM32 VS Code Extension, choose the command to import a CMake project.
5. In STM32CubeMX select the STM32Cube BSP that matches your board and install it.
6. Copy the BSP drivers into your project's source tree and modify `CMakeLists.txt` to
   add them to the build.
7. Add C++ support to the generated CMake project by adding `CXX` to the `LANGUAGES` option
   of the `project` command.
8. Download and extract <https://github.com/slint-ui/slint/releases/latest/download/Slint-cpp-nightly-thumbv7em-none-eabihf.tar.gz>
9. Set `CMAKE_PREFIX_PATH` to the extracted directory.
10. Adjust your `CMakeLists.txt` for Slint use:
    1.  Add a `find_package(Slint)` call.
    2.  Enable resource embedding by adding `set(DEFAULT_SLINT_EMBED_RESOURCES embed-for-software-renderer)`.
    3.  Set the default style to `Fluent` with `set(SLINT_STYLE "fluent-light")`.
    4.  Add `SLINT_STM32_BSP_NAME=<name>` to your `target_compile_definitions` and replace `<name>` with the
        name of your BSP (for example `stm32h735g_discovery` or `stm32h747i_discovery`).
    5.  Add `Slint::Slint` to your `target_link_libraries`.
11. Add a C++ source file to your project, for example `appmain.cpp`, with an `appmain` function and call it
    from the generated `main.c`.
12. Add a `.slint` file to your project and include it in your project by calling `slint_target_sources`.
13. In your `appmain` function, initialize the screen via `BSP_LCD_InitEx` as well as the touch screen via `BSP_TS_Init`.
14. Include `#include <slint-stm32.h>` and call `slint_stm32_init(SlintPlatformConfiguration());` to initialize the
    Slint platform integration for STM32.
15. Finally, include the header file for your `.slint` file, instantiate the generated class, and invoke the event loop by
    calling `->run()` on the instance.



