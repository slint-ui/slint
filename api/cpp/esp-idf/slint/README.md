<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial -->
# Slint

[![Component Registry](https://components.espressif.com/components/slint/slint/badge.svg)](https://components.espressif.com/components/slint/slint)

Slint is a declarative GUI toolkit to build native user interfaces for desktop and embedded applications written in Rust, C++, or JavaScript.

This component provides the C++ version of [Slint](https://slint.dev/) for the [Espressif IoT Development Framework](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/index.html).

It has been tested on ESP32-S3 devices.

![Screenshot](https://user-images.githubusercontent.com/959326/260754861-e2130cce-9d2b-4925-9536-88293818ac3e.jpeg)

## Prerequisites

In order to compile Slint, you need a working [Rust toolchain for your device](https://esp-rs.github.io/book/installation/index.html).
Follow the instructions from <https://github.com/esp-rs/espup#installation> to set it up.

You can then either set the esp toolchain as the default toolchain with `rustup` or create a `rust-toolchain.toml` file in the root directory of your project with the following contents:

```toml
[toolchain]
channel = "esp"
```

If you see this error while compiling Slint:

```
error: the `-Z` flag is only accepted on the nightly channel of Cargo, but this is the `stable` channel
```

this means you are not using the right toolchain.


## Usage

By using this component, the `Slint::Slint` CMake target is linked to your application and you can access the entire functionality of the
[Slint C++ API](https://slint.dev/docs/cpp).

In addition, this component provides the `slint-esp.h` header file, which provides a a function to initialize the Slint backend
that must be called before setting up the UI.
Behind the scene, it will use `esp_lcd_panel_draw_bitmap` to render to the screen,
and is base on [ESP LCD Touch](https://components.espressif.com/components/espressif/esp_lcd_touch) for the touch events.

Use this platform implementation by instantiating it with a `esp_lcd_panel_handle_t`, an optional `esp_lcd_touch_handle_t`, and a pointer to one
or two `slint::platform::Rgb565Pixel` frame buffers. Next, register the instance with the Slint run-time library by calling `slint_esp_init()`:

```cpp
#include "slint-esp.h"
// ...


// Initialize the panel and touch handle
esp_lcd_panel_handle_t panel_handle = NULL;
bsp_display_new(&bsp_disp_cfg, &panel_handle, &io_handle);
bsp_display_backlight_on();
esp_lcd_touch_handle_t touch_handle = NULL;
bsp_touch_new(&bsp_touch_cfg, &touch_handle);

// Allocate a frame buffer
static std::vector<slint::platform::Rgb565Pixel> buffer(BSP_LCD_H_RES * BSP_LCD_V_RES);

// initialize the Slint ESP backend
slint_esp_init(slint::PhysicalSize({ BSP_LCD_H_RES, BSP_LCD_V_RES }), panel_handle,
                                   touch_handle, buffer);

```

Alternatively, you can implement your own sub-class of `slint::platform::Platform` to drive the screen and handle input events.

Next, integrate your `.slint` files into the build by compiling them to C++ and linking them to your component:

```cmake
slint_target_sources(${COMPONENT_LIB} my_application_ui.slint)
```

Instantiate the Slint component by including the generated header file (`my_application_ui.h` in the above example), call `create()`
on the generated class and `run()` to spin the event loop:

```cpp
#include "my_application_ui.h"

/// ...

auto my_application_ui = MainWindow::create();

my_application_ui->run();
```

## License

You can use Slint under ***any*** of the following licenses, at your choice:

1. [GNU GPLv3](https://github.com/slint-ui/slint/blob/master/LICENSES/GPL-3.0-only.txt),
2. [Paid license](https://slint.dev/pricing.html).

See also the [Licensing FAQ](https://github.com/slint-ui/slint/blob/master/FAQ.md#licensing).

Slint is also available with a third license (Royalty Free) for desktop applications.

## Links

[Website](https://slint.dev) · [GitHub](https://github.com/slint-ui/slint) · [Docs](https://slint.dev/docs/cpp)
