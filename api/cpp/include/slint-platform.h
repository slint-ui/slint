// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "slint.h"

#include <cassert>
#include <cstdint>
#include <utility>
#include <ranges>

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

/// Use the types in this namespace when implementing a custom Slint platform.
///
/// Slint comes with built-in support for different windowing systems, called backends. A backend
/// is a module that implements the Platform interface in this namespace, interacts with a
/// windowing system, and uses one of Slint's renderers to display a scene to the windowing system.
/// A typical Slint application uses one of the built-in backends. Implement your own Platform if
/// you're using Slint in an environment without a windowing system, such as with microcontrollers,
/// or you're embedding a Slint UI as plugin in other applications.
///
/// Examples of custom platform implementation can be found in the Slint repository:
///  - https://github.com/slint-ui/slint/tree/master/examples/cpp/platform_native
///  - https://github.com/slint-ui/slint/tree/master/examples/cpp/platform_qt
///  - https://github.com/slint-ui/slint/blob/master/api/cpp/esp-idf/slint/src/slint-esp.cpp
///
/// The entry point to re-implement a platform is the Platform class. Derive
/// from slint::platform::Platform, and call slint::platform::set_platform
/// to set it as the Slint platform.
///
/// Another important class to subclass is the WindowAdapter.
namespace platform {

/// Internal interface for a renderer for use with the WindowAdapter.
///
/// This class is not intended to be re-implemented. In places where this class is required, use
/// of one the existing implementations such as SoftwareRenderer or SkiaRenderer.
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

/// Base class for the layer between a slint::Window and the windowing system specific window type,
/// such as a Win32 `HWND` handle or a `wayland_surface_t`.
///
/// Re-implement this class to establish the link between the two, and pass messages in both
/// directions:
///
/// - When receiving messages from the windowing system about state changes, such as the window
///   being resized, the user requested the window to be closed, input being received, etc. you
///   need to call the corresponding event functions on the Window, such as
///   Window::dispatch_resize_event(), Window::dispatch_mouse_press_event(), or
///   Window::dispatch_close_requested_event().
///
/// - Slint sends requests to change visibility, position, size, etc. via virtual functions such as
///   set_visible(), set_size(), set_position(), or update_window_properties().
///   Re-implement these functions and delegate the requests to the windowing system.
///
/// If the implementation of this bi-directional message passing protocol is incomplete, the user
/// may experience unexpected behavior, or the intention of the developer calling functions on the
/// Window API may not be fulfilled.
///
/// Your WindowAdapter subclass must hold a renderer (either a SoftwareRenderer or a SkiaRenderer).
/// In the renderer() method, you must return a reference to it.
///
/// # Example
/// ```cpp
/// class MyWindowAdapter : public slint::platform::WindowAdapter {
///     slint::platform::SoftwareRenderer m_renderer;
///     NativeHandle m_native_window; // a handle to the native window
/// public:
///     void request_redraw() override { m_native_window.refresh(); }
///     slint::PhysicalSize size() const override {
///        return slint::PhysicalSize({m_native_window.width, m_native_window.height});
///     }
///     slint::platform::AbstractRenderer &renderer() override { return m_renderer; }
///     void set_visible(bool v) override {
///         if (v) {
///             m_native_window.show();
///         } else {
///             m_native_window.hide();
///         }
///     }
///     // ...
///     void repaint_callback();
/// }
/// ```
///
/// Rendering is typically asynchronous, and your windowing system or event loop would invoke
/// a callback when it is time to render.
/// ```cpp
/// void MyWindowAdapter::repaint_callback()
/// {
///     slint::platform::update_timers_and_animations();
///     m_renderer.render(m_native_window.buffer(), m_native_window.width);
///     // if animations are running, schedule the next frame
///     if (window().has_active_animations())  m_native_window.refresh();
/// }
/// ```
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
                this, [](void *wa) { delete reinterpret_cast<WindowAdapter *>(wa); },
                [](void *wa) {
                    return reinterpret_cast<WindowAdapter *>(wa)->renderer().renderer_handle();
                },
                [](void *wa, bool visible) {
                    reinterpret_cast<WindowAdapter *>(wa)->set_visible(visible);
                },
                [](void *wa) { reinterpret_cast<WindowAdapter *>(wa)->request_redraw(); },
                [](void *wa) -> cbindgen_private::IntSize {
                    return reinterpret_cast<WindowAdapter *>(wa)->size();
                },
                [](void *wa, cbindgen_private::IntSize size) {
                    reinterpret_cast<WindowAdapter *>(wa)->set_size(
                            slint::PhysicalSize({ size.width, size.height }));
                },
                [](void *wa, const cbindgen_private::WindowProperties *p) {
                    reinterpret_cast<WindowAdapter *>(wa)->update_window_properties(
                            *reinterpret_cast<const WindowProperties *>(p));
                },
                [](void *wa, cbindgen_private::Point2D<int32_t> *point) -> bool {
                    if (auto pos = reinterpret_cast<WindowAdapter *>(wa)->position()) {
                        *point = *pos;
                        return true;
                    } else {
                        return false;
                    }
                },
                [](void *wa, cbindgen_private::Point2D<int32_t> point) {
                    reinterpret_cast<WindowAdapter *>(wa)->set_position(
                            slint::PhysicalPosition({ point.x, point.y }));
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
    ///
    /// When the window becomes visible, this is a good time to call
    /// slint::Window::dispatch_scale_factor_change_event to initialise the scale factor.
    virtual void set_visible(bool) { }

    /// This function is called when Slint detects that the window need to be repainted.
    ///
    /// Reimplement this function to forward the call to the window manager.
    ///
    /// You should not render the window in the implementation of this call. Instead you should
    /// do that in the next iteration of the event loop, or in a callback from the window manager.
    virtual void request_redraw() { }

    /// Request a new size for the window to the specified size on the screen, in physical or
    /// logical pixels and excluding a window frame (if present).
    ///
    /// This is called from slint::Window::set_size().
    ///
    /// The default implementation does nothing
    ///
    /// This function should sent the size to the Windowing system. If the window size actually
    /// changes, you should call slint::Window::dispatch_resize_event to propagate the new size
    /// to the slint view.
    virtual void set_size(slint::PhysicalSize) { }

    /// Returns the actual physical size of the window
    virtual slint::PhysicalSize size() = 0;

    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    ///
    /// The default implementation does nothing
    ///
    /// Called from slint::Window::set_position().
    virtual void set_position(slint::PhysicalPosition) { }

    /// Returns the position of the window on the screen, in physical screen coordinates and
    /// including a window frame (if present).
    ///
    /// The default implementation returns std::nullopt.
    ///
    /// Called from slint::Window::position().
    virtual std::optional<slint::PhysicalPosition> position() { return std::nullopt; }

    /// This struct contains getters that provide access to properties of the Window
    /// element, and is used with WindowAdapter::update_window_properties().
    struct WindowProperties
    {
        /// Returns the title of the window.
        SharedString title() const
        {
            SharedString out;
            cbindgen_private::slint_window_properties_get_title(inner(), &out);
            return out;
        }

        /// Returns the background brush of the window.
        Brush background() const
        {
            Brush out;
            cbindgen_private::slint_window_properties_get_background(inner(), &out);
            return out;
        }

        /// \deprecated Use is_fullscreen() instead
        [[deprecated("Renamed is_fullscreen()")]] bool fullscreen() const
        {
            return is_fullscreen();
        }

        /// Returns true if the window should be shown fullscreen; false otherwise.
        bool is_fullscreen() const
        {
            return cbindgen_private::slint_window_properties_get_fullscreen(inner());
        }

        /// Returns true if the window should be minimized; false otherwise
        bool is_minimized() const
        {
            return cbindgen_private::slint_window_properties_get_minimized(inner());
        }

        /// Returns true if the window should be maximized; false otherwise
        bool is_maximized() const
        {
            return cbindgen_private::slint_window_properties_get_maximized(inner());
        }

        /// This struct describes the layout constraints of a window.
        ///
        /// It is the return value of WindowProperties::layout_constraints().
        struct LayoutConstraints
        {
            /// This represents the minimum size the window can be. If this is set, the window
            /// should not be able to be resized smaller than this size. If it is left unset, there
            /// is no minimum size.
            std::optional<LogicalSize> min;
            /// This represents the maximum size the window can be. If this is set, the window
            /// should not be able to be resized larger than this size. If it is left unset, there
            /// is no maximum size.
            std::optional<LogicalSize> max;
            /// This represents the preferred size of the window. This is the size the window
            /// should have by default
            LogicalSize preferred;
        };

        /// Returns the layout constraints of the window
        LayoutConstraints layout_constraints() const
        {
            auto lc = cbindgen_private::slint_window_properties_get_layout_constraints(inner());
            return LayoutConstraints {
                .min = lc.has_min ? std::optional(LogicalSize(lc.min)) : std::nullopt,
                .max = lc.has_max ? std::optional(LogicalSize(lc.max)) : std::nullopt,
                .preferred = LogicalSize(lc.preferred)
            };
        }

    private:
        /// This struct is opaque and cannot be constructed by C++
        WindowProperties() = delete;
        ~WindowProperties() = delete;
        WindowProperties(const WindowProperties &) = delete;
        WindowProperties &operator=(const WindowProperties &) = delete;
        const cbindgen_private::WindowProperties *inner() const
        {
            return reinterpret_cast<const cbindgen_private::WindowProperties *>(this);
        }
    };

    /// Re-implement this function to update the properties such as window title or layout
    /// constraints.
    ///
    /// This function is called before `set_visible(true)`, and will be called again when the
    /// properties that were queried on the last call are changed. If you do not query any
    /// properties, it may not be called again.
    virtual void update_window_properties(const WindowProperties &) { }

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

/// The platform acts as a factory to create WindowAdapter instances.
///
/// Call slint::platform::set_platform() before creating any other Slint handles. Any subsequently
/// created Slint windows will use the WindowAdapter provided by the create_window_adapter function.
class Platform
{
public:
    virtual ~Platform() = default;
    Platform(const Platform &) = delete;
    Platform &operator=(const Platform &) = delete;
    Platform() = default;

    /// Returns a new WindowAdapter
    virtual std::unique_ptr<WindowAdapter> create_window_adapter() = 0;

#if defined(SLINT_FEATURE_FREESTANDING) || defined(DOXYGEN)
    /// Returns the amount of milliseconds since start of the application.
    ///
    /// This function should only be implemented  if the runtime is compiled with
    /// SLINT_FEATURE_FREESTANDING
    virtual std::chrono::milliseconds duration_since_start() = 0;
#endif

    /// The type of clipboard used in Platform::clipboard_text and PLatform::set_clipboard_text.
    enum class Clipboard {
        /// This is the default clipboard used for text action for Ctrl+V,  Ctrl+C.
        /// Corresponds to the secondary selection on X11.
        DefaultClipboard = static_cast<uint8_t>(cbindgen_private::Clipboard::DefaultClipboard),
        /// This is the clipboard that is used when text is selected
        /// Corresponds to the primary selection on X11.
        /// The Platform implementation should do nothing if copy on select is not supported on that
        /// platform.
        SelectionClipboard = static_cast<uint8_t>(cbindgen_private::Clipboard::SelectionClipboard),
    };

    /// Sends the given text into the system clipboard.
    ///
    /// If the platform doesn't support the specified clipboard, this function should do nothing
    virtual void set_clipboard_text(const SharedString &, Clipboard) { }

    /// Returns a copy of text stored in the system clipboard, if any.
    ///
    /// If the platform doesn't support the specified clipboard, the function should return nullopt
    virtual std::optional<SharedString> clipboard_text(Clipboard) { return {}; }

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
#ifndef SLINT_FEATURE_FREESTANDING
                return 0;
#else
                return reinterpret_cast<Platform *>(p)->duration_since_start().count();
#endif
            },
            [](void *p, const SharedString *text, cbindgen_private::Clipboard clipboard) {
                reinterpret_cast<Platform *>(p)->set_clipboard_text(
                        *text, static_cast<Platform::Clipboard>(clipboard));
            },
            [](void *p, SharedString *out_text, cbindgen_private::Clipboard clipboard) -> bool {
                auto maybe_clipboard = reinterpret_cast<Platform *>(p)->clipboard_text(
                        static_cast<Platform::Clipboard>(clipboard));

                bool status = maybe_clipboard.has_value();
                if (status)
                    *out_text = *maybe_clipboard;
                return status;
            },
            [](void *p) { return reinterpret_cast<Platform *>(p)->run_event_loop(); },
            [](void *p) { return reinterpret_cast<Platform *>(p)->quit_event_loop(); },
            [](void *p, cbindgen_private::PlatformTaskOpaque event) {
                return reinterpret_cast<Platform *>(p)->run_in_event_loop(Platform::Task(event));
            });
}

#ifdef SLINT_FEATURE_RENDERER_SOFTWARE

/// A 16bit pixel that has 5 red bits, 6 green bits and 5 blue bits
struct Rgb565Pixel
{
    /// The blue component, encoded in 5 bits.
    uint16_t b : 5;
    /// The green component, encoded in 6 bits.
    uint16_t g : 6;
    /// The red component, encoded in 5 bits.
    uint16_t r : 5;

    /// Default constructor.
    constexpr Rgb565Pixel() : b(0), g(0), r(0) { }

    /// \brief Constructor that constructs from an Rgb8Pixel.
    explicit constexpr Rgb565Pixel(const Rgb8Pixel &pixel)
        : b(pixel.b >> 3), g(pixel.g >> 2), r(pixel.r >> 3)
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
    /// Represents a region on the screen, used for partial rendering.
    ///
    /// The region may be composed of multiple sub-regions.
    struct PhysicalRegion
    {
        /// Returns the size of the bounding box of this region.
        PhysicalSize bounding_box_size() const
        {
            if (inner.count == 0) {
                return PhysicalSize();
            }
            auto origin = bounding_box_origin();
            PhysicalSize size({ .width = uint32_t(inner.rectangles[0].max.x - origin.x),
                                .height = uint32_t(inner.rectangles[0].max.y - origin.y) });
            for (size_t i = 1; i < inner.count; ++i) {
                size.width = std::max(size.width, uint32_t(inner.rectangles[i].max.x - origin.x));
                size.height = std::max(size.height, uint32_t(inner.rectangles[i].max.y - origin.y));
            }
            return size;
        }
        /// Returns the origin of the bounding box of this region.
        PhysicalPosition bounding_box_origin() const
        {
            if (inner.count == 0) {
                return PhysicalPosition();
            }
            PhysicalPosition origin(
                    { .x = inner.rectangles[0].min.x, .y = inner.rectangles[0].min.y });
            for (size_t i = 1; i < inner.count; ++i) {
                origin.x = std::min<int>(origin.x, inner.rectangles[i].min.x);
                origin.y = std::min<int>(origin.y, inner.rectangles[i].min.y);
            }
            return origin;
        }

        /// Returns a view on all the rectangles in this region.
        /// The rectangles do not overlap.
        /// The returned type is a C++ view over PhysicalRegion::Rect structs.
        ///
        /// It can be used like so:
        /// ```cpp
        /// for (auto [origin, size] : region.rectangles()) {
        ///     // Do something with the rect
        /// }
        /// ```
        auto rectangles() const
        {
            SharedVector<cbindgen_private::IntRect> rectangles;
            slint_software_renderer_region_to_rects(&inner, &rectangles);
#    if __cpp_lib_ranges >= 202110L // DR20 P2415R2
            using std::ranges::owning_view;
#    else
            struct owning_view : std::ranges::view_interface<owning_view>
            {
                SharedVector<cbindgen_private::IntRect> rectangles;
                owning_view(SharedVector<cbindgen_private::IntRect> &&rectangles)
                    : rectangles(rectangles)
                {
                }
                auto begin() const { return rectangles.begin(); }
                auto end() const { return rectangles.end(); }
            };
#    endif
            return owning_view(std::move(rectangles)) | std::views::transform([](const auto &r) {
                       return Rect { .origin = PhysicalPosition({ .x = r.x, .y = r.y }),
                                     .size = PhysicalSize({ .width = uint32_t(r.width),
                                                            .height = uint32_t(r.height) }) };
                   });
        }

        /// A Rectangle defined with an origin and a size.
        /// The PhysicalRegion::rectangles() function returns a view over them
        struct Rect
        {
            /// The origin of the rectangle.
            PhysicalPosition origin;
            /// The size of the rectangle.
            PhysicalSize size;
        };

    private:
        cbindgen_private::PhysicalRegion inner;
        friend class SoftwareRenderer;
        PhysicalRegion(cbindgen_private::PhysicalRegion inner) : inner(std::move(inner)) { }
    };

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

#    ifdef SLINT_FEATURE_EXPERIMENTAL
    /// Representation of a texture to blend in the destination buffer.
    // (FIXME: this is currently opaque, but should be exposed)
    using DrawTextureArgs = cbindgen_private::DrawTextureArgs;
    /// Arguments for draw_rectagle
    using DrawRectangleArgs = cbindgen_private::DrawRectangleArgs;

    /// Abstract base class for a target pixel buffer where certain drawing operations can be
    /// delegated. Use this to implement support for hardware accelerators such as DMA2D, PPA, or
    /// PXP on Microcontrollers.
    ///
    /// **Note**: This class is still experimental - it's API is subject to changes and not
    /// stabilized yet. To use the class, you must enable the `SLINT_FEATURE_EXPERIMENTAL=ON` CMake
    /// option.
    template<typename PixelType>
    struct TargetPixelBuffer
    {
        virtual ~TargetPixelBuffer() { }

        /// Returns a span of pixels for the specified line number.
        virtual std::span<PixelType> line_slice(std::size_t line_number) = 0;
        /// Returns the number of lines in the buffer. This is the height of the buffer in pixels.
        virtual std::size_t num_lines() = 0;

        /// Draw a portion of provided texture to the specified pixel coordinates.
        /// Each pixel of the texture is to be blended with the given colorize color as well as the
        /// alpha value.
        // FIXME: Texture is currently opaque, but should be exposed
        virtual bool draw_texture(const DrawTextureArgs &texture, const PhysicalRegion &clip) = 0;

        /// Fill the background of the buffer with the given brush.
        virtual bool fill_background(const Brush &brush, const PhysicalRegion &clip) = 0;

        /// Draw a rectangle specified by the DrawRectangleArgs. That rectangle must be clipped to
        /// the given region.
        virtual bool draw_rectangle(const DrawRectangleArgs &args, const PhysicalRegion &clip) = 0;

    private:
        friend class SoftwareRenderer;
        cbindgen_private::CppTargetPixelBuffer<PixelType> wrap()
        {
            return cbindgen_private::CppTargetPixelBuffer<PixelType> {
                .user_data = this,
                .line_slice =
                        [](void *self, uintptr_t line_number, PixelType **slice_ptr,
                           uintptr_t *slice_len) {
                            auto *buffer = reinterpret_cast<TargetPixelBuffer<PixelType> *>(self);
                            auto slice = buffer->line_slice(line_number);
                            *slice_ptr = slice.data();
                            *slice_len = slice.size();
                        },
                .num_lines =
                        [](void *self) {
                            auto *buffer = reinterpret_cast<TargetPixelBuffer<PixelType> *>(self);
                            return buffer->num_lines();
                        },
                .fill_background =
                        [](void *self, const Brush *brush,
                           const cbindgen_private::PhysicalRegion *clip) {
                            auto *buffer = reinterpret_cast<TargetPixelBuffer<PixelType> *>(self);
                            auto clip_region = PhysicalRegion { *clip };
                            return buffer->fill_background(*brush, clip_region);
                        },
                .draw_rectangle =
                        [](void *self, const cbindgen_private::DrawRectangleArgs *args,
                           const cbindgen_private::PhysicalRegion *clip) {
                            auto *buffer = reinterpret_cast<TargetPixelBuffer<PixelType> *>(self);
                            auto clip_region = PhysicalRegion { *clip };
                            return buffer->draw_rectangle(*args, clip_region);
                        },
                .draw_texture =
                        [](void *self, const cbindgen_private::DrawTextureArgs *texture,
                           const cbindgen_private::PhysicalRegion *clip) {
                            auto *buffer = reinterpret_cast<TargetPixelBuffer<PixelType> *>(self);
                            auto clip_region = PhysicalRegion { *clip };
                            return buffer->draw_texture(*texture, clip_region);
                        }
            };
        }
    };
#    endif

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

    /// Render the window scene, line by line. The provided Callback will be invoked for each line
    /// that needs to rendered.
    ///
    /// The renderer uses a cache internally and will only render the part of the window
    /// which are dirty.
    ///
    /// This function returns the physical region that was rendered considering the rotation.
    ///
    /// The callback must be an invocable with the signature (size_t line, size_t begin, size_t end,
    /// auto render_fn). It is invoked with the line number as first parameter, and the start x and
    /// end x coordinates of the line as second and third parameter. The implementation must provide
    /// a line buffer (as std::span) and invoke the provided fourth parameter (render_fn) with it,
    /// to fill it with pixels. After the line buffer is filled with pixels, your implementation is
    /// free to flush that line to the screen for display.
    ///
    /// The first template parameter (PixelType) must be specified and can be either Rgb565Pixel or
    /// Rgb8Pixel.
    template<typename PixelType, typename Callback>
        requires requires(Callback callback) {
            callback(size_t(0), size_t(0), size_t(0), [&callback](std::span<PixelType>) { });
        }
    PhysicalRegion render_by_line(Callback process_line_callback) const
    {
        auto process_line_fn = [](void *process_line_callback_ptr, uintptr_t line,
                                  uintptr_t line_start, uintptr_t line_end,
                                  void (*render_fn)(const void *, PixelType *, std::size_t),
                                  const void *render_fn_data) {
            (*reinterpret_cast<Callback *>(process_line_callback_ptr))(
                    std::size_t(line), std::size_t(line_start), std::size_t(line_end),
                    [render_fn, render_fn_data](std::span<PixelType> line_span) {
                        render_fn(render_fn_data, line_span.data(), line_span.size());
                    });
        };

        if constexpr (std::is_same_v<PixelType, Rgb565Pixel>) {
            return PhysicalRegion { cbindgen_private::slint_software_renderer_render_by_line_rgb565(
                    inner, process_line_fn, &process_line_callback) };
        } else if constexpr (std::is_same_v<PixelType, Rgb8Pixel>) {
            return PhysicalRegion { cbindgen_private::slint_software_renderer_render_by_line_rgb8(
                    inner, process_line_fn, &process_line_callback) };
        } else {
            static_assert(std::is_same_v<PixelType, Rgba8Pixel>
                                  || std::is_same_v<PixelType, Rgb565Pixel>,
                          "Unsupported PixelType. It must be either Rgba8Pixel or Rgb565Pixel");
        }
    }

#    ifdef SLINT_FEATURE_EXPERIMENTAL
    /// Renders into the given TargetPixelBuffer.
    ///
    /// **Note**: This class is still experimental - it's API is subject to changes and not
    /// stabilized yet. To use the class, you must enable the `SLINT_FEATURE_EXPERIMENTAL=ON` CMake
    /// option.
    PhysicalRegion render(TargetPixelBuffer<Rgb8Pixel> *buffer) const
    {
        auto wrapper = buffer->wrap();
        auto r = cbindgen_private::slint_software_renderer_render_accel_rgb8(inner, &wrapper);
        return PhysicalRegion { r };
    }

    /// Renders into the given TargetPixelBuffer.
    ///
    /// **Note**: This class is still experimental - it's API is subject to changes and not
    /// stabilized yet. To use the class, you must enable the `SLINT_FEATURE_EXPERIMENTAL=ON` CMake
    /// option.
    PhysicalRegion render(TargetPixelBuffer<Rgb565Pixel> *buffer) const
    {
        auto wrapper = buffer->wrap();
        auto r = cbindgen_private::slint_software_renderer_render_accel_rgb565(inner, &wrapper);
        return PhysicalRegion { r };
    }
#    endif

    /// This enum describes the rotation that is applied to the buffer when rendering.
    /// To be used in set_rendering_rotation()
    enum class RenderingRotation {
        /// No rotation
        NoRotation = 0,
        /// Rotate 90° to the left
        Rotate90 = 90,
        /// 180° rotation (upside-down)
        Rotate180 = 180,
        /// Rotate 90° to the right
        Rotate270 = 270,
    };

    /// Set how the window need to be rotated in the buffer.
    ///
    /// This is typically used to implement screen rotation in software
    void set_rendering_rotation(RenderingRotation rotation)
    {
        cbindgen_private::slint_software_renderer_set_rendering_rotation(
                inner, static_cast<int>(rotation));
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
