// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#pragma once

#include "slint.h"

#include <utility>
#include <cassert>

struct xcb_connection_t;
struct wl_surface;
struct wl_display;

#if defined(__APPLE__) && !defined(_WIN32) && !defined(_WIN64)
#    ifdef __OBJC__
@class NSView;
@class NSWindow;
#    else
typedef struct objc_object NSView;
typedef struct objc_object NSWindow;
#    endif
#endif

namespace slint {

/// Namespace to be used when you implement your own Platform
namespace platform {

/// Internal interface for a renderer for use with the WindowAdapter.
///
/// You are not supposed to re-implement this class, but you can use one of the provided one
/// such as SoftwareRenderer or SkiaRenderer.
class AbstractRenderer
{
private:
    virtual ~AbstractRenderer() { }
    AbstractRenderer(const AbstractRenderer &) = delete;
    AbstractRenderer &operator=(const AbstractRenderer &) = delete;
    AbstractRenderer() = default;

    /// \private
    virtual cbindgen_private::RendererPtr renderer_handle() const = 0;
    friend class WindowAdapter;
    friend class SoftwareRenderer;
    friend class SkiaRenderer;
};

/// Base class for the layer between a slint::Window and the internal window from the platform
///
/// Re-implement this class to do the link between the two.
///
class WindowAdapter
{
    // This is a pointer to the rust window that own us.
    // Note that we do not have ownership (there is no reference increase for this)
    // because it would otherwise be a reference loop
    cbindgen_private::WindowAdapterRcOpaque self {};
    // Whether this WindowAdapter was already given to the slint runtime
    bool was_initialized = false;

    cbindgen_private::WindowAdapterRcOpaque initialize()
    {
        cbindgen_private::slint_window_adapter_new(
                this, [](void *wa) { delete reinterpret_cast<const WindowAdapter *>(wa); },
                [](void *wa) {
                    return reinterpret_cast<WindowAdapter *>(wa)->renderer().renderer_handle();
                },
                [](void *wa, bool visible) {
                    reinterpret_cast<WindowAdapter *>(wa)->set_visible(visible);
                },
                [](void *wa) { reinterpret_cast<WindowAdapter *>(wa)->request_redraw(); },
                [](void *wa) -> cbindgen_private::IntSize {
                    return reinterpret_cast<const WindowAdapter *>(wa)->physical_size();
                },
                &self);
        was_initialized = true;
        return self;
    }

    friend inline void set_platform(std::unique_ptr<class Platform> platform);

public:
    /// Construct a WindowAdapter
    explicit WindowAdapter() { }
    virtual ~WindowAdapter() = default;

    /// This function is called by Slint when the slint window is shown or hidden.
    ///
    /// Re-implement this function to forward the call to show/hide the native window
    virtual void set_visible(bool) { }

    /// This function is called when Slint detects that the window need to be repainted.
    ///
    /// Reimplement this function to forward the call to the window manager.
    ///
    /// You should not render the window in the implementation of this call. Instead you should
    /// do that in the next iteration of the event loop, or in a callback from the window manager.
    virtual void request_redraw() { }

    /// Returns the actual physical size of the window
    virtual slint::PhysicalSize physical_size() const = 0;

    /// Re-implement this function to provide a reference to the renderer for use with the window
    /// adapter.
    ///
    /// Your re-implementation should contain a renderer such as SoftwareRenderer or SkiaRenderer
    /// and you must return a reference to it.
    virtual AbstractRenderer &renderer() = 0;

    /// Return the slint::Window associated with this window.
    ///
    /// Note that this function can only be called if the window was initialized, which is only
    /// the case after it has been returned from a call to Platform::create_window_adapter
    const Window &window() const
    {
        if (!was_initialized)
            std::abort();
        // This works because cbindgen_private::WindowAdapterRcOpaque and Window have the same
        // layout
        return *reinterpret_cast<const Window *>(&self);
    }

    /// Overload
    Window &window()
    {
        if (!was_initialized)
            std::abort();
        // This works because cbindgen_private::WindowAdapterRcOpaque and Window have the same
        // layout
        return *reinterpret_cast<Window *>(&self);
    }
};

/// The platform is acting like a factory to create a WindowAdapter
///
/// slint::platform::set_platform() need to be called before any other Slint handle
/// are created, and if it is called, it will use the WindowAdapter provided by the
/// create_window_adapter function.
class Platform
{
public:
    virtual ~Platform() = default;
    Platform(const Platform &) = delete;
    Platform &operator=(const Platform &) = delete;
    Platform() = default;

    /// Returns a new WindowAdapter
    virtual std::unique_ptr<WindowAdapter> create_window_adapter() = 0;

    // see internal/core/platform.rs
    enum class Clipboard : uint8_t {
        /// Secondary clipboard on X11.
        DefaultClipboard = 0,
        /// Primary clipboard on X11.
        SelectionClipboard = 1,
    };

#ifndef SLINT_FEATURE_STD
    /// Returns the amount of milliseconds since start of the application.
    ///
    /// This function should only be implemented  if the runtime is compiled with no_std
    virtual std::chrono::milliseconds duration_since_start() const
    {
        return {};
    }
#endif

    /// Sends the given text into the system clipboard.
    ///
    /// If the platform doesn't support the specified clipboard, this function should do nothing
    virtual void set_clipboard_text(const SharedString text, Clipboard clipboard) { }

    /// Returns a copy of text stored in the system clipboard, if any.
    ///
    /// If the platform doesn't support the specified clipboard, the function should return nullopt
    virtual std::optional<SharedString> clipboard_text(Clipboard clipboard)
    {
        return {};
    }

    /// Spins an event loop and renders the visible windows.
    virtual void run_event_loop() { }

    /// Exits the event loop.
    ///
    /// This is what is called by slint::quit_event_loop() and can be called from a different thread
    /// or re-enter from the event loop
    virtual void quit_event_loop() { }

    /// An task that is passed to the Platform::run_in_event_loop function and needs to be
    /// run in the event loop and not in any other thread.
    class Task
    {
        cbindgen_private::PlatformTaskOpaque inner { nullptr, nullptr };
        friend inline void set_platform(std::unique_ptr<Platform> platform);
        explicit Task(cbindgen_private::PlatformTaskOpaque inner) : inner(inner) { }

    public:
        ~Task()
        {
            if (inner._0) {
                cbindgen_private::slint_platform_task_drop(
                        std::exchange(inner, { nullptr, nullptr }));
            }
        }
        Task(const Task &) = delete;
        Task &operator=(const Task &) = delete;
        /// Move constructor. A moved from Task can no longer be run.
        Task(Task &&other) : inner(other.inner) { other.inner = { nullptr, nullptr }; }
        /// Move operator.
        Task &operator=(Task &&other)
        {
            std::swap(other.inner, inner);
            return *this;
        }

        /// Run the task.
        ///
        /// Can only be invoked once and should only be called from the event loop.
        void run() &&
        {
            private_api::assert_main_thread();
            assert(inner._0 && "calling invoke form a moved-from Task");
            if (inner._0) {
                cbindgen_private::slint_platform_task_run(
                        std::exchange(inner, { nullptr, nullptr }));
            }
        };
    };

    /// Run a task from the event loop.
    ///
    /// This function is called by slint::invoke_from_event_loop().
    /// It can be called from any thread, but the passed function must only be called
    /// from the event loop.
    /// Reimplements this function and move the event to the event loop before calling
    /// Task::run()
    virtual void run_in_event_loop(Task) { }
};

/// Registers the platform with Slint. Must be called before Slint windows are created.
/// Can only be called once in an application.
inline void set_platform(std::unique_ptr<Platform> platform)
{
    cbindgen_private::slint_platform_register(
            platform.release(), [](void *p) { delete reinterpret_cast<const Platform *>(p); },
            [](void *p, cbindgen_private::WindowAdapterRcOpaque *out) {
                auto w = reinterpret_cast<Platform *>(p)->create_window_adapter();
                *out = w->initialize();
                (void)w.release();
            },
            []([[maybe_unused]] void *p) -> uint64_t {
#ifdef SLINT_FEATURE_STD
                return 0;
#else
                return reinterpret_cast<const Platform *>(p)->duration_since_start().count();
#endif
            },
            // NOTE: if size_t is not at 32 bit unsigned integer on a 32 bit platform,
            // this may not link with rust properly.
            [](void *p, const SharedString *text, uint8_t clipboard) {
                reinterpret_cast<Platform *>(p)->set_clipboard_text(*text,
                                                                    Platform::Clipboard(clipboard));
            },
            [](void *p, SharedString *out_text, uint8_t clipboard) {
                auto maybe_clipboard = reinterpret_cast<Platform *>(p)->clipboard_text(
                        Platform::Clipboard(clipboard));

                if (maybe_clipboard)
                    out_text = std::move(*maybe_clipboard)
            },
            [](void *p) { return reinterpret_cast<Platform *>(p)->run_event_loop(); },
            [](void *p) { return reinterpret_cast<Platform *>(p)->quit_event_loop(); },
            [](void *p, cbindgen_private::PlatformTaskOpaque event) {
                return reinterpret_cast<Platform *>(p)->run_in_event_loop(Platform::Task(event));
            });
}

#ifdef SLINT_FEATURE_RENDERER_SOFTWARE
/// Represents a region on the screen, used for partial rendering.
///
/// The region may be composed of multiple sub-regions.
struct PhysicalRegion
{
    /// Returns the size of the bounding box of this region.
    PhysicalSize bounding_box_size() const
    {
        return PhysicalSize({ uint32_t(inner.width), uint32_t(inner.height) });
    }
    /// Returns the origin of the bounding box of this region.
    PhysicalPosition bounding_box_origin() const { return PhysicalPosition({ inner.x, inner.y }); }

private:
    cbindgen_private::types::IntRect inner;
    friend class SoftwareRenderer;
    PhysicalRegion(cbindgen_private::types::IntRect inner) : inner(inner) { }
};

/// A 16bit pixel that has 5 red bits, 6 green bits and 5 blue bits
struct Rgb565Pixel
{
    /// The red component, encoded in 5 bits.
    uint16_t r : 5;
    /// The green component, encoded in 6 bits.
    uint16_t g : 6;
    /// The blue component, encoded in 5 bits.
    uint16_t b : 5;

    /// Default constructor.
    constexpr Rgb565Pixel() : r(0), g(0), b(0) { }

    /// \brief Constructor that constructs from an Rgb8Pixel.
    explicit constexpr Rgb565Pixel(const Rgb8Pixel &pixel)
        : r(pixel.r >> 3), g(pixel.g >> 2), b(pixel.b >> 3)
    {
    }

    /// \brief Get the red component as an 8-bit value.
    ///
    /// The bits are shifted so that the result is between 0 and 255.
    /// \return The red component as an 8-bit value.
    constexpr uint8_t red() const { return (r << 3) | (r >> 2); }

    /// \brief Get the green component as an 8-bit value.
    ///
    /// The bits are shifted so that the result is between 0 and 255.
    /// \return The green component as an 8-bit value.
    constexpr uint8_t green() const { return (g << 2) | (g >> 4); }

    /// \brief Get the blue component as an 8-bit value.
    ///
    /// The bits are shifted so that the result is between 0 and 255.
    /// \return The blue component as an 8-bit value.
    constexpr uint8_t blue() const { return (b << 3) | (b >> 2); }

    /// \brief Convert to Rgb8Pixel.
    constexpr operator Rgb8Pixel() const { return { red(), green(), blue() }; }

    /// Returns true if \a lhs \a rhs are pixels with identical colors.
    friend bool operator==(const Rgb565Pixel &lhs, const Rgb565Pixel &rhs) = default;
};

/// Slint's software renderer.
///
/// To be used as a template parameter of the WindowAdapter.
///
/// Use the render() function to render in a buffer
class SoftwareRenderer : public AbstractRenderer
{
    mutable cbindgen_private::SoftwareRendererOpaque inner;

    /// \private
    cbindgen_private::RendererPtr renderer_handle() const override
    {
        return cbindgen_private::slint_software_renderer_handle(inner);
    }

public:
    /// This enum describes which parts of the buffer passed to the SoftwareRenderer may be
    /// re-used to speed up painting.
    enum class RepaintBufferType : uint32_t {
        /// The full window is always redrawn. No attempt at partial rendering will be made.
        NewBuffer = 0,
        /// Only redraw the parts that have changed since the previous call to render().
        ///
        /// This variant assumes that the same buffer is passed on every call to render() and
        /// that it still contains the previously rendered frame.
        ReusedBuffer = 1,

        /// Redraw the part that have changed since the last two frames were drawn.
        ///
        /// This is used when using double buffering and swapping of the buffers.
        SwappedBuffers = 2,
    };

    virtual ~SoftwareRenderer() { cbindgen_private::slint_software_renderer_drop(inner); };
    SoftwareRenderer(const SoftwareRenderer &) = delete;
    SoftwareRenderer &operator=(const SoftwareRenderer &) = delete;
    /// Constructs a new SoftwareRenderer with the \a buffer_type as strategy for handling the
    /// differences between rendering buffers.
    explicit SoftwareRenderer(RepaintBufferType buffer_type)
    {
        inner = cbindgen_private::slint_software_renderer_new(uint32_t(buffer_type));
    }

    /// Render the window scene into a pixel buffer
    ///
    /// The buffer must be at least as large as the associated slint::Window
    ///
    /// The stride is the amount of pixels between two lines in the buffer.
    /// It is must be at least as large as the width of the window.
    PhysicalRegion render(std::span<slint::Rgb8Pixel> buffer, std::size_t pixel_stride) const
    {
        auto r = cbindgen_private::slint_software_renderer_render_rgb8(inner, buffer.data(),
                                                                       buffer.size(), pixel_stride);
        return PhysicalRegion { r };
    }

    /// Render the window scene into an RGB 565 encoded pixel buffer
    ///
    /// The buffer must be at least as large as the associated slint::Window
    ///
    /// The stride is the amount of pixels between two lines in the buffer.
    /// It is must be at least as large as the width of the window.
    PhysicalRegion render(std::span<Rgb565Pixel> buffer, std::size_t pixel_stride) const
    {
        auto r = cbindgen_private::slint_software_renderer_render_rgb565(
                inner, reinterpret_cast<uint16_t *>(buffer.data()), buffer.size(), pixel_stride);
        return PhysicalRegion { r };
    }
};
#endif

#ifdef SLINT_FEATURE_RENDERER_SKIA
/// An opaque, low-level window handle that internalizes everything necessary to exchange messages
/// with the windowing system. This includes the connection to the display server, if necessary.
///
/// Note that this class does not provide any kind of ownership. The caller is responsible for
/// ensuring that the pointers supplied to the constructor are valid throughout the lifetime of the
/// NativeWindowHandle.
class NativeWindowHandle
{
    cbindgen_private::CppRawHandleOpaque inner;
    friend class SkiaRenderer;

    NativeWindowHandle(cbindgen_private::CppRawHandleOpaque inner) : inner(inner) { }

public:
    NativeWindowHandle() = delete;
    NativeWindowHandle(const NativeWindowHandle &) = delete;
    NativeWindowHandle &operator=(const NativeWindowHandle &) = delete;
    /// Creates a new NativeWindowHandle by moving the handle data from \a other into this
    /// NativeWindowHandle.
    NativeWindowHandle(NativeWindowHandle &&other) { inner = std::exchange(other.inner, nullptr); }
    /// Creates a new NativeWindowHandle by moving the handle data from \a other into this
    /// NativeWindowHandle.
    NativeWindowHandle &operator=(NativeWindowHandle &&other)
    {
        if (this == &other) {
            return *this;
        }
        if (inner) {
            cbindgen_private::slint_raw_window_handle_drop(inner);
        }
        inner = std::exchange(other.inner, nullptr);
        return *this;
    }

#    if (!defined(__APPLE__) && !defined(_WIN32) && !defined(_WIN64)) || defined(DOXYGEN)
    /// Creates a new NativeWindowHandle from the given xcb_window_t \a window,
    /// xcb_visualid_t \a visual_id, XCB \a connection, and \a screen number.
    static NativeWindowHandle from_x11_xcb(uint32_t /*xcb_window_t*/ window,
                                           uint32_t /*xcb_visualid_t*/ visual_id,
                                           xcb_connection_t *connection, int screen)
    {

        return { cbindgen_private::slint_new_raw_window_handle_x11_xcb(window, visual_id,
                                                                       connection, screen) };
    }

    /// Creates a new NativeWindowHandle from the given XLib \a window,
    /// VisualID \a visual_id, Display \a display, and \a screen number.
    static NativeWindowHandle from_x11_xlib(uint32_t /*Window*/ window,
                                            unsigned long /*VisualID*/ visual_id,
                                            void /*Display*/ *display, int screen)
    {

        return { cbindgen_private::slint_new_raw_window_handle_x11_xlib(window, visual_id, display,
                                                                        screen) };
    }

    /// Creates a new NativeWindowHandle from the given wayland \a surface,
    /// and \a display.
    static NativeWindowHandle from_wayland(wl_surface *surface, wl_display *display)
    {

        return { cbindgen_private::slint_new_raw_window_handle_wayland(surface, display) };
    }

#    endif
#    if (defined(__APPLE__) && !defined(_WIN32) && !defined(_WIN64)) || defined(DOXYGEN)

    /// Creates a new NativeWindowHandle from the given \a nsview, and \a nswindow.
    static NativeWindowHandle from_appkit(NSView *nsview, NSWindow *nswindow)
    {

        return { cbindgen_private::slint_new_raw_window_handle_appkit(nsview, nswindow) };
    }

#    endif
#    if (!defined(__APPLE__) && (defined(_WIN32) || !defined(_WIN64))) || defined(DOXYGEN)

    /// Creates a new NativeWindowHandle from the given HWND \a hwnd, and HINSTANCE \a hinstance.
    static NativeWindowHandle from_win32(void *hwnd, void *hinstance)
    {
        return { cbindgen_private::slint_new_raw_window_handle_win32(hwnd, hinstance) };
    }
#    endif
    /// Destroys the NativeWindowHandle.
    ~NativeWindowHandle()
    {
        if (inner) {
            cbindgen_private::slint_raw_window_handle_drop(inner);
        }
    }
};

/// Slint's Skia renderer.
///
/// Create the renderer when you have created a native window with a non-zero size.
/// Call the render() function to render the scene into the window.
class SkiaRenderer : public AbstractRenderer
{
    mutable cbindgen_private::SkiaRendererOpaque inner;

    /// \private
    cbindgen_private::RendererPtr renderer_handle() const override
    {
        return cbindgen_private::slint_skia_renderer_handle(inner);
    }

public:
    virtual ~SkiaRenderer() { cbindgen_private::slint_skia_renderer_drop(inner); }
    SkiaRenderer(const SkiaRenderer &) = delete;
    SkiaRenderer &operator=(const SkiaRenderer &) = delete;
    /// Constructs a new Skia renderer for the given window - referenced by the provided
    /// WindowHandle - and the specified initial size.
    explicit SkiaRenderer(const NativeWindowHandle &window_handle, PhysicalSize initial_size)
    {
        inner = cbindgen_private::slint_skia_renderer_new(window_handle.inner, initial_size);
    }

    /// Renders the scene into the window provided to the SkiaRenderer's constructor.
    void render() const { cbindgen_private::slint_skia_renderer_render(inner); }
};
#endif

/// Call this function at each iteration of the event loop to call the timer handler and advance
/// the animations.  This should be called before the rendering or processing input events
inline void update_timers_and_animations()
{
    cbindgen_private::slint_platform_update_timers_and_animations();
}

/// Returns the duration until the next timer if there are  pending timers
inline std::optional<std::chrono::milliseconds> duration_until_next_timer_update()
{
    uint64_t val = cbindgen_private::slint_platform_duration_until_next_timer_update();
    if (val == std::numeric_limits<uint64_t>::max()) {
        return std::nullopt;
    } else if (val >= uint64_t(std::chrono::milliseconds::max().count())) {
        return std::chrono::milliseconds::max();
    } else {
        return std::chrono::milliseconds(val);
    }
}
}
}
