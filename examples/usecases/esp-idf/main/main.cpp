// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "slint-esp.h"
#include "app.h"
#include <ctime>
#include <memory>
#include "esp_log.h"

#include <slint-platform.h>

#include <bsp/display.h>
#include <bsp/esp-bsp.h>
#include <bsp/touch.h>
#include <vector>

#undef BSP_LCD_H_RES
#define BSP_LCD_H_RES 800
#undef BSP_LCD_V_RES
#define BSP_LCD_V_RES 1280

void init_virtual_keyboard(slint::ComponentHandle<App> app)
{
    app->global<VirtualKeyboardHandler>().on_key_pressed([=](auto key) {
        app->window().dispatch_key_press_event(key);
        app->window().dispatch_key_release_event(key);
    });
}

extern "C" void app_main(void)
{

    /* Initialize I2C (for touch and audio) */
    bsp_i2c_init();

    /* Initialize display  */
    esp_lcd_panel_io_handle_t io_handle = NULL;
    esp_lcd_panel_handle_t panel_handle = NULL;
    bsp_lcd_handles_t handles = { 0 };

    bsp_display_new_with_handles(nullptr, &handles);

    esp_lcd_touch_handle_t touch_handle = NULL;
    const bsp_touch_config_t bsp_touch_cfg = {};
    bsp_touch_new(&bsp_touch_cfg, &touch_handle);

    panel_handle = handles.panel;

    /* Set display brightness to 100% */
    bsp_display_backlight_on();

    slint_esp_init(slint::PhysicalSize({ BSP_LCD_H_RES, BSP_LCD_V_RES }), panel_handle,
                   touch_handle);

    auto app = App::create();
    init_virtual_keyboard(app);

    app->run();
}
