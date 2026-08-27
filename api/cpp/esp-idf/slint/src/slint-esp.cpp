// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore LOGI rgbpix
#include <deque>
#include <mutex>
#include "slint-esp.h"
#include "slint-platform.h"
#include "esp_lcd_panel_ops.h"
#if __has_include("soc/soc_caps.h")
#    include "soc/soc_caps.h"
#endif
#if SOC_LCD_RGB_SUPPORTED && ESP_IDF_VERSION_MAJOR >= 5
#    include "esp_lcd_panel_rgb.h"
#endif
#if SOC_MIPI_DSI_SUPPORTED && ESP_IDF_VERSION_MAJOR >= 5
#    include "esp_lcd_mipi_dsi.h"
#endif
#include "esp_log.h"

static const char *TAG = "slint_platform";

using RepaintBufferType = slint::platform::SoftwareRenderer::RepaintBufferType;

class EspWindowAdapter : public slint::platform::WindowAdapter
{
public:
    slint::platform::SoftwareRenderer m_renderer;
    bool needs_redraw = true;
    const slint::PhysicalSize m_size;

    explicit EspWindowAdapter(RepaintBufferType buffer_type, slint::PhysicalSize size)
        : m_renderer(buffer_type), m_size(size)
    {
    }

    slint::platform::AbstractRenderer &renderer() override { return m_renderer; }

    slint::PhysicalSize size() override { return m_size; }

    void request_redraw() override { needs_redraw = true; }
};

template<typename PixelType>
struct EspPlatform : public slint::platform::Platform
{
    EspPlatform(const SlintPlatformConfiguration<PixelType> &config)
        : size(config.size),
          panel_handle(config.panel_handle),
          touch_handle(config.touch_handle),
          buffer1(config.buffer1),
          buffer2(config.buffer2),
          byte_swap(config.byte_swap),
          rotation(config.rotation)
    {
        task = xTaskGetCurrentTaskHandle();
    }

    std::unique_ptr<slint::platform::WindowAdapter> create_window_adapter() override;

    std::chrono::milliseconds duration_since_start() override;
    void run_event_loop() override;
    void quit_event_loop() override;
    void run_in_event_loop(Task) override;

private:
    slint::PhysicalSize size;
    esp_lcd_panel_handle_t panel_handle;
    esp_lcd_touch_handle_t touch_handle;
    std::optional<std::span<PixelType>> buffer1;
    std::optional<std::span<PixelType>> buffer2;
    bool byte_swap;
    slint::platform::SoftwareRenderer::RenderingRotation rotation;
    class EspWindowAdapter *m_window = nullptr;

    // Need to be static because we can't pass user data to the touch interrupt callback
    static TaskHandle_t task;
    std::mutex queue_mutex;
    std::deque<slint::platform::Platform::Task> queue; // protected by queue_mutex
    bool quit = false; // protected by queue_mutex
};

template<typename PixelType>
std::unique_ptr<slint::platform::WindowAdapter> EspPlatform<PixelType>::create_window_adapter()
{
    if (m_window != nullptr) {
        ESP_LOGI(TAG, "FATAL: create_window_adapter called multiple times");
        return nullptr;
    }

    auto buffer_type =
            buffer2 ? RepaintBufferType::SwappedBuffers : RepaintBufferType::ReusedBuffer;
    auto window = std::make_unique<EspWindowAdapter>(buffer_type, size);
    m_window = window.get();
    m_window->m_renderer.set_rendering_rotation(rotation);
    return window;
}

template<typename PixelType>
std::chrono::milliseconds EspPlatform<PixelType>::duration_since_start()
{
    auto ticks = xTaskGetTickCount();
    return std::chrono::milliseconds(pdTICKS_TO_MS(ticks));
}

#if SOC_LCD_RGB_SUPPORTED && ESP_IDF_VERSION_MAJOR >= 5
static SemaphoreHandle_t sem_vsync_end;
static SemaphoreHandle_t sem_gui_ready;

extern "C" bool on_vsync_event(esp_lcd_panel_handle_t panel,
                               const esp_lcd_rgb_panel_event_data_t *edata, void *)
{
    BaseType_t high_task_awoken = pdFALSE;
    if (xSemaphoreTakeFromISR(sem_gui_ready, &high_task_awoken) == pdTRUE) {
        xSemaphoreGiveFromISR(sem_vsync_end, &high_task_awoken);
    }
    return high_task_awoken == pdTRUE;
}
#endif

#if SOC_MIPI_DSI_SUPPORTED && ESP_IDF_VERSION_MAJOR >= 5
// Synchronization for MIPI-DSI DPI panels, whose operations are asynchronous:
//  - sem_dpi_draw_done serializes esp_lcd_panel_draw_bitmap. The draw is async and only one
//    transfer may be in flight at a time; issuing a new draw before the previous one finished
//    trips the driver's "previous draw operation is not finished" error. It starts "available"
//    (given), is taken before each draw_bitmap, and is given back by on_color_trans_done.
//  - sem_dpi_refresh is given by on_refresh_done at each frame boundary, letting the event loop
//    pace animations to the panel's refresh rate instead of busy-looping (which would saturate
//    the CPU and starve the idle task, tripping the task watchdog). It starts empty.
static SemaphoreHandle_t sem_dpi_draw_done;
static SemaphoreHandle_t sem_dpi_refresh;

// Give a semaphore from either ISR or task context: on_color_trans_done runs in an ISR for the
// DMA2D copy path but inline (task context) for the no-copy and CPU-copy paths, while
// on_refresh_done always runs in an ISR.
static bool dpi_give_from_any_context(SemaphoreHandle_t sem)
{
    if (xPortInIsrContext()) {
        BaseType_t high_task_awoken = pdFALSE;
        xSemaphoreGiveFromISR(sem, &high_task_awoken);
        return high_task_awoken == pdTRUE;
    }
    xSemaphoreGive(sem);
    return false;
}

extern "C" bool on_dpi_color_trans_done(esp_lcd_panel_handle_t, esp_lcd_dpi_panel_event_data_t *,
                                        void *)
{
    return dpi_give_from_any_context(sem_dpi_draw_done);
}

extern "C" bool on_dpi_refresh_done(esp_lcd_panel_handle_t, esp_lcd_dpi_panel_event_data_t *,
                                    void *)
{
    return dpi_give_from_any_context(sem_dpi_refresh);
}
#endif

namespace {
void byte_swap_color(slint::platform::Rgb565Pixel *pixel)
{
    // Swap endianness to big endian
    auto px = reinterpret_cast<uint16_t *>(pixel);
    *px = (*px << 8) | (*px >> 8);
}
void byte_swap_color(slint::Rgb8Pixel *pixel)
{
    std::swap(pixel->r, pixel->b);
}
}

template<typename PixelType>
void EspPlatform<PixelType>::run_event_loop()
{
    esp_lcd_panel_disp_on_off(panel_handle, true);

    TickType_t max_ticks_to_wait = portMAX_DELAY;

    if (touch_handle) {
        if (esp_lcd_touch_register_interrupt_callback(
                    touch_handle, [](auto) { vTaskNotifyGiveFromISR(task, nullptr); })
            != ESP_OK) {

            // No touch interrupt assigned or supported? Fall back to polling like esp_lvgl_port.
            // LVGL polls in 5ms intervals, but FreeRTOS tick interval is 10ms, so go for that
            max_ticks_to_wait = pdMS_TO_TICKS(10);
        }
    }
#if SOC_LCD_RGB_SUPPORTED && ESP_IDF_VERSION_MAJOR >= 5
    if (buffer2) {
        sem_vsync_end = xSemaphoreCreateBinary();
        sem_gui_ready = xSemaphoreCreateBinary();
        esp_lcd_rgb_panel_event_callbacks_t cbs = {};
        cbs.on_vsync = on_vsync_event;
        esp_lcd_rgb_panel_register_event_callbacks(panel_handle, &cbs, this);
    }
#endif

#if SOC_MIPI_DSI_SUPPORTED && ESP_IDF_VERSION_MAJOR >= 5
    // Assumes panel_handle is a MIPI-DSI DPI panel (the case on e.g. ESP32-P4). Register the
    // completion callbacks that drive the draw and refresh synchronization described above.
    // sem_dpi_draw_done starts "available" so the first draw isn't blocked.
    sem_dpi_draw_done = xSemaphoreCreateBinary();
    xSemaphoreGive(sem_dpi_draw_done);
    sem_dpi_refresh = xSemaphoreCreateBinary();
    esp_lcd_dpi_panel_event_callbacks_t dpi_cbs = {};
    dpi_cbs.on_color_trans_done = on_dpi_color_trans_done;
    dpi_cbs.on_refresh_done = on_dpi_refresh_done;
    if (esp_lcd_dpi_panel_register_event_callbacks(panel_handle, &dpi_cbs, nullptr) != ESP_OK) {
        // Not a DPI panel (or callbacks unsupported): fall back to draws without synchronization.
        vSemaphoreDelete(sem_dpi_draw_done);
        sem_dpi_draw_done = nullptr;
        vSemaphoreDelete(sem_dpi_refresh);
        sem_dpi_refresh = nullptr;
    }
#endif

    // Issues esp_lcd_panel_draw_bitmap, first waiting for any previous asynchronous DPI
    // transfer to complete so we never overrun the panel's single in-flight draw operation.
    auto draw_bitmap = [this](int x_start, int y_start, int x_end, int y_end,
                              const void *color_data) {
#if SOC_MIPI_DSI_SUPPORTED && ESP_IDF_VERSION_MAJOR >= 5
        if (sem_dpi_draw_done) {
            xSemaphoreTake(sem_dpi_draw_done, portMAX_DELAY);
        }
#endif
        esp_lcd_panel_draw_bitmap(panel_handle, x_start, y_start, x_end, y_end, color_data);
    };

    float last_touch_x = 0;
    float last_touch_y = 0;
    bool touch_down = false;

    while (true) {
        slint::platform::update_timers_and_animations();

        std::optional<slint::platform::Platform::Task> event;
        {
            std::unique_lock lock(queue_mutex);
            if (queue.empty()) {
                if (quit) {
                    quit = false;
                    break;
                }
            } else {
                event = std::move(queue.front());
                queue.pop_front();
            }
        }
        if (event) {
            std::move(*event).run();
            event.reset();
            continue;
        }

        if (m_window) {

            if (touch_handle) {
                uint16_t touchpad_x[1] = { 0 };
                uint16_t touchpad_y[1] = { 0 };
                uint8_t touchpad_cnt = 0;

                /* Read touch controller data */
                esp_lcd_touch_read_data(touch_handle);

                /* Get coordinates */
                bool touchpad_pressed = esp_lcd_touch_get_coordinates(
                        touch_handle, touchpad_x, touchpad_y, NULL, &touchpad_cnt, 1);

                if (touchpad_pressed && touchpad_cnt > 0) {
                    auto scale_factor = m_window->window().scale_factor();
                    // ESP_LOGI(TAG, "x: %i, y: %i", touchpad_x[0], touchpad_y[0]);
                    last_touch_x = float(touchpad_x[0]) / scale_factor;
                    last_touch_y = float(touchpad_y[0]) / scale_factor;
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
            }

            if (std::exchange(m_window->needs_redraw, false)) {
                using slint::platform::SoftwareRenderer;
                auto rotated = rotation == SoftwareRenderer::RenderingRotation::Rotate90
                        || rotation == SoftwareRenderer::RenderingRotation::Rotate270;
                auto stride = rotated ? size.height : size.width;
                if (buffer1) {
#if SOC_LCD_RGB_SUPPORTED && ESP_IDF_VERSION_MAJOR >= 5
                    if (buffer2) {
                        xSemaphoreGive(sem_gui_ready);
                        xSemaphoreTake(sem_vsync_end, portMAX_DELAY);
                    }
#endif
                    auto region = m_window->m_renderer.render(buffer1.value(), stride);

                    if (byte_swap) {
                        for (auto [o, s] : region.rectangles()) {
                            for (int y = o.y; y < o.y + s.height; y++) {
                                for (int x = o.x; x < o.x + s.width; x++) {
                                    byte_swap_color(&buffer1.value()[y * stride + x]);
                                }
                            }
                        }
                    }

                    if (buffer2) {
                        auto s = region.bounding_box_size();
                        if (s.width > 0 && s.height > 0) {
                            // Assuming that using double buffer means that the buffer comes from
                            // the driver and we need to pass the exact pointer.
                            // https://github.com/espressif/esp-idf/blob/53ff7d43dbff642d831a937b066ea0735a6aca24/components/esp_lcd/src/esp_lcd_panel_rgb.c#L681
                            draw_bitmap(0, 0, size.width, size.height, buffer1->data());

                            std::swap(buffer1, buffer2);
                        }
                    } else {
                        for (auto [o, s] : region.rectangles()) {
                            for (int y = o.y; y < o.y + s.height; y++) {
                                draw_bitmap(o.x, y, o.x + s.width, y + 1,
                                            buffer1->data() + y * stride + o.x);
                            }
                        }
                    }
                } else {
                    // esp_lcd_panel_draw_bitmap is "async" so we have two buffers, one in which we
                    // render, and one which is being transmitted with a DMA transfer in parallel.
                    // On MIPI-DSI DPI panels draw_bitmap() above waits for the previous transfer to
                    // finish before starting the next one, and we wait once more below before the
                    // line buffers are freed. Other asynchronous panels (e.g. SPI/i80) are not yet
                    // synchronized here, so a transfer may still be in progress past these calls.
                    using Uniq = std::unique_ptr<PixelType, void (*)(void *)>;
                    auto alloc = [&] {
                        void *ptr = heap_caps_malloc(stride * sizeof(PixelType),
                                                     MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT);
                        if (!ptr) {
                            ESP_LOGE(TAG, "malloc failed to allocate line buffer");
                            abort();
                        }
                        return Uniq(reinterpret_cast<PixelType *>(ptr), heap_caps_free);
                    };
                    Uniq lb[2] = { alloc(), alloc() };
                    int idx = 0;
                    m_window->m_renderer.render_by_line<PixelType>(
                            [this, &lb, &idx,
                             &draw_bitmap](std::size_t line_y, std::size_t line_start,
                                           std::size_t line_end, auto &&render_fn) {
                                std::span<PixelType> view { lb[idx].get(), line_end - line_start };
                                render_fn(view);
                                if (byte_swap) {
                                    // Swap endianness to big endian
                                    std::for_each(view.begin(), view.end(),
                                                  [](auto &rgbpix) { byte_swap_color(&rgbpix); });
                                }
                                draw_bitmap(line_start, line_y, line_end, line_y + 1, view.data());
                                idx = (idx + 1) % 2;
                            });
#if SOC_MIPI_DSI_SUPPORTED && ESP_IDF_VERSION_MAJOR >= 5
                    // Wait for the last line's transfer to finish before the line buffers are
                    // freed, then leave the semaphore available for the next frame's first draw.
                    if (sem_dpi_draw_done) {
                        xSemaphoreTake(sem_dpi_draw_done, portMAX_DELAY);
                        xSemaphoreGive(sem_dpi_draw_done);
                    }
#endif
                }
            }

            if (m_window->window().has_active_animations()) {
#if SOC_MIPI_DSI_SUPPORTED && ESP_IDF_VERSION_MAJOR >= 5
                // Pace continuous animations to the panel's refresh so the GUI task blocks
                // (letting the idle task run) between frames instead of saturating the CPU and
                // tripping the task watchdog. Drain a refresh that may have arrived while we were
                // rendering, then wait for the next one. The timeout bounds the wait so a panel
                // that stops signaling refresh degrades to a slow loop rather than hanging.
                if (sem_dpi_refresh) {
                    xSemaphoreTake(sem_dpi_refresh, 0);
                    xSemaphoreTake(sem_dpi_refresh, pdMS_TO_TICKS(100));
                }
#endif
                continue;
            }
        }

        TickType_t ticks_to_wait = max_ticks_to_wait;
        if (auto wait_time = slint::platform::duration_until_next_timer_update()) {
            ticks_to_wait = std::min(ticks_to_wait, pdMS_TO_TICKS(wait_time->count()));
        }

        ulTaskNotifyTake(/*reset to zero*/ pdTRUE, ticks_to_wait);
    }

    vTaskDelete(NULL);
}

template<typename PixelType>
void EspPlatform<PixelType>::quit_event_loop()
{
    {
        const std::unique_lock lock(queue_mutex);
        quit = true;
    }
    vTaskNotifyGiveFromISR(task, nullptr);
}

template<typename PixelType>
void EspPlatform<PixelType>::run_in_event_loop(slint::platform::Platform::Task event)
{
    {
        const std::unique_lock lock(queue_mutex);
        queue.push_back(std::move(event));
    }
    vTaskNotifyGiveFromISR(task, nullptr);
}

template<typename PixelType>
TaskHandle_t EspPlatform<PixelType>::task = {};

void slint_esp_init(slint::PhysicalSize size, esp_lcd_panel_handle_t panel,
                    std::optional<esp_lcd_touch_handle_t> touch,
                    std::span<slint::platform::Rgb565Pixel> buffer1,
                    std::optional<std::span<slint::platform::Rgb565Pixel>> buffer2)
{

    SlintPlatformConfiguration<slint::platform::Rgb565Pixel> config {
        .size = size,
        .panel_handle = panel,
        .touch_handle = touch ? *touch : nullptr,
        .buffer1 = buffer1,
        .buffer2 = buffer2,
        // For compatibility with earlier versions of Slint, we compute the value of
        // byte_swap the way it was implemented in Slint (slint-esp) <= 1.6.0:
        .byte_swap = !buffer2.has_value()
    };
    slint_esp_init(config);
}

void slint_esp_init(const SlintPlatformConfiguration<slint::platform::Rgb565Pixel> &config)
{
    slint::platform::set_platform(
            std::make_unique<EspPlatform<slint::platform::Rgb565Pixel>>(config));
}

void slint_esp_init(const SlintPlatformConfiguration<slint::Rgb8Pixel> &config)
{
    slint::platform::set_platform(std::make_unique<EspPlatform<slint::Rgb8Pixel>>(config));
}
