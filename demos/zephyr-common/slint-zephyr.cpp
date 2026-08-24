// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore llims
#include "slint-zephyr.h"

#include <slint-platform.h>

#include <zephyr/logging/log.h>
LOG_MODULE_REGISTER(zephyrSlint, LOG_LEVEL_DBG);

#include <zephyr/kernel.h>
#include <zephyr/drivers/display.h>
#include <zephyr/input/input.h>
#include <zephyr/version.h>

// Zephyr 4.4 renamed PIXEL_FORMAT_BGR_565 to PIXEL_FORMAT_RGB_565X. The EK-RZ/A3M board support
// currently only exists on a 4.3 based fork, so this demo has to compile against both spellings.
#if KERNEL_VERSION_NUMBER >= ZEPHYR_VERSION(4, 4, 0)
#    define SLINT_PIXEL_FORMAT_RGB565_SWAPPED PIXEL_FORMAT_RGB_565X
#    define SLINT_PIXEL_FORMAT_RGB565_SWAPPED_NAME "RGB_565X"
#else
#    define SLINT_PIXEL_FORMAT_RGB565_SWAPPED PIXEL_FORMAT_BGR_565
#    define SLINT_PIXEL_FORMAT_RGB565_SWAPPED_NAME "BGR_565"
#endif

#include <chrono>
#include <deque>
#include <ranges>

// Set by boards whose driver declares one byte order and consumes the other, and by trees
// older than Zephyr 4.4. See https://github.com/zephyrproject-rtos/zephyr/issues/53642
#ifndef SLINT_ZEPHYR_RGB565_NATIVE_ENDIAN
#    define SLINT_ZEPHYR_RGB565_NATIVE_ENDIAN 0
#endif

// The rotation, in degrees, that brings the panel's natural orientation in line with the
// orientation the user interface is designed for. Panels that are mounted sideways define this in
// their board's section of the demo's CMakeLists.txt, and the software renderer turns the user
// interface while it draws.
#ifndef SLINT_ZEPHYR_PANEL_ROTATION
#    define SLINT_ZEPHYR_PANEL_ROTATION 0
#endif

namespace {
constexpr bool is_big_endian_format(display_pixel_format format)
{
#if KERNEL_VERSION_NUMBER >= ZEPHYR_VERSION(4, 4, 0)
    return format == SLINT_PIXEL_FORMAT_RGB565_SWAPPED;
#else
    // display_sdl.c read RGB_565 with sys_be16_to_cpu() and BGR_565 natively until the rename.
    return format == PIXEL_FORMAT_RGB_565;
#endif
}

constexpr bool needs_byte_swap(display_pixel_format format)
{
    if (SLINT_ZEPHYR_RGB565_NATIVE_ENDIAN) {
        return false;
    }
    return is_big_endian_format(format) != static_cast<bool>(IS_ENABLED(CONFIG_BIG_ENDIAN));
}

bool is_supported_pixel_format(display_pixel_format current_pixel_format)
{
    switch (current_pixel_format) {
    case PIXEL_FORMAT_RGB_565:
    case SLINT_PIXEL_FORMAT_RGB565_SWAPPED:
        return true;
    case PIXEL_FORMAT_RGB_888:
        // Slint supports this format, but it uses more space.
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

using RenderingRotation = slint::platform::SoftwareRenderer::RenderingRotation;

constexpr int rotation_degrees(RenderingRotation rotation)
{
    return static_cast<int>(rotation);
}

constexpr RenderingRotation rotation_from_degrees(int degrees)
{
    switch (((degrees % 360) + 360) % 360) {
    case 90:
        return RenderingRotation::Rotate90;
    case 180:
        return RenderingRotation::Rotate180;
    case 270:
        return RenderingRotation::Rotate270;
    default:
        return RenderingRotation::NoRotation;
    }
}

constexpr bool transposes(RenderingRotation rotation)
{
    return rotation == RenderingRotation::Rotate90 || rotation == RenderingRotation::Rotate270;
}

constexpr slint::PhysicalSize transposed_if(slint::PhysicalSize size, bool condition)
{
    if (condition)
        std::swap(size.width, size.height);
    return size;
}

// Describes how the panel, the frame buffer and the user interface are oriented relative to each
// other. Two rotations can be in play at once: the display hardware may turn the frame buffer on
// its way to the panel, and the software renderer may turn the user interface while it draws.
struct DisplayRotation
{
    // Resolution the display driver reports, in the panel's natural orientation.
    slint::PhysicalSize panel_size;
    // Rotation the software renderer applies while drawing into the frame buffer.
    RenderingRotation rendering = RenderingRotation::NoRotation;
    // Rotation the display hardware applies to the frame buffer on its way to the panel, for
    // instance through the i.MX RT PXP. The renderer must not turn the interface again in that
    // case, so this is kept apart from `rendering`.
    RenderingRotation hardware = RenderingRotation::NoRotation;

    // Geometry of the frame buffer handed to display_write(): the panel geometry, transposed when
    // the hardware turns it by a quarter.
    slint::PhysicalSize buffer_size() const
    {
        return transposed_if(panel_size, transposes(hardware));
    }

    // Size the user interface is laid out in, which is what the user ends up seeing.
    slint::PhysicalSize logical_size() const
    {
        return transposed_if(buffer_size(), transposes(rendering));
    }

    // Rotation that maps a position reported by the touch controller, which is in panel
    // coordinates, into logical coordinates. Turning the user interface by `rendering` carries a
    // touch along by the same amount, whereas `hardware` has already been applied to the panel and
    // has to be undone.
    RenderingRotation touch_rotation() const
    {
        return rotation_from_degrees(rotation_degrees(rendering) - rotation_degrees(hardware));
    }
};

// Applies `rotation` to `position`, which lives in a coordinate system of `size`.
slint::LogicalPosition rotated(slint::LogicalPosition position, RenderingRotation rotation,
                               slint::PhysicalSize size)
{
    switch (rotation) {
    case RenderingRotation::NoRotation:
        break;
    case RenderingRotation::Rotate90:
        return slint::LogicalPosition({ position.y, size.width - position.x - 1 });
    case RenderingRotation::Rotate180:
        return slint::LogicalPosition(
                { size.width - position.x - 1, size.height - position.y - 1 });
    case RenderingRotation::Rotate270:
        return slint::LogicalPosition({ size.height - position.y - 1, position.x });
    }
    return position;
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
                                 const DisplayRotation &rotation, bool needs_byte_swap);

    void request_redraw() override;
    slint::PhysicalSize size() override;
    slint::platform::AbstractRenderer &renderer() override;

    void maybe_redraw();

    // Maps a position reported by the touch controller into logical coordinates.
    slint::LogicalPosition map_touch_position(slint::LogicalPosition position) const;

private:
    slint::platform::SoftwareRenderer m_renderer;

    const struct device *m_display;
    const DisplayRotation m_rotation;
    const slint::PhysicalSize m_buffer_size;
    const bool m_needs_byte_swap;

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
    case SLINT_PIXEL_FORMAT_RGB565_SWAPPED:
        LOG_WRN("Unsupported pixel format: " SLINT_PIXEL_FORMAT_RGB565_SWAPPED_NAME);
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
    LOG_INF("Supports " SLINT_PIXEL_FORMAT_RGB565_SWAPPED_NAME ": %d",
            static_cast<bool>(capabilities.supported_pixel_formats
                              & SLINT_PIXEL_FORMAT_RGB565_SWAPPED));

    // Keep the format the display already uses: its panel may be wired for that byte order.
    auto pixel_format = capabilities.current_pixel_format;
    if (!is_supported_pixel_format(pixel_format)) {
        if (capabilities.supported_pixel_formats & PIXEL_FORMAT_RGB_565) {
            LOG_INF("Switching to RGB_565");
            pixel_format = PIXEL_FORMAT_RGB_565;
        } else if (capabilities.supported_pixel_formats & SLINT_PIXEL_FORMAT_RGB565_SWAPPED) {
            LOG_INF("Switching to " SLINT_PIXEL_FORMAT_RGB565_SWAPPED_NAME);
            pixel_format = SLINT_PIXEL_FORMAT_RGB565_SWAPPED;
        } else {
            LOG_WRN("No supported pixel formats!");
        }

        if (pixel_format != capabilities.current_pixel_format) {
            if (const auto result = display_set_pixel_format(display, pixel_format); result != 0) {
                LOG_ERR("Failed to set pixel format: %d", result);
                pixel_format = capabilities.current_pixel_format;
            }
        }
    }
    LOG_INF("Byte swapping pixel data: %d", needs_byte_swap(pixel_format));

    DisplayRotation rotation;
    rotation.panel_size =
            slint::PhysicalSize({ capabilities.x_resolution, capabilities.y_resolution });

    // The PXP turns the frame buffer on its way to the panel. Its Kconfig names the rotation the
    // panel sees, which is the opposite of the one the frame buffer undergoes.
    if (IS_ENABLED(CONFIG_MCUX_ELCDIF_PXP_ROTATE_90))
        rotation.hardware = RenderingRotation::Rotate270;
    else if (IS_ENABLED(CONFIG_MCUX_ELCDIF_PXP_ROTATE_180))
        rotation.hardware = RenderingRotation::Rotate180;
    else if (IS_ENABLED(CONFIG_MCUX_ELCDIF_PXP_ROTATE_270))
        rotation.hardware = RenderingRotation::Rotate90;

    // Panels that are mounted sideways are turned by the software renderer instead.
    rotation.rendering = rotation_from_degrees(SLINT_ZEPHYR_PANEL_ROTATION);

    const auto logicalSize = rotation.logical_size();
    LOG_INF("User interface size: %u x %u", logicalSize.width, logicalSize.height);
    return std::make_unique<ZephyrWindowAdapter>(display, bufferType, rotation,
                                                 needs_byte_swap(pixel_format));
}

ZephyrWindowAdapter::ZephyrWindowAdapter(const device *display, RepaintBufferType buffer_type,
                                         const DisplayRotation &rotation, bool needs_byte_swap)
    : m_renderer(buffer_type),
      m_display(display),
      m_rotation(rotation),
      m_buffer_size(rotation.buffer_size()),
      m_needs_byte_swap(needs_byte_swap)
{
    m_buffer.resize(m_buffer_size.width * m_buffer_size.height);

    m_buffer_descriptor.buf_size = sizeof(m_buffer[0]) * m_buffer.size();
    m_buffer_descriptor.width = m_buffer_size.width;
    m_buffer_descriptor.height = m_buffer_size.height;
    m_buffer_descriptor.pitch = m_buffer_size.width;

    m_renderer.set_rendering_rotation(m_rotation.rendering);
}

void ZephyrWindowAdapter::request_redraw()
{
    m_needs_redraw = true;
}

slint::PhysicalSize ZephyrWindowAdapter::size()
{
    return m_rotation.logical_size();
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
    auto region = m_renderer.render(m_buffer, m_buffer_size.width);
    const auto slintRenderDelta = k_uptime_delta(&start);
    LOG_DBG("Rendering %d dirty regions:", std::ranges::size(region.rectangles()));
    for (auto [o, s] : region.rectangles()) {
        if (m_needs_byte_swap) {
            for (int y = o.y; y < o.y + s.height; y++) {
                for (int x = o.x; x < o.x + s.width; x++) {
                    auto px = reinterpret_cast<uint16_t *>(&m_buffer[y * m_buffer_size.width + x]);
                    *px = (*px << 8) | (*px >> 8);
                }
            }
            LOG_DBG("   - converted pixel data for x: %d y: %d w: %d h: %d", o.x, o.y, s.width,
                    s.height);
        }

#ifndef CONFIG_MCUX_ELCDIF_PXP
        m_buffer_descriptor.width = s.width;
        m_buffer_descriptor.height = s.height;

        if (const auto ret = display_write(m_display, o.x, o.y, &m_buffer_descriptor,
                                           m_buffer.data() + ((o.y * m_buffer_size.width) + o.x))
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

slint::LogicalPosition
ZephyrWindowAdapter::map_touch_position(slint::LogicalPosition position) const
{
    return rotated(position, m_rotation.touch_rotation(), m_rotation.panel_size);
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
            auto wait_time_ms = next_timer_update.value().count();
#ifdef CONFIG_BOARD_RZA3M_EK
            wait_time_ms = std::min(wait_time_ms, static_cast<decltype(wait_time_ms)>(10000));
#endif
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
                // Transform the panel position to the logical coordinate
                const auto slintPos = ZEPHYR_WINDOW->map_touch_position(pos);
                ZEPHYR_WINDOW->window().dispatch_pointer_move_event(slintPos);
                ZEPHYR_WINDOW->window().dispatch_pointer_press_event(slintPos, button);
            });
        } else if (event->value) {
            LOG_DBG("Move");
            slint::invoke_from_event_loop([=] {
                __ASSERT(ZEPHYR_WINDOW, "Expected ZephyrWindowAdapter");
                // Transform the panel position to the logical coordinate
                const auto slintPos = ZEPHYR_WINDOW->map_touch_position(pos);
                ZEPHYR_WINDOW->window().dispatch_pointer_move_event(slintPos);
            });
        } else {
            LOG_DBG("Release");
            slint::invoke_from_event_loop([=, button = button.value()] {
                __ASSERT(ZEPHYR_WINDOW, "Expected ZephyrWindowAdapter");
                // Transform the panel position to the logical coordinate
                const auto slintPos = ZEPHYR_WINDOW->map_touch_position(pos);
                ZEPHYR_WINDOW->window().dispatch_pointer_release_event(slintPos, button);
                ZEPHYR_WINDOW->window().dispatch_pointer_exit_event();
            });
            button.reset();
        }
    }
}

#if DT_HAS_CHOSEN(zephyr_touch)
INPUT_CALLBACK_DEFINE(DEVICE_DT_GET(DT_CHOSEN(zephyr_touch)), zephyr_process_input_event, NULL);
#endif

void slint_zephyr_init(const struct device *display)
{
    display_blanking_off(display);
    slint::platform::set_platform(std::make_unique<ZephyrPlatform>(display));
}
