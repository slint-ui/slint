// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "demo-sw-renderer.h"

#include <chrono>
#include <ctime>

#include <esp_check.h>
#include <esp_lcd_panel_ops.h>
#include <slint-esp.h>

#include <bsp/display.h>
#include <bsp/esp-bsp.h>
#include <bsp/touch.h>
#include <esp_lcd_touch_gt911.h>

namespace {

void update_clock_if_valid(const AppWindow &ui)
{
    std::time_t now = std::chrono::system_clock::to_time_t(std::chrono::system_clock::now());
    std::tm local_time {};

    if (!localtime_r(&now, &local_time)) {
        return;
    }

    auto year = local_time.tm_year + 1900;
    if (year < 2024) {
        return;
    }

    auto &api = ui.global<Api>();
    api.set_current_date(Date {
            .year = year,
            .month = local_time.tm_mon + 1,
            .day = local_time.tm_mday,
    });
    api.set_current_time(Time {
            .hour = local_time.tm_hour,
            .minute = local_time.tm_min,
            .second = local_time.tm_sec,
    });
}

} // namespace

extern "C" void app_main(void)
{
    ESP_ERROR_CHECK(bsp_i2c_init());

    bsp_lcd_handles_t display_handles {};
    bsp_display_config_t display_config {};
    display_config.hdmi_resolution = BSP_HDMI_RES_NONE;
    display_config.dsi_bus.lane_bit_rate_mbps = BSP_LCD_MIPI_DSI_LANE_BITRATE_MBPS;
    ESP_ERROR_CHECK(bsp_display_new_with_handles(&display_config, &display_handles));
    ESP_ERROR_CHECK(bsp_display_backlight_on());

    esp_lcd_touch_handle_t touch_handle = nullptr;
    const esp_lcd_touch_config_t tp_cfg = {
        .x_max = BSP_LCD_H_RES,
        .y_max = BSP_LCD_V_RES,
        .rst_gpio_num = BSP_LCD_TOUCH_RST,
        .int_gpio_num = BSP_LCD_TOUCH_INT,
        .levels = { .reset = 0, .interrupt = 0 },
        .flags = { .swap_xy = true, .mirror_x = false, .mirror_y = true },
    };
    auto i2c_handle = bsp_i2c_get_handle();
    esp_lcd_panel_io_handle_t tp_io_handle = nullptr;
    esp_lcd_panel_io_i2c_config_t tp_io_config = ESP_LCD_TOUCH_IO_I2C_GT911_CONFIG();
    tp_io_config.scl_speed_hz = CONFIG_BSP_I2C_CLK_SPEED_HZ;
    ESP_ERROR_CHECK(esp_lcd_new_panel_io_i2c(i2c_handle, &tp_io_config, &tp_io_handle));
    ESP_ERROR_CHECK(esp_lcd_touch_new_i2c_gt911(tp_io_handle, &tp_cfg, &touch_handle));

    slint_esp_init(SlintPlatformConfiguration {
            .size = slint::PhysicalSize({ BSP_LCD_V_RES, BSP_LCD_H_RES }),
            .panel_handle = display_handles.panel,
            .touch_handle = touch_handle,
            .rotation = slint::platform::SoftwareRenderer::RenderingRotation::Rotate90,
            .byte_swap = false,
    });

    auto ui = AppWindow::create();
    update_clock_if_valid(*ui);

    slint::Timer clock_update_timer(std::chrono::seconds(1),
                                    [ui_weak = slint::ComponentWeakHandle(ui)]() {
                                        if (auto ui = ui_weak.lock()) {
                                            update_clock_if_valid(**ui);
                                        }
                                    });

    ui->run();
}
