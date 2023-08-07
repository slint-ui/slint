// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#pragma once

#include "esp_lcd_touch.h"
#include "esp_lcd_types.h"
#include "slint-platform.h"

struct EspPlatform : public slint::platform::Platform
{
    EspPlatform(slint::PhysicalSize size, esp_lcd_panel_handle_t panel,
                std::optional<esp_lcd_touch_handle_t> touch,
                std::span<slint::platform::Rgb565Pixel> buffer1,
                std::optional<std::span<slint::platform::Rgb565Pixel>> buffer2 = {})
        : size(size), panel_handle(panel), touch_handle(touch), buffer1(buffer1), buffer2(buffer2)
    {
    }

    std::unique_ptr<slint::platform::WindowAdapter> create_window_adapter() override;

    std::chrono::milliseconds duration_since_start() const override;

    void run_event_loop() override;

private:
    slint::PhysicalSize size;
    esp_lcd_panel_handle_t panel_handle;
    std::optional<esp_lcd_touch_handle_t> touch_handle;
    std::span<slint::platform::Rgb565Pixel> buffer1;
    std::optional<std::span<slint::platform::Rgb565Pixel>> buffer2;
    class EspWindowAdapter *m_window = nullptr;
};
