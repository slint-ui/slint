// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore llims
#include "slint-zephyr.h"

#include <slint-platform.h>

#include <zephyr/logging/log.h>
LOG_MODULE_REGISTER(zephyrSlint, LOG_LEVEL_DBG);

#include <zephyr/kernel.h>
#include <zephyr/version.h>
#include <zephyr/drivers/display.h>
#include <zephyr/input/input.h>

#include <chrono>
#include <deque>
#include <ranges>

// Zephyr renamed this format upstream in commit b13d9a0510b ("display: rename
// current BGR_565 format into RGB_565X"); PIXEL_FORMAT_BGR_565 no longer
// exists from Zephyr v4.4.0 onwards.
#if ZEPHYR_VERSION(4, 4, 0) > ZEPHYR_VERSION_CODE
#    define SLINT_ZEPHYR_PIXEL_FORMAT_BGR_565 PIXEL_FORMAT_BGR_565
#else
#    define SLINT_ZEPHYR_PIXEL_FORMAT_BGR_565 PIXEL_FORMAT_RGB_565X
#endif

namespace {
bool is_supported_pixel_format(display_pixel_format current_pixel_format)
{
    switch (current_pixel_format) {
    case PIXEL_FORMAT_RGB_565:
        return true;
    case PIXEL_FORMAT_RGB_888:
        // Slint supports this format, but it uses more space.
        return false;
    case SLINT_ZEPHYR_PIXEL_FORMAT_BGR_565:
#if defined(CONFIG_SHIELD_RK055HDMIPI4MA0) || defined(CONFIG_SHIELD_LCD_PAR_S035)
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

// Scale factor applied to the Slint window. Values below 1.0 render a larger
// logical UI scaled down to the physical display (e.g. 0.5 shows a 960x640
// logical UI on a 480x320 panel). Override via compile definition.
#ifndef SLINT_ZEPHYR_SCALE_FACTOR
#    define SLINT_ZEPHYR_SCALE_FACTOR 1.0f
#endif

#ifdef SLINT_ZEPHYR_HEAP_PROBE
#    include <cstdlib>
// Measure remaining malloc headroom by greedily allocating blocks (halving on
// failure), then freeing them. Reports total free bytes and the largest
// contiguous block. C malloc returns NULL on failure, so this cannot trigger
// the Rust alloc-failure panic.
static void log_heap_headroom()
{
    void *ptrs[128];
    int n = 0;
    std::size_t total = 0, largest = 0, sz = 128 * 1024;
    while (sz >= 64 && n < 128) {
        void *p = std::malloc(sz);
        if (p) {
            ptrs[n++] = p;
            total += sz;
            if (sz > largest)
                largest = sz;
        } else {
            sz /= 2;
        }
    }
    for (int i = 0; i < n; i++)
        std::free(ptrs[i]);
    LOG_INF("HEAPROBE free=%u largest=%u", (unsigned)total, (unsigned)largest);
}
#endif

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
#ifdef CONFIG_SHIELD_LCD_PAR_S035
    slint::platform::Rgb565Pixel *m_buffer;
#else
    std::vector<slint::platform::Rgb565Pixel> m_buffer;
#endif
    display_buffer_descriptor m_buffer_descriptor;
};

static ZephyrWindowAdapter *ZEPHYR_WINDOW = nullptr;

// LCD-PAR-S035 shield: fixed 480x320 display resolution, BGR565 pixel format.
// render_by_line's callback is invoked once per physical line (see
// slint-platform.h), and maybe_redraw below always fills the buffer from
// offset 0 and flushes it with display_write before the next line is
// rendered, so only one line's worth of pixels is ever live at a time. The
// buffer is a static BSS array (not heap-allocated) to avoid heap allocation
// on this RAM-constrained board (320 KB total SRAM).
//
// With SLINT_ZEPHYR_RENDER_BUFFER_SRAMX defined, the buffer is placed in the
// otherwise-unused SRAMX region instead of the main SRAM. SRAMX must be
// reachable by the display write path for this to work, so it is an opt-in
// toggle.
#ifdef CONFIG_SHIELD_LCD_PAR_S035
#    ifdef SLINT_ZEPHYR_RENDER_BUFFER_SRAMX
#        define SLINT_ZEPHYR_RENDER_BUFFER_SECTION __attribute__((section("SRAMX")))
#    else
#        define SLINT_ZEPHYR_RENDER_BUFFER_SECTION
#    endif
static constexpr std::size_t RENDER_BUFFER_WIDTH = 480;
static constexpr std::size_t RENDER_BUFFER_HEIGHT = 1;
SLINT_ZEPHYR_RENDER_BUFFER_SECTION alignas(8) static slint::platform::Rgb565Pixel
        render_buffer[RENDER_BUFFER_WIDTH * RENDER_BUFFER_HEIGHT];
// GT911 touch X-axis range in native orientation (320 pixels, used for invert-x transform)
static constexpr int GT911_TOUCH_X_MAX = 319;
#endif

std::unique_ptr<ZephyrWindowAdapter> ZephyrWindowAdapter::init_from(const device *display)
{
    display_capabilities capabilities;
    display_get_capabilities(display, &capabilities);

#ifdef CONFIG_SHIELD_LCD_PAR_S035
    RepaintBufferType bufferType = RepaintBufferType::NewBuffer;
#else
    // TODO: Double buffer
    RepaintBufferType bufferType = RepaintBufferType::ReusedBuffer;
    // if (capabilities.screen_info & SCREEN_INFO_DOUBLE_BUFFER)
    //     bufferType = RepaintBufferType::SwappedBuffers;
#endif

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
    case SLINT_ZEPHYR_PIXEL_FORMAT_BGR_565:
        LOG_WRN("Unsupported pixel format: RGB_565X");
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
    LOG_INF("Supports RGB_565X: %d",
            static_cast<bool>(capabilities.supported_pixel_formats
                              & SLINT_ZEPHYR_PIXEL_FORMAT_BGR_565));

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
#ifdef CONFIG_SHIELD_LCD_PAR_S035
    m_buffer = render_buffer;
    m_buffer_descriptor.buf_size = sizeof(render_buffer);
    m_buffer_descriptor.width = m_size.width;
    m_buffer_descriptor.height = RENDER_BUFFER_HEIGHT;
#else
    m_buffer.resize(m_size.width * m_size.height);
    m_buffer_descriptor.buf_size = sizeof(m_buffer[0]) * m_buffer.size();
    m_buffer_descriptor.width = m_size.width;
    m_buffer_descriptor.height = m_size.height;
#endif
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

#ifdef CONFIG_SHIELD_LCD_PAR_S035
    display_buffer_descriptor line_desc {};
    line_desc.pitch = m_size.width;

    m_renderer.render_by_line<slint::platform::Rgb565Pixel>(
            [this, &line_desc](size_t line_y, size_t first_x, size_t last_x, auto render_fn) {
                size_t width = last_x - first_x;
                render_fn(std::span<slint::platform::Rgb565Pixel>(m_buffer, width));

                line_desc.width = width;
                line_desc.height = 1;
                line_desc.buf_size = width * sizeof(slint::platform::Rgb565Pixel);

                display_write(m_display, first_x, line_y, &line_desc, m_buffer);
            });
#else
    auto start = k_uptime_get();
    auto region = m_renderer.render(m_buffer, m_size.width);
    const auto slintRenderDelta = k_uptime_delta(&start);
    LOG_DBG("Rendering %d dirty regions:", std::ranges::size(region.rectangles()));
    for (auto [o, s] : region.rectangles()) {
#    ifndef CONFIG_SHIELD_RK055HDMIPI4MA0
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
#    endif

#    ifndef CONFIG_MCUX_ELCDIF_PXP
        m_buffer_descriptor.width = s.width;
        m_buffer_descriptor.height = s.height;

        if (const auto ret = display_write(m_display, o.x, o.y, &m_buffer_descriptor,
                                           m_buffer.data() + ((o.y * m_size.width) + o.x))
                    != 0) {
            LOG_WRN("display_write returned non-zero: %d", ret);
        }
        LOG_DBG("   - rendered x: %d y: %d w: %d h: %d", o.x, o.y, s.width, s.height);
#    endif
    }

#    ifdef CONFIG_MCUX_ELCDIF_PXP
    // The display driver cannot do partial updates when the PXP is using the DMA API.
    if (const auto ret =
                display_write(m_display, 0, 0, &m_buffer_descriptor, m_buffer.data()) != 0) {
        LOG_WRN("display_write returned non-zero: %d", ret);
    }
    LOG_DBG("   - rendered x: 0 y: 0 w: %d h: %d", m_buffer_descriptor.width,
            m_buffer_descriptor.height);
#    endif

    const auto displayWriteDelta = k_uptime_delta(&start);
    LOG_DBG(" - total: %lld ms, slint: %lld ms, write: %lld ms",
            slintRenderDelta + displayWriteDelta, slintRenderDelta, displayWriteDelta);
#endif
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

    if (m_window && SLINT_ZEPHYR_SCALE_FACTOR != 1.0f) {
        const auto physical = m_window->size();
        m_window->window().dispatch_scale_factor_change_event(SLINT_ZEPHYR_SCALE_FACTOR);
        m_window->window().dispatch_resize_event(
                slint::LogicalSize({ physical.width / SLINT_ZEPHYR_SCALE_FACTOR,
                                     physical.height / SLINT_ZEPHYR_SCALE_FACTOR }));
        LOG_INF("Scale factor: %f, logical size: %f x %f", (double)SLINT_ZEPHYR_SCALE_FACTOR,
                (double)(physical.width / SLINT_ZEPHYR_SCALE_FACTOR),
                (double)(physical.height / SLINT_ZEPHYR_SCALE_FACTOR));
    }

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

#ifdef SLINT_ZEPHYR_HEAP_PROBE
            static int64_t last_probe = -10000; // fire on the first frame, then every 5s
            if (k_uptime_get() - last_probe > 5000) {
                last_probe = k_uptime_get();
                log_heap_headroom();
            }
#endif

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

// Transform a physical touch position (after rotation) into logical coordinates
static slint::LogicalPosition to_logical(slint::LogicalPosition p)
{
    return slint::LogicalPosition(
            { p.x / SLINT_ZEPHYR_SCALE_FACTOR, p.y / SLINT_ZEPHYR_SCALE_FACTOR });
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
#if defined(CONFIG_SHIELD_LCD_PAR_S035) && ZEPHYR_VERSION(4, 4, 0) > ZEPHYR_VERSION_CODE
        // LCD-PAR-S035: swap-xy + invert-x (workaround for GT911 driver lacking
        // touchscreen-common support before Zephyr v4.4.0; see upstream 0f07faa14b3)
        pos.y = GT911_TOUCH_X_MAX - event->value;
#else
        pos.x = event->value;
#endif
        break;
    case INPUT_ABS_Y:
#if defined(CONFIG_SHIELD_LCD_PAR_S035) && ZEPHYR_VERSION(4, 4, 0) > ZEPHYR_VERSION_CODE
        pos.x = event->value;
#else
        pos.y = event->value;
#endif
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
                const auto slintPos = to_logical(transformed(pos, ZEPHYR_WINDOW->rotationInfo()));
                ZEPHYR_WINDOW->window().dispatch_pointer_move_event(slintPos);
                ZEPHYR_WINDOW->window().dispatch_pointer_press_event(slintPos, button);
            });
        } else if (event->value) {
            LOG_DBG("Move");
            slint::invoke_from_event_loop([=] {
                __ASSERT(ZEPHYR_WINDOW, "Expected ZephyrWindowAdapter");
                // Transform the physical screen position to the logical coordinate
                const auto slintPos = to_logical(transformed(pos, ZEPHYR_WINDOW->rotationInfo()));
                ZEPHYR_WINDOW->window().dispatch_pointer_move_event(slintPos);
            });
        } else {
            LOG_DBG("Release");
            slint::invoke_from_event_loop([=, button = button.value()] {
                __ASSERT(ZEPHYR_WINDOW, "Expected ZephyrWindowAdapter");
                // Transform the physical screen position to the logical coordinate
                const auto slintPos = to_logical(transformed(pos, ZEPHYR_WINDOW->rotationInfo()));
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
