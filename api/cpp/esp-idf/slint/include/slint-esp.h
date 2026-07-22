// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "slint-platform.h"
#include "esp_lcd_touch.h"
#include "esp_lcd_types.h"

/**
 * This data structure configures the Slint platform for use with ESP-IDF, in particular
 * the esp_lcd component (
 * https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/peripherals/lcd.html )
 * for touch input and on-screen rendering.
 *
 * Slint supports three different ways of rendering:
 *
 * * Single-buffering: Allocate one frame-buffer at a location of your choosing in RAM, and
 *                     set the `buffer1` field.
 * * Double-buffering: Call `esp_lcd_rgb_panel_get_frame_buffer` to obtain two frame buffers
 *                     allocated by the `esp_lcd` driver and set `buffer1` and `buffer2`.
 * * Line-by-line rendering: Set neither `buffer1` nor `buffer2` to instruct Slint to allocate
 *                           a buffer (with MALLOC_CAP_INTERNAL) big enough to hold one line,
 *                           render into it, and send it to the display.
 *
 *  Use single-buffering if you can allocate a buffer in a memory region that allows the esp_lcd
 *  driver to efficiently transfer to the display. Use double-buffering if your driver supports
 *  calling `esp_lcd_rgb_panel_get_frame_buffer` and the buffers can be accessed directly by the
 *  display controller. Use line-by-line rendering if you don't have sufficient memory or rendering
 *  to internal memory (MALLOC_CAP_INTERNAL) and flushing to the display is faster than rendering
 *  into memory buffers that may be slower to access for the CPU.
 *
 *  The data structure is a template where the pixel type is configurable.
 *  The default depends on the sdkconfig, but you can use either `slint::Rgb8Pixel` or
 *  `slint::platform::Rgb565Pixel`, depending on how the display is configured.
 */
template<typename PixelType =
#if CONFIG_BSP_LCD_COLOR_FORMAT_RGB888
                 slint::Rgb8Pixel
#else
                 slint::platform::Rgb565Pixel
#endif
         >

struct SlintPlatformConfiguration
{
    /// The size of the screen in pixels.
    slint::PhysicalSize size;
    /// The handle to the display as previously initialized by `bsp_display_new` or
    /// `esp_lcd_panel_init`. Must be set to a valid, non-null esp_lcd_panel_handle_t.
    esp_lcd_panel_handle_t panel_handle = nullptr;
    /// The touch screen handle, if the device is equipped with a touch screen. Set to nullptr
    /// otherwise;
    esp_lcd_touch_handle_t touch_handle = nullptr;
    /// The buffer Slint will render into. It must have have the size of at least one frame. Slint
    /// calls esp_lcd_panel_draw_bitmap to flush the buffer to the screen.
    std::optional<std::span<PixelType>> buffer1 = {};
    /// If specified, this is a second buffer that will be used for double-buffering. Use this if
    /// your LCD panel supports double buffering: Call `esp_lcd_rgb_panel_get_frame_buffer` to
    /// obtain two buffers and set `buffer` and `buffer2` in this data structure.
    std::optional<std::span<PixelType>> buffer2 = {};
    slint::platform::SoftwareRenderer::RenderingRotation rotation =
            slint::platform::SoftwareRenderer::RenderingRotation::NoRotation;
    /// Swap the 2 bytes of RGB 565 pixels before sending to the display, or turn 24-bit RGB into
    /// BGR. Use this if your CPU is little endian but the display expects big-endian.
    union {
        [[deprecated("Renamed to byte_swap")]] bool color_swap_16;
        bool byte_swap = false;
    };
};

template<typename... Args>
SlintPlatformConfiguration(Args...) -> SlintPlatformConfiguration<>;

/**
 * Initialize the Slint platform for ESP-IDF
 *
 * This must be called before any other call to the Slint library.
 *
 * - `size` is the size of the screen
 * - `panel` is a handle to the display.
 * - `touch` is a handle to the touch screen, if the device has a touch screen
 * - `buffer1`, is a buffer of at least the size of the frame in which the slint scene
 *    will be drawn. Slint will take care to flush it to the screen
 * - `buffer2`, if specified, is a second buffer to be used with double buffering,
 *    both buffer1 and buffer2 should then be obtained with `esp_lcd_rgb_panel_get_frame_buffer`
 *
 * Note: For compatibility, this function overload selects RGB16 byte swapping if single-buffering
 * is selected as rendering method.
 *
 *  \deprecated Prefer the overload taking a SlintPlatformConfiguration
 */
[[deprecated("Use the overload taking a SlintPlatformConfiguration")]]
void slint_esp_init(slint::PhysicalSize size, esp_lcd_panel_handle_t panel,
                    std::optional<esp_lcd_touch_handle_t> touch,
                    std::span<slint::platform::Rgb565Pixel> buffer1,
                    std::optional<std::span<slint::platform::Rgb565Pixel>> buffer2 = {});

/**
 * Initialize the Slint platform for ESP-IDF.
 *
 * This must be called before any other call to the Slint library.
 */
void slint_esp_init(const SlintPlatformConfiguration<slint::platform::Rgb565Pixel> &config);
void slint_esp_init(const SlintPlatformConfiguration<slint::Rgb8Pixel> &config);
