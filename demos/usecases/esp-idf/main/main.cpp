// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "../../cpp/main.cpp"

#include "slint-esp.h"
#include <ctime>
#include <memory>

#include <slint-platform.h>

#include <bsp/display.h>
#include <bsp/esp-bsp.h>
#include <bsp/touch.h>
#include <vector>

#undef BSP_LCD_H_RES
#define BSP_LCD_H_RES 1024
#undef BSP_LCD_V_RES
#define BSP_LCD_V_RES 600

extern "C" void app_main(void)
{

    /* Initialize I2C (for touch and audio) */
    bsp_i2c_init();

    /* Initialize display  */
    bsp_lcd_handles_t handles {};
    const bsp_display_config_t
            bsp_display_config = { .dsi_bus = {
                                           .lane_bit_rate_mbps = BSP_LCD_MIPI_DSI_LANE_BITRATE_MBPS,
                                   } };
    bsp_display_new_with_handles(&bsp_display_config, &handles);

    esp_lcd_touch_handle_t touch_handle = NULL;
    const bsp_touch_config_t bsp_touch_cfg = {};
    bsp_touch_new(&bsp_touch_cfg, &touch_handle);

    /* Set display brightness to 100% */
    bsp_display_backlight_on();

    slint_esp_init(SlintPlatformConfiguration {
            .size = slint::PhysicalSize({ BSP_LCD_H_RES, BSP_LCD_V_RES }),
            .panel_handle = handles.panel,
            .touch_handle = touch_handle });

    run();
}
