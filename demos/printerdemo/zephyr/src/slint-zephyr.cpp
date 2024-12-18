// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "slint-zephyr.h"

#include <slint-platform.h>

#include <zephyr/logging/log.h>
LOG_MODULE_REGISTER(zephyrSlint, LOG_LEVEL_DBG);

#include <zephyr/kernel.h>
#include <zephyr/drivers/display.h>
#include <zephyr/input/input.h>

#include <chrono>
#include <deque>
#include <ranges>

namespace {
bool is_supported_pixel_format(display_pixel_format current_pixel_format)
{
    switch (current_pixel_format) {
    case PIXEL_FORMAT_RGB_565:
        return true;
    case PIXEL_FORMAT_RGB_888:
        // Slint supports this format, but it uses more space.
        return false;
    case PIXEL_FORMAT_BGR_565:
#ifdef CONFIG_SHIELD_RK055HDMIPI4MA0
        // Zephyr expects pixel data to be big endian [1].

        // The display driver expects RGB 565 pixel data [2], and appears to expect it to be little
        // endian.

        // By passing Slint's little endian, RGB 565 pixel data without converting to big endian as
        // Zephyr expects, we get colors that work.

        // [1]
        // https://docs.zephyrproject.org/latest/hardware/peripherals/display/index.html#c.display_pixel_format
        // [2]
        // https://github.com/zephyrproject-rtos/zephyr/blob/c211cb347e0af0a4931e0e7af3d93577bcc7af8f/drivers/display/display_mcux_elcdif.c#L256

        // See also:
        // https://github.com/zephyrproject-rtos/zephyr/issues/53642
        return true;
#else
        return false;
#endif
        return false;
    case PIXEL_FORMAT_MONO01:
    case PIXEL_FORMAT_MONO10:
    case PIXEL_FORMAT_ARGB_8888:
        return false;
    }
    assert(false);
}

struct k_unique_lock
{
    k_unique_lock(struct k_mutex *m) : mutex(m) { k_mutex_lock(mutex, K_FOREVER); }
    ~k_unique_lock() { k_mutex_unlock(mutex); }
    struct k_mutex *mutex = nullptr;
};

struct RotationInfo
{
    using RenderingRotation = slint::platform::SoftwareRenderer::RenderingRotation;
    RenderingRotation rotation = RenderingRotation::NoRotation;
    slint::PhysicalSize size;

    bool is_transpose() const
    {
        return rotation == RenderingRotation::Rotate90 || rotation == RenderingRotation::Rotate270;
    }

    bool mirror_width() const
    {
        return rotation == RenderingRotation::Rotate180 || rotation == RenderingRotation::Rotate270;
    }

    bool mirror_height() const
    {
        return rotation == RenderingRotation::Rotate90 || rotation == RenderingRotation::Rotate180;
    }
};

slint::LogicalPosition transformed(slint::LogicalPosition p, const RotationInfo &info)
{
    if (info.mirror_width())
        p.x = info.size.width - p.x - 1;
    if (info.mirror_height())
        p.y = info.size.height - p.y - 1;
    if (info.is_transpose())
        std::swap(p.x, p.y);
    return p;
}

slint::PhysicalSize transformed(slint::PhysicalSize s, const RotationInfo &info)
{
    if (info.is_transpose())
        std::swap(s.width, s.height);
    return s;
}
}

using namespace std::chrono_literals;

using RepaintBufferType = slint::platform::SoftwareRenderer::RepaintBufferType;

K_SEM_DEFINE(SLINT_SEM, 0, 1);

class ZephyrPlatform : public slint::platform::Platform
{
public:
    explicit ZephyrPlatform(const struct device *display);

    std::unique_ptr<slint::platform::WindowAdapter> create_window_adapter() override;
    std::chrono::milliseconds duration_since_start() override;
    void run_event_loop() override;
    void quit_event_loop() override;
    void run_in_event_loop(Task) override;

private:
    const struct device *m_display;
    class ZephyrWindowAdapter *m_window = nullptr;

    struct k_mutex m_queue_mutex;
    std::deque<slint::platform::Platform::Task> m_queue; // protected by m_queue_mutex
    bool m_quit = false; // protected by m_queue_mutex
};

class ZephyrWindowAdapter : public slint::platform::WindowAdapter
{
public:
    static std::unique_ptr<ZephyrWindowAdapter> init_from(const device *display);

    explicit ZephyrWindowAdapter(const device *display, RepaintBufferType buffer_type,
                                 const RotationInfo &info);

    void request_redraw() override;
    slint::PhysicalSize size() override;
    slint::platform::AbstractRenderer &renderer() override;

    void maybe_redraw();

    const RotationInfo &rotationInfo() const;

private:
    slint::platform::SoftwareRenderer m_renderer;

    const struct device *m_display;
    const RotationInfo m_rotationInfo;
    const slint::PhysicalSize m_size;

    bool m_needs_redraw = true;
    std::vector<slint::platform::Rgb565Pixel> m_buffer;
    display_buffer_descriptor m_buffer_descriptor;
};

static ZephyrWindowAdapter *ZEPHYR_WINDOW = nullptr;

std::unique_ptr<ZephyrWindowAdapter> ZephyrWindowAdapter::init_from(const device *display)
{
    display_capabilities capabilities;
    display_get_capabilities(display, &capabilities);

    // TODO: Double buffer
    RepaintBufferType bufferType = RepaintBufferType::ReusedBuffer;
    // if (capabilities.screen_info & SCREEN_INFO_DOUBLE_BUFFER)
    //     bufferType = RepaintBufferType::SwappedBuffers;

    LOG_INF("Screen size: %u x %u", capabilities.x_resolution, capabilities.y_resolution);
    LOG_INF("Double buffering: %d", (capabilities.screen_info & SCREEN_INFO_DOUBLE_BUFFER));
    LOG_INF("Has framebuffer: %d", (display_get_framebuffer(display) != nullptr));

    switch (capabilities.current_pixel_format) {
    case PIXEL_FORMAT_RGB_565:
        LOG_INF("Pixel format: RGB_565");
        break;
    case PIXEL_FORMAT_RGB_888:
        // Slint supports this format, but it uses more space.
        LOG_WRN("Unsupported pixel format: RGB_888");
        break;
    case PIXEL_FORMAT_MONO01:
        LOG_WRN("Unsupported pixel format: MONO01");
        break;
    case PIXEL_FORMAT_MONO10:
        LOG_WRN("Unsupported pixel format: MONO10");
        break;
    case PIXEL_FORMAT_ARGB_8888:
        LOG_WRN("Unsupported pixel format: ARGB_8888");
        break;
    case PIXEL_FORMAT_BGR_565:
        LOG_WRN("Unsupported pixel format: BGR_565");
        break;
    }

    LOG_INF("Supports RGB_888: %d",
            static_cast<bool>(capabilities.supported_pixel_formats & PIXEL_FORMAT_RGB_888));
    LOG_INF("Supports MONO01: %d",
            static_cast<bool>(capabilities.supported_pixel_formats & PIXEL_FORMAT_MONO01));
    LOG_INF("Supports MONO10: %d",
            static_cast<bool>(capabilities.supported_pixel_formats & PIXEL_FORMAT_MONO10));
    LOG_INF("Supports ARGB_8888: %d",
            static_cast<bool>(capabilities.supported_pixel_formats & PIXEL_FORMAT_ARGB_8888));
    LOG_INF("Supports RGB_565: %d",
            static_cast<bool>(capabilities.supported_pixel_formats & PIXEL_FORMAT_RGB_565));
    LOG_INF("Supports BGR_565: %d",
            static_cast<bool>(capabilities.supported_pixel_formats & PIXEL_FORMAT_BGR_565));

    if (!is_supported_pixel_format(capabilities.current_pixel_format)) {
        if (capabilities.supported_pixel_formats & PIXEL_FORMAT_RGB_565) {
            LOG_INF("Switching to RGB_565");
            if (const auto result = display_set_pixel_format(display, PIXEL_FORMAT_RGB_565);
                result != 0) {
                LOG_ERR("Failed to set pixel format: %d", result);
            }
        } else {
            LOG_WRN("No supported pixel formats!");
        }
    }

    RotationInfo info;
    info.size = slint::PhysicalSize({ capabilities.x_resolution, capabilities.y_resolution });
    if (IS_ENABLED(CONFIG_MCUX_ELCDIF_PXP_ROTATE_90))
        info.rotation = slint::platform::SoftwareRenderer::RenderingRotation::Rotate270;
    else if (IS_ENABLED(CONFIG_MCUX_ELCDIF_PXP_ROTATE_180))
        info.rotation = slint::platform::SoftwareRenderer::RenderingRotation::Rotate180;
    else if (IS_ENABLED(CONFIG_MCUX_ELCDIF_PXP_ROTATE_270))
        info.rotation = slint::platform::SoftwareRenderer::RenderingRotation::Rotate90;

    const auto rotatedSize = transformed(info.size, info);
    LOG_INF("Rotated screen size: %u x %u", rotatedSize.width, rotatedSize.height);
    return std::make_unique<ZephyrWindowAdapter>(display, bufferType, info);
}

ZephyrWindowAdapter::ZephyrWindowAdapter(const device *display, RepaintBufferType buffer_type,
                                         const RotationInfo &info)
    : m_renderer(buffer_type),
      m_display(display),
      m_rotationInfo(info),
      m_size(transformed(m_rotationInfo.size, m_rotationInfo))
{
    m_buffer.resize(m_size.width * m_size.height);

    m_buffer_descriptor.buf_size = sizeof(m_buffer[0]) * m_buffer.size();
    m_buffer_descriptor.width = m_size.width;
    m_buffer_descriptor.height = m_size.height;
    m_buffer_descriptor.pitch = m_size.width;
}

void ZephyrWindowAdapter::request_redraw()
{
    m_needs_redraw = true;
}

slint::PhysicalSize ZephyrWindowAdapter::size()
{
    return m_size;
}

slint::platform::AbstractRenderer &ZephyrWindowAdapter::renderer()
{
    return m_renderer;
}

void ZephyrWindowAdapter::maybe_redraw()
{
    if (!std::exchange(m_needs_redraw, false))
        return;

    auto start = k_uptime_get();
    auto region = m_renderer.render(m_buffer, m_size.width);
    const auto slintRenderDelta = k_uptime_delta(&start);
    LOG_DBG("Rendering %d dirty regions:", std::ranges::size(region.rectangles()));
    for (auto [o, s] : region.rectangles()) {
#ifndef CONFIG_SHIELD_RK055HDMIPI4MA0
        // Convert to big endian pixel data for Zephyr, unless we are using the RK055HDMIPI4MA0
        // shield. See is_supported_pixel_format above.
        for (int y = o.y; y < o.y + s.height; y++) {
            for (int x = o.x; x < o.x + s.width; x++) {
                auto px = reinterpret_cast<uint16_t *>(&m_buffer[y * m_size.width + x]);
                *px = (*px << 8) | (*px >> 8);
            }
        }
        LOG_DBG("   - converted pixel data for x: %d y: %d w: %d h: %d", o.x, o.y, s.width,
                s.height);
#endif

#ifndef CONFIG_MCUX_ELCDIF_PXP
        m_buffer_descriptor.width = s.width;
        m_buffer_descriptor.height = s.height;

        if (const auto ret = display_write(m_display, o.x, o.y, &m_buffer_descriptor,
                                           m_buffer.data() + ((o.y * m_size.width) + o.x))
                    != 0) {
            LOG_WRN("display_write returned non-zero: %d", ret);
        }
        LOG_DBG("   - rendered x: %d y: %d w: %d h: %d", o.x, o.y, s.width, s.height);
#endif
    }

#ifdef CONFIG_MCUX_ELCDIF_PXP
    // The display driver cannot do partial updates when the PXP is using the DMA API.
    if (const auto ret =
                display_write(m_display, 0, 0, &m_buffer_descriptor, m_buffer.data()) != 0) {
        LOG_WRN("display_write returned non-zero: %d", ret);
    }
    LOG_DBG("   - rendered x: 0 y: 0 w: %d h: %d", m_buffer_descriptor.width,
            m_buffer_descriptor.height);
#endif

    const auto displayWriteDelta = k_uptime_delta(&start);
    LOG_DBG(" - total: %lld ms, slint: %lld ms, write: %lld ms",
            slintRenderDelta + displayWriteDelta, slintRenderDelta, displayWriteDelta);
}

const RotationInfo &ZephyrWindowAdapter::rotationInfo() const
{
    return m_rotationInfo;
}

ZephyrPlatform::ZephyrPlatform(const struct device *display) : m_display(display)
{
    k_mutex_init(&m_queue_mutex);
}

std::unique_ptr<slint::platform::WindowAdapter> ZephyrPlatform::create_window_adapter()
{
    if (m_window || ZEPHYR_WINDOW) {
        LOG_ERR("create_window_adapter called multiple times");
        return nullptr;
    }

    auto window = ZephyrWindowAdapter::init_from(m_display);
    m_window = window.get();
    ZEPHYR_WINDOW = m_window;
    return window;
}

std::chrono::milliseconds ZephyrPlatform::duration_since_start()
{
    // Better precision could be provided by k_uptime_ticks()
    return std::chrono::milliseconds(k_uptime_get());
}

void ZephyrPlatform::run_event_loop()
{
    LOG_DBG("Start");

    while (true) {
        LOG_DBG("Loop");
        slint::platform::update_timers_and_animations();

        std::optional<slint::platform::Platform::Task> event;
        {
            k_unique_lock lock(&m_queue_mutex);
            if (m_queue.empty()) {
                if (m_quit) {
                    m_quit = false;
                    break;
                }
            } else {
                event = std::move(m_queue.front());
                m_queue.pop_front();
            }
        }
        if (event) {
            LOG_DBG("Running event");
            std::move(*event).run();
            event.reset();
            continue;
        }

        if (m_window) {
            m_window->maybe_redraw();

            if (m_window->window().has_active_animations()) {
                LOG_DBG("Has active animations");
#if defined(CONFIG_ARCH_POSIX)
                // The Zephyr POSIX architecture used by the native simulator is unable to interrupt
                // a busy thread. Therefore we must sleep here to allow other threads to progress,
                // otherwise we end up in an infinite loop.
                // https://docs.zephyrproject.org/3.7.0/boards/native/doc/arch_soc.html#important-limitations
                constexpr long simulatorSleepTime = 10;
                LOG_DBG("Sleeping for %llims", simulatorSleepTime);
                k_sem_take(&SLINT_SEM, K_MSEC(simulatorSleepTime));
#endif
                continue;
            }
        }

        if (auto next_timer_update = slint::platform::duration_until_next_timer_update()) {
            const auto wait_time_ms = next_timer_update.value().count();
            LOG_DBG("Sleeping for %llims", wait_time_ms);
            k_sem_take(&SLINT_SEM, K_MSEC(wait_time_ms));
        } else {
            LOG_DBG("Sleeping for forever");
            k_sem_take(&SLINT_SEM, K_FOREVER);
        }
    }
}

void ZephyrPlatform::quit_event_loop()
{
    {
        k_unique_lock lock(&m_queue_mutex);
        m_quit = true;
    }
    k_sem_give(&SLINT_SEM);
}

void ZephyrPlatform::run_in_event_loop(Task event)
{
    {
        k_unique_lock lock(&m_queue_mutex);
        m_queue.push_back(std::move(event));
    }
    k_sem_give(&SLINT_SEM);
}

void zephyr_process_input_event(struct input_event *event, void *user_data)
{
    ARG_UNUSED(user_data);

    static slint::LogicalPosition pos;
    static std::optional<slint::PointerEventButton> button;

    LOG_DBG("Input event. Type: %#x, code: %u (%#x), value: %d, sync: %d", event->type, event->type,
            event->code, event->value, event->sync);

    switch (event->code) {
    case INPUT_BTN_TOUCH:
        break;
    case INPUT_ABS_X:
        pos.x = event->value;
        break;
    case INPUT_ABS_Y:
        pos.y = event->value;
        break;
    default:
        LOG_WRN("Unexpected input event. Type: %#x, code: %u (%#x), value: %d, sync: %d",
                event->type, event->type, event->code, event->value, event->sync);
        return;
    }

    if (event->sync) {
        __ASSERT(event->code == INPUT_BTN_TOUCH,
                 "Expected touch press/release events to be driving the sync status");

        if (!button.has_value()) {
            if (!event->value)
                return;

            LOG_DBG("Press");
            button = slint::PointerEventButton::Left;
            slint::invoke_from_event_loop([=, button = button.value()] {
                __ASSERT(ZEPHYR_WINDOW, "Expected ZephyrWindowAdapter");
                // Transform the physical screen position to the logical coordinate
                const auto slintPos = transformed(pos, ZEPHYR_WINDOW->rotationInfo());
                ZEPHYR_WINDOW->window().dispatch_pointer_move_event(slintPos);
                ZEPHYR_WINDOW->window().dispatch_pointer_press_event(slintPos, button);
            });
        } else if (event->value) {
            LOG_DBG("Move");
            slint::invoke_from_event_loop([=] {
                __ASSERT(ZEPHYR_WINDOW, "Expected ZephyrWindowAdapter");
                // Transform the physical screen position to the logical coordinate
                const auto slintPos = transformed(pos, ZEPHYR_WINDOW->rotationInfo());
                ZEPHYR_WINDOW->window().dispatch_pointer_move_event(slintPos);
            });
        } else {
            LOG_DBG("Release");
            slint::invoke_from_event_loop([=, button = button.value()] {
                __ASSERT(ZEPHYR_WINDOW, "Expected ZephyrWindowAdapter");
                // Transform the physical screen position to the logical coordinate
                const auto slintPos = transformed(pos, ZEPHYR_WINDOW->rotationInfo());
                ZEPHYR_WINDOW->window().dispatch_pointer_release_event(slintPos, button);
                ZEPHYR_WINDOW->window().dispatch_pointer_exit_event();
            });
            button.reset();
        }
    }
}

INPUT_CALLBACK_DEFINE(DEVICE_DT_GET(DT_CHOSEN(zephyr_touch)), zephyr_process_input_event, NULL);

void slint_zephyr_init(const struct device *display)
{
    display_blanking_off(display);
    slint::platform::set_platform(std::make_unique<ZephyrPlatform>(display));
}
