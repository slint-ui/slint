// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "launcher.h"

#include "slint-esp.h"
#include "esp_ota_ops.h"

#include <slint-platform.h>

#include <bsp/display.h>
#include <bsp/esp-bsp.h>
#include <bsp/touch.h>

#undef BSP_LCD_H_RES
#define BSP_LCD_H_RES 1024
#undef BSP_LCD_V_RES
#define BSP_LCD_V_RES 600

// copied from
// https://github.com/georgik/esp32-graphical-bootloader/blob/993df0fa6c498fcb3dfc463c13ccd4c1395f1e72/main/bootloader_ui.c#L373
static void ota_swich_to_app(int app_index)
{
    // Initially assume the first OTA partition, which is typically 'ota_0'
    const esp_partition_t *next_partition = esp_ota_get_next_update_partition(NULL);

    // Iterate to find the correct OTA partition only if button ID is greater than 1
    if (app_index > 0 && app_index <= 5) {
        for (int i = 0; i < app_index; i++) {
            next_partition = esp_ota_get_next_update_partition(next_partition);
            if (!next_partition)
                break; // If no next partition, break from the loop
        }
    }

    // For button 1, next_partition will not change, thus pointing to 'ota_0'
    if (next_partition && esp_ota_set_boot_partition(next_partition) == ESP_OK) {
        printf("Setting boot partition to %s\n", next_partition->label);
        esp_restart(); // Restart to boot from the new partition
    } else {
        printf("Failed to set boot partition\n");
    }
}

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

    bsp_display_backlight_on();

    slint_esp_init(SlintPlatformConfiguration {
            .size = slint::PhysicalSize({ BSP_LCD_H_RES, BSP_LCD_V_RES }),
            .panel_handle = handles.panel,
            .touch_handle = touch_handle });

    auto demo = Launcher::create();
    demo->on_launch(ota_swich_to_app);
    demo->run();
}
