// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#include "esp_slint.h"
#include <chrono>
#include <cstdint>
#include <type_traits>

#include "esp_lcd_panel_ops.h"
#include "esp_log.h"
#include "slint-platform.h"
#include "slint_size.h"

// The C code cause these warnings
#pragma GCC diagnostic ignored "-Wmissing-field-initializers"

static const char *TAG = "slint_platform";

using RepaintBufferType = slint::platform::SoftwareRenderer::RepaintBufferType;

class EspWindowAdapter : public slint::platform::WindowAdapter
{
public:
    slint::platform::SoftwareRenderer m_renderer;
    bool needs_redraw = true;
    const slint::PhysicalSize size;

    explicit EspWindowAdapter(RepaintBufferType buffer_type, slint::PhysicalSize size)
        : m_renderer(buffer_type), size(size)
    {
    }

    slint::platform::AbstractRenderer &renderer() override { return m_renderer; }

    slint::PhysicalSize physical_size() const override { return size; }

    void request_redraw() override { needs_redraw = true; }

    void set_visible(bool v) override
    {
        if (v) {
            window().dispatch_resize_event(
                    slint::LogicalSize({ float(size.width), float(size.height) }));
        }
    }
};

std::unique_ptr<slint::platform::WindowAdapter> EspPlatform::create_window_adapter()
{
    if (m_window != nullptr) {
        ESP_LOGI(TAG, "FATAL: create_window_adapter called multiple times");
        return nullptr;
    }

    auto buffer_type =
            buffer2 ? RepaintBufferType::SwappedBuffers : RepaintBufferType::ReusedBuffer;
    auto window = std::make_unique<EspWindowAdapter>(buffer_type, size);
    m_window = window.get();
    return window;
}

std::chrono::milliseconds EspPlatform::duration_since_start() const
{
    auto ticks = xTaskGetTickCount();
    return std::chrono::milliseconds(pdTICKS_TO_MS(ticks));
}

void EspPlatform::run_event_loop()
{

    constexpr UBaseType_t TouchNotificationIndex = 0;

    esp_lcd_panel_disp_on_off(panel_handle, true);

    if (touch_handle) {
        // Must be static since we can't pass user data to the interrupt callback
        static TaskHandle_t slint_platform_task;
        slint_platform_task = xTaskGetCurrentTaskHandle();
        esp_lcd_touch_register_interrupt_callback(*touch_handle, [](auto) {
            vTaskNotifyGiveIndexedFromISR(slint_platform_task, TouchNotificationIndex, nullptr);
        });
    }

    int last_touch_x = 0;
    int last_touch_y = 0;
    bool touch_down = false;
    bool has_touch_events = false;

    while (true) {
        slint::platform::update_timers_and_animations();

        if (m_window) {

            if (touch_handle && has_touch_events) {
                uint16_t touchpad_x[1] = { 0 };
                uint16_t touchpad_y[1] = { 0 };
                uint8_t touchpad_cnt = 0;

                /* Read touch controller data */
                esp_lcd_touch_read_data(*touch_handle);

                /* Get coordinates */
                bool touchpad_pressed = esp_lcd_touch_get_coordinates(
                        *touch_handle, touchpad_x, touchpad_y, NULL, &touchpad_cnt, 1);

                if (touchpad_pressed && touchpad_cnt > 0) {
                    // ESP_LOGI(TAG, "x: %i, y: %i", touchpad_x[0], touchpad_y[0]);
                    last_touch_x = touchpad_x[0];
                    last_touch_y = touchpad_y[0];
                    m_window->window().dispatch_pointer_move_event(
                            slint::LogicalPosition({ float(last_touch_x), float(last_touch_y) }));
                    if (!touch_down) {
                        m_window->window().dispatch_pointer_press_event(
                                slint::LogicalPosition(
                                        { float(last_touch_x), float(last_touch_y) }),
                                slint::PointerEventButton::Left);
                    }
                    touch_down = true;
                } else if (touch_down) {
                    m_window->window().dispatch_pointer_release_event(
                            slint::LogicalPosition({ float(last_touch_x), float(last_touch_y) }),
                            slint::PointerEventButton::Left);
                    m_window->window().dispatch_pointer_exit_event();
                    touch_down = false;
                }
            }

            if (std::exchange(m_window->needs_redraw, false)) {
                auto region = m_window->m_renderer.render(buffer1, size.width);
                auto o = region.bounding_box_origin();
                auto s = region.bounding_box_size();
                if (s.width > 0 && s.height > 0) {
                    if (buffer2) {
                        // Assuming that using double buffer means that the buffer comes from the
                        // driver and we need to pass the exact pointer.
                        // https://github.com/espressif/esp-idf/blob/53ff7d43dbff642d831a937b066ea0735a6aca24/components/esp_lcd/src/esp_lcd_panel_rgb.c#L681
                        esp_lcd_panel_draw_bitmap(panel_handle, 0, 0, size.width, size.height,
                                                  buffer1.data());
                        std::swap(buffer1, buffer2.value());
                        // FIXME: wait until swap finishes
                    } else {
                        for (int y = o.y; y < o.y + s.height; y++) {
                            for (int x = o.x; x < o.x + s.width; x++) {
                                // Swap endianess to big endian
                                auto px =
                                        reinterpret_cast<uint16_t *>(&buffer1[y * size.width + x]);
                                *px = (*px << 8) | (*px >> 8);
                            }
                            esp_lcd_panel_draw_bitmap(panel_handle, o.x, y, o.x + s.width, y + 1,
                                                      buffer1.data() + y * size.width + o.x);
                        }
                    }
                }
            }

            if (m_window->window().has_active_animations()) {
                continue;
            }
        }

        TickType_t ticks_to_wait;
        if (auto wait_time = slint::platform::duration_until_next_timer_update()) {
            ticks_to_wait = pdMS_TO_TICKS(wait_time->count());
        } else {
            ticks_to_wait = portMAX_DELAY;
        }

        has_touch_events = ulTaskNotifyTakeIndexed(TouchNotificationIndex, /*reset to zero*/ pdTRUE,
                                                   ticks_to_wait)
                > 0;
    }

    vTaskDelete(NULL);
}