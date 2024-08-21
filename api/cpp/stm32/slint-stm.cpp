// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#include <cstdint>
#include <memory>
#include "slint-stm.h"
#include <type_traits>

using Pixel = slint::platform::Rgb565Pixel;

static __IO bool screen_ready = true;

struct StmWindowAdapter : public slint::platform::WindowAdapter
{
    slint::platform::SoftwareRenderer m_renderer {
        slint::platform::SoftwareRenderer::RepaintBufferType::SwappedBuffers
    };
    bool needs_redraw = true;
    const slint::PhysicalSize m_size;

    explicit StmWindowAdapter(slint::PhysicalSize size) : m_size(size) { }

    slint::platform::AbstractRenderer &renderer() override { return m_renderer; }

    slint::PhysicalSize size() override { return m_size; }

    void request_redraw() override { needs_redraw = true; }
};

struct StmSlintPlatform : public slint::platform::Platform
{

    StmWindowAdapter *m_window = nullptr;
    const slint::PhysicalSize size;
    std::span<Pixel> buffer1;
    std::span<Pixel> buffer2;

    StmSlintPlatform(slint::PhysicalSize size, std::span<Pixel> buffer1, std::span<Pixel> buffer2)
        : size(size), buffer1(buffer1), buffer2(buffer2)
    {
        BSP_LCD_LayerConfig_t config;
        config.X0 = 0;
        config.X1 = LCD_DEFAULT_WIDTH;
        config.Y0 = 0;
        config.Y1 = LCD_DEFAULT_HEIGHT;
        config.PixelFormat = LCD_PIXEL_FORMAT_RGB565;
        config.Address = uintptr_t(buffer1.data());
        BSP_LCD_ConfigLayer(0, 0, &config);
        HAL_LTDC_RegisterCallback(&hlcd_ltdc, HAL_LTDC_RELOAD_EVENT_CB_ID,
                                  [](auto *) { screen_ready = true; });
    }

    std::unique_ptr<slint::platform::WindowAdapter> create_window_adapter() override
    {
        auto w = std::make_unique<StmWindowAdapter>(size);
        m_window = w.get();
        return w;
    }

    std::chrono::milliseconds duration_since_start() override
    {
        return std::chrono::milliseconds(HAL_GetTick());
    }

    void run_event_loop() override
    {

        float last_touch_x = 0;
        float last_touch_y = 0;
        bool touch_down = false;

        while (true) {
            slint::platform::update_timers_and_animations();

            if (m_window) {
                TS_State_t TS_State {};
                BSP_TS_GetState(0, &TS_State);
                if (TS_State.TouchDetected) {
                    auto scale_factor = m_window->window().scale_factor();
                    last_touch_x = float(TS_State.TouchX) / scale_factor;
                    last_touch_y = float(TS_State.TouchY) / scale_factor;

                    m_window->window().dispatch_pointer_move_event(
                            slint::LogicalPosition({ last_touch_x, last_touch_y }));
                    if (!touch_down) {
                        m_window->window().dispatch_pointer_press_event(
                                slint::LogicalPosition({ last_touch_x, last_touch_y }),
                                slint::PointerEventButton::Left);
                    }
                    touch_down = true;
                } else if (touch_down) {
                    m_window->window().dispatch_pointer_release_event(
                            slint::LogicalPosition({ last_touch_x, last_touch_y }),
                            slint::PointerEventButton::Left);
                    m_window->window().dispatch_pointer_exit_event();
                    touch_down = false;
                }

                if (std::exchange(m_window->needs_redraw, false)) {
                    while (!screen_ready) { }

                    m_window->m_renderer.render(buffer1, m_window->m_size.width);

                    SCB_CleanDCache_by_Addr((uint32_t *)buffer1.data(), buffer1.size());

                    BSP_LCD_Relaod(0, BSP_LCD_RELOAD_NONE);
                    BSP_LCD_SetLayerAddress(0, 0, uintptr_t(buffer1.data()));
                    screen_ready = false;
                    BSP_LCD_Relaod(0, BSP_LCD_RELOAD_VERTICAL_BLANKING);

                    std::swap(buffer1, buffer2);
                }
            }
        }
    }
};

void slint_stm_init(const SlintPlatformConfiguration &config)
{
    auto a = config.size.width * config.size.height;
    std::span<Pixel> buffer1(reinterpret_cast<Pixel *>(config.lcd_layer_0_address), a);
    std::span<Pixel> buffer2(reinterpret_cast<Pixel *>(config.lcd_layer_1_address), a);

    slint::platform::set_platform(
            std::make_unique<StmSlintPlatform>(config.size, buffer1, buffer2));
}
