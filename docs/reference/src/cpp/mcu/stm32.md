<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# STMicroelectronics' STM32Cube Ecosystem

Slint provides a platform integration with into STMicroelectronics' (STM) STM32Cube software platform.
It uses the `BSP_TS` APIs to retrieve touch input and uses the `BSP_LCD` and `HAL_LTDC` APIs to render
to the screen with double-buffering.

## Prerequisites

To build a C++ application with Slint for STM32 MCUs, install the following tools:

  * **[cmake](https://cmake.org/download/)** (3.21 or newer)
  * **[STM32CubeCLT](https://www.st.com/en/development-tools/stm32cubeclt.html)**
  * **[Visual Studio Code](https://code.visualstudio.com)**
  * **[Slint extension](https://marketplace.visualstudio.com/items?itemName=Slint.slint)**
  * **[STM32 VS Code Extension](https://marketplace.visualstudio.com/items?itemName=stmicroelectronics.stm32-vscode-extension)**
  * **[CMake Tools](https://marketplace.visualstudio.com/items?itemName=ms-vscode.cmake-tools)**

## First Steps

We provide templates for different STM32 Discovery Kits that provide:

 - A pre-configured build system.
 - Application skeleton source code with sample Slint UI.
 - Example usage of callbacks, properties, and basic widgets.

To get started, select a download from the following table. If your board is not included in the table below, see our [](stm32/generic.md) instructions.

| STM32 Board | Download |
|----------------------------------|----------|
| [STM32H747I-DISCO](https://www.st.com/en/evaluation-tools/stm32h747i-disco.html): Dual-core Arm M7/M4 MCU with 4” touch LCD display module | [slint-cpp-template-stm32h747i-disco.zip](https://github.com/slint-ui/slint/releases/latest/download/slint-cpp-template-stm32h747i-disco.zip) |
| [STM32H735G-DK](https://www.st.com/en/evaluation-tools/stm32h735g-dk.html): Arm M7 MCU with 4” touch LCD display module | [slint-cpp-template-stm32h735g-dk.zip](https://github.com/slint-ui/slint/releases/latest/download/slint-cpp-template-stm32h735g-dk.zip) |


1. Download and extract the archive that matches our STM32 Discovery Kit.
2. Open the extracted folder with VS Code.
3. Configure the project either via "CMake: Select Configure Preset" from the command palette or the CMake extension panel.
4. Build, Flash to Device, and debug by hitting `F5` or running the `CMake: Debug` command from the command palette.

## Next Steps

 - For more details about the Slint language, check out the [Slint Language Documentation](slint-reference:).
 - Learn about the [](../types.md) between Slint and C++.
 - Study the [](../api/library_root).

```{toctree}
:maxdepth: 2
:hidden:
:caption: STMicroelectronics' STM32Cube Ecosystem

stm32/generic.md
```
