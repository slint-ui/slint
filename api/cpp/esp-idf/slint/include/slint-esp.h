// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "slint-platform.h"
#include "esp_lcd_touch.h"
#include "esp_lcd_types.h"

/**
 * Initialize the Slint platform for ESP-IDF
 *
 * This must be called before any other call to the slint library.
 *
 * - `size` is the size of the screen
 * - `panel` is a handle to the display.
 * - `touch` is a handle to the touch screen, if the device has a touch screen
 * - `buffer1`, is a buffer of at least the size of the frame in which the slint scene
 *    will be drawn. Slint will take care to flush it to the screen
 * - `buffer2`, if specified, is a second buffer to be used with double buffering,
 *    both buffer1 and buffer2 should then be obtained with `esp_lcd_rgb_panel_get_frame_buffer`
 * - `rotation` applies a transformation while rendering in the buffer
 */
void slint_esp_init(slint::PhysicalSize size, esp_lcd_panel_handle_t panel,
                    std::optional<esp_lcd_touch_handle_t> touch,
                    std::span<slint::platform::Rgb565Pixel> buffer1,
                    std::optional<std::span<slint::platform::Rgb565Pixel>> buffer2 = {}
#ifdef SLINT_FEATURE_EXPERIMENTAL
                    ,
                    slint::platform::SoftwareRenderer::RenderingRotation rotation = {}
#endif
);

#ifdef SLINT_FEATURE_EXPERIMENTAL
/**
 * Same as the other overload but do rendering line-by-line, by allocating a line buffer with
 * MALLOC_CAP_INTERNAL, and flush it to the screen with esp_lcd_panel_draw_bitmap. (experimental)
 */
void slint_esp_init(slint::PhysicalSize size, esp_lcd_panel_handle_t panel,
                    std::optional<esp_lcd_touch_handle_t> touch,
                    slint::platform::SoftwareRenderer::RenderingRotation rotation = {});
#endif
