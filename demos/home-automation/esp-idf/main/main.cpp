// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "demo-sw-renderer.h"

#include "slint-esp.h"
#include <ctime>
#include <memory>

#include <slint-platform.h>

#include <bsp/display.h>
#include <bsp/esp-bsp.h>
#include <bsp/touch.h>
#include <vector>
#include "esp_ota_ops.h"
#include "esp_lcd_touch_gt911.h"
#include "nvs_flash.h"
#include "nvs.h"
#include "nvs_handle.hpp"

using RenderingRotation = slint::platform::SoftwareRenderer::RenderingRotation;

#undef BSP_LCD_H_RES
#define BSP_LCD_H_RES 1024
#undef BSP_LCD_V_RES
#define BSP_LCD_V_RES 600

#include "esp_ota_ops.h"

void reset_to_factory_app()
{
    // Get the partition structure for the factory partition
    const esp_partition_t *factory_partition = esp_partition_find_first(
            ESP_PARTITION_TYPE_APP, ESP_PARTITION_SUBTYPE_APP_FACTORY, NULL);
    if (factory_partition != NULL) {
        if (esp_ota_set_boot_partition(factory_partition) == ESP_OK) {
            printf("Set boot partition to factory, restarting now.\\n");
        } else {
            printf("Failed to set boot partition to factory.\\n");
        }
    } else {
        printf("Factory partition not found.\\n");
    }

    fflush(stdout);
}

RenderingRotation read_rotation()
{
    esp_err_t err = nvs_flash_init();
    if (err != ESP_OK) {
        return RenderingRotation::NoRotation;
    }

    auto handle = nvs::open_nvs_handle("slint", NVS_READONLY, &err);
    if (err != ESP_OK) {
        return RenderingRotation::NoRotation;
    }

    uint32_t rotation = 0;
    err = handle->get_item("rotation", rotation);
    if (err != ESP_OK) {
        return RenderingRotation::NoRotation;
    }
    return static_cast<RenderingRotation>(rotation);
}

extern "C" void app_main(void)
{
    RenderingRotation rotation = read_rotation();
    rotation = RenderingRotation::Rotate90;

    bool swap_xy =
            rotation == RenderingRotation::Rotate90 || rotation == RenderingRotation::Rotate270;

    reset_to_factory_app();

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
    if (rotation == RenderingRotation::NoRotation) {
        const bsp_touch_config_t bsp_touch_cfg = {};
        bsp_touch_new(&bsp_touch_cfg, &touch_handle);
    } else {
        const esp_lcd_touch_config_t tp_cfg = {
                .x_max = BSP_LCD_H_RES,
                .y_max = BSP_LCD_V_RES,
                .rst_gpio_num = BSP_LCD_TOUCH_RST, // Shared with LCD reset
                .int_gpio_num = BSP_LCD_TOUCH_INT,
                .levels = {
                        .reset = 0,
                        .interrupt = 0,
                },
                .flags = {
                .swap_xy = swap_xy,
#if CONFIG_BSP_LCD_TYPE_1024_600
                .mirror_x = rotation != RenderingRotation::Rotate90 && rotation != RenderingRotation::Rotate180,
                .mirror_y = rotation != RenderingRotation::Rotate270 && rotation != RenderingRotation::Rotate180,
#else
                .mirror_x = 0,
                .mirror_y = 0,
#endif
                },
        };
        auto i2c_handle = bsp_i2c_get_handle();
        esp_lcd_panel_io_handle_t tp_io_handle = nullptr;
        esp_lcd_panel_io_i2c_config_t tp_io_config = ESP_LCD_TOUCH_IO_I2C_GT911_CONFIG();
        tp_io_config.scl_speed_hz = CONFIG_BSP_I2C_CLK_SPEED_HZ;
        esp_lcd_new_panel_io_i2c(i2c_handle, &tp_io_config, &tp_io_handle);
        esp_lcd_touch_new_i2c_gt911(tp_io_handle, &tp_cfg, &touch_handle);
    }

    /* Set display brightness to 100% */
    bsp_display_backlight_on();

    slint_esp_init(SlintPlatformConfiguration {
        .size = swap_xy ? slint::PhysicalSize({ BSP_LCD_V_RES, BSP_LCD_H_RES })
                        : slint::PhysicalSize({ BSP_LCD_H_RES, BSP_LCD_V_RES }),
        .panel_handle = handles.panel, .touch_handle = touch_handle, .rotation = rotation,
#if CONFIG_BSP_LCD_COLOR_FORMAT_RGB888
        .byte_swap = true,
#endif
    });

    auto demo = AppWindow::create();
    demo->run();
}
