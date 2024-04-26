#include "slint-zephyr.h"

#include <slint-platform.h>

#include <zephyr/logging/log.h>
LOG_MODULE_REGISTER(zephyrSlint, LOG_LEVEL_DBG);

#include <zephyr/kernel.h>
#include <zephyr/drivers/display.h>

#include <chrono>

namespace {
bool is_supported_pixel_format(display_pixel_format current_pixel_format)
{
    switch (current_pixel_format) {
    case PIXEL_FORMAT_RGB_565:
        return true;
    case PIXEL_FORMAT_RGB_888:
        // Slint supports this format, but it uses more space.
        return false;
    case PIXEL_FORMAT_MONO01:
    case PIXEL_FORMAT_MONO10:
    case PIXEL_FORMAT_ARGB_8888:
    case PIXEL_FORMAT_BGR_565:
        return false;
    }
    assert(false);
}
}

using namespace std::chrono_literals;

using RepaintBufferType = slint::platform::SoftwareRenderer::RepaintBufferType;

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
};

class ZephyrWindowAdapter : public slint::platform::WindowAdapter
{
public:
    static std::unique_ptr<ZephyrWindowAdapter> init_from(const device *display);

    explicit ZephyrWindowAdapter(const device *display, RepaintBufferType buffer_type,
                                 slint::PhysicalSize size);

    void request_redraw() override;
    slint::PhysicalSize size() override;
    slint::platform::AbstractRenderer &renderer() override;

    void maybe_redraw();

private:
    slint::platform::SoftwareRenderer m_renderer;

    const struct device *m_display;
    const slint::PhysicalSize m_size;

    bool m_needs_redraw = true;
    std::vector<slint::platform::Rgb565Pixel> m_buffer;
    display_buffer_descriptor m_buffer_descriptor;
};

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
            LOG_ERR("No supported pixel formats!");
        }
    }

    return std::make_unique<ZephyrWindowAdapter>(
            display, bufferType,
            slint::PhysicalSize({ capabilities.x_resolution, capabilities.y_resolution }));
}

ZephyrWindowAdapter::ZephyrWindowAdapter(const device *display, RepaintBufferType buffer_type,
                                         slint::PhysicalSize size)
    : m_renderer(buffer_type), m_display(display), m_size(size)
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

    display_blanking_on(m_display);
    auto rotated = false;
    auto region = m_renderer.render(m_buffer, rotated ? m_size.height : m_size.width);
    auto o = region.bounding_box_origin();
    auto s = region.bounding_box_size();
    LOG_DBG("Rendering x: %d y: %d w: %d h: %d", o.x, o.y, s.width, s.height);
    if (s.width > 0 && s.height > 0) {
        for (int y = o.y; y < o.y + s.height; y++) {
            for (int x = o.x; x < o.x + s.width; x++) {
                // Swap endianess to big endian
                auto px = reinterpret_cast<uint16_t *>(&m_buffer[y * m_size.width + x]);
                *px = (*px << 8) | (*px >> 8);
            }
        }

        m_buffer_descriptor.width = s.width;
        m_buffer_descriptor.height = s.height;

        if (const auto ret = display_write(m_display, o.x, o.y, &m_buffer_descriptor,
                                           m_buffer.data() + ((o.y * m_size.width) + o.x))
                    != 0) {
            LOG_WRN("display_write returned non-zero: %d", ret);
        }
    }
    display_blanking_off(m_display);
}

ZephyrPlatform::ZephyrPlatform(const struct device *display) : m_display(display) { }

std::unique_ptr<slint::platform::WindowAdapter> ZephyrPlatform::create_window_adapter()
{
    if (m_window) {
        LOG_ERR("create_window_adapter called multiple times");
        return nullptr;
    }

    auto window = ZephyrWindowAdapter::init_from(m_display);
    m_window = window.get();
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
    const auto max_wait_time = 1000ms; // TODO: K_FOREVER?

    while (true) {
        LOG_DBG("Loop");
        slint::platform::update_timers_and_animations();

        if (m_window) {
            m_window->maybe_redraw();

            if (m_window->window().has_active_animations()) {
                continue;
            }
        }

        auto wait_time = max_wait_time;
        if (auto next_timer_update = slint::platform::duration_until_next_timer_update()) {
            wait_time = std::min(wait_time, next_timer_update.value());
        }
        LOG_DBG("Sleeping for %ims", wait_time.count());
        k_sleep(K_MSEC(wait_time.count()));
    }
}

void ZephyrPlatform::quit_event_loop()
{
    // TODO
}

void ZephyrPlatform::run_in_event_loop(Task event)
{
    // TODO
    (void)event;
}

void slint_zephyr_init(const struct device *display)
{
    slint::platform::set_platform(std::make_unique<ZephyrPlatform>(display));
}
