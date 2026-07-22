// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "slint-esp.h"
#include "carousel_demo.h"
#include <ctime>
#include <memory>

#include <slint-platform.h>

#include <bsp/display.h>
#include <bsp/esp-bsp.h>

#if defined(BSP_LCD_DRAW_BUFF_SIZE)
#    define DRAW_BUF_SIZE BSP_LCD_DRAW_BUFF_SIZE
#else
#    define DRAW_BUF_SIZE (BSP_LCD_H_RES * CONFIG_BSP_LCD_DRAW_BUF_HEIGHT)
#endif

#if defined(EXAMPLE_TARGET_S3_BOX)
#    include <bsp/touch.h>
#endif
#include <vector>

extern "C" void app_main(void)
{
    /* Initialize I2C (for touch and audio) */
#if defined(EXAMPLE_TARGET_S3_BOX)
    bsp_i2c_init();
#endif

    /* Initialize display  */
    esp_lcd_panel_io_handle_t io_handle = NULL;
    esp_lcd_panel_handle_t panel_handle = NULL;
    const bsp_display_config_t bsp_disp_cfg = {
        .max_transfer_sz = DRAW_BUF_SIZE * sizeof(uint16_t),
    };
    bsp_display_new(&bsp_disp_cfg, &panel_handle, &io_handle);
    esp_lcd_touch_handle_t touch_handle = NULL;
#if defined(EXAMPLE_TARGET_S3_BOX)
    const bsp_touch_config_t bsp_touch_cfg = {};
    bsp_touch_new(&bsp_touch_cfg, &touch_handle);
#endif

    /* Set display brightness to 100% */
    bsp_display_backlight_on();

    static std::vector<slint::platform::Rgb565Pixel> buffer(BSP_LCD_H_RES * BSP_LCD_V_RES);

    slint_esp_init(SlintPlatformConfiguration {
            .size = slint::PhysicalSize({ BSP_LCD_H_RES, BSP_LCD_V_RES }),
            .panel_handle = panel_handle,
            .touch_handle = touch_handle,
            .buffer1 = buffer,
            .byte_swap = true });

    auto carousel_demo = MainWindow::create();

    carousel_demo->run();
}
