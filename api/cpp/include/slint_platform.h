// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#pragma once

#ifndef SLINT_FEATURE_EXPERIMENTAL
#    warning "slint_platform.h API only available when SLINT_FEATURE_EXPERIMENTAL is activated"
#else

#    include "slint.h"
#    include <utility>

struct xcb_connection_t;
struct wl_surface;
struct wl_display;

namespace slint {

/// This namespace contains experimental API.
/// No compatibility across version.
///
/// \private
namespace experimental {

/// Namespace to be used when you implement your own Platform
namespace platform {

/// Internal interface for a renderer for use with the WindowAdapter.
class AbstractRenderer
{
private:
    /// \private
    virtual cbindgen_private::RendererPtr renderer_handle() const = 0;
    friend class WindowAdapter;
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

private:
    friend class Platform;

    cbindgen_private::WindowAdapterRcOpaque initialize()
    {
        cbindgen_private::slint_window_adapter_new(
                this, [](void *wa) { delete reinterpret_cast<const WindowAdapter *>(wa); },
                [](void *wa) {
                    return reinterpret_cast<WindowAdapter *>(wa)->renderer().renderer_handle();
                },
                [](void *wa) { reinterpret_cast<const WindowAdapter *>(wa)->show(); },
                [](void *wa) { reinterpret_cast<const WindowAdapter *>(wa)->hide(); },
                [](void *wa) { reinterpret_cast<const WindowAdapter *>(wa)->request_redraw(); },
                [](void *wa) -> cbindgen_private::IntSize {
                    return reinterpret_cast<const WindowAdapter *>(wa)->physical_size();
                },
                &self);
        was_initialized = true;
        return self;
    }

public:
    /// Construct a WindowAdapter
    explicit WindowAdapter() { }
    virtual ~WindowAdapter() = default;

    /// This function is called by Slint when the slint window is shown.
    ///
    /// Re-implement this function to forward the call to show to the native window
    virtual void show() const { }
    /// This function is called by Slint when the slint window is hidden.
    ///
    /// Re-implement this function to forward the call to hide to the native window
    virtual void hide() const { }

    /// This function is called when Slint detects that the window need to be repainted.
    ///
    /// Reimplement this function to forward the call to the window manager.
    ///
    /// You should not render the window in the implementation of this call. Instead you should
    /// do that in the next iteration of the event loop, or in a callback from the window manager.
    virtual void request_redraw() const { }

    /// Returns the actual physical size of the window
    virtual slint::PhysicalSize physical_size() const = 0;

    /// Re-implement this function to provide a reference to the renderer for use with the window
    /// adapter.
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

    /// Send a pointer event to this window
    // Note: in rust, this is on the Window. FIXME: use a public event type
    void dispatch_pointer_event(const cbindgen_private::MouseEvent &event)
    {
        private_api::assert_main_thread();
        if (was_initialized) {
            cbindgen_private::slint_windowrc_dispatch_pointer_event(&self, event);
        }
    }

    /// Set the logical size of this window after a resize event
    // Note: in rust, this is an event on the Window
    void dispatch_resize_event(slint::LogicalSize s)
    {
        private_api::assert_main_thread();
        if (was_initialized) {
            cbindgen_private::slint_windowrc_dispatch_resize_event(&self, s.width, s.height);
        }
    }

    /// Notifies the platform about a change in the device pixel ratio.
    void dispatch_scale_factor_change_event(float factor)
    {
        private_api::assert_main_thread();
        if (was_initialized) {
            cbindgen_private::slint_windowrc_dispatch_scale_factor_change_event(&self, factor);
        }
    }

    /// Returns true if the window is currently animating
    bool has_active_animations() const
    {
        return cbindgen_private::slint_windowrc_has_active_animations(&self);
    }
};

/// The platform is acting like a factory to create a WindowAdapter
///
/// Platform::register_platform() need to be called before any other Slint handle
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
    virtual std::unique_ptr<WindowAdapter> create_window_adapter() const = 0;

    /// Returns the amount of milliseconds since start of the application.
    ///
    /// This function should only be implemented  if the runtime is compiled with no_std
    virtual uint64_t duration_since_start() const { return 0; }

    /// Register the platform to Slint. Must be called before Slint window are created. Can only
    /// be called once in an application.
    static void register_platform(std::unique_ptr<Platform> platform)
    {
        cbindgen_private::slint_platform_register(
                platform.release(), [](void *p) { delete reinterpret_cast<const Platform *>(p); },
                [](void *p, cbindgen_private::WindowAdapterRcOpaque *out) {
                    auto w = reinterpret_cast<const Platform *>(p)->create_window_adapter();
                    *out = w->initialize();
                    (void)w.release();
                },
                [](void *p) -> uint64_t {
                    return reinterpret_cast<const Platform *>(p)->duration_since_start();
                });
    }
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
    virtual ~SoftwareRenderer() { cbindgen_private::slint_software_renderer_drop(inner); };
    SoftwareRenderer(const SoftwareRenderer &) = delete;
    SoftwareRenderer &operator=(const SoftwareRenderer &) = delete;
    SoftwareRenderer(int max_buffer_age)
    {
        inner = cbindgen_private::slint_software_renderer_new(max_buffer_age);
    }

    /// Render the window scene into a pixel buffer
    ///
    /// The buffer must be at least as large as the associated slint::Window
    ///
    /// The stride is the amount of pixels between two lines in the buffer.
    /// It is must be at least as large as the width of the window.
    void render(const Window &window, std::span<slint::Rgb8Pixel> buffer,
                std::size_t pixel_stride) const
    {
        cbindgen_private::slint_software_renderer_render_rgb8(
                inner, &window.window_handle().inner, buffer.data(), buffer.size(), pixel_stride);
    }
};

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
    NativeWindowHandle(NativeWindowHandle &&other) { inner = std::exchange(other.inner, nullptr); }
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

#    if !defined(__APPLE__) && !defined(_WIN32) && !defined(_WIN64)
    static NativeWindowHandle from_x11_xcb(uint32_t /*xcb_window_t*/ window,
                                           uint32_t /*xcb_visualid_t*/ visual_id,
                                           xcb_connection_t *connection, int screen)
    {

        return { cbindgen_private::slint_new_raw_window_handle_x11_xcb(window, visual_id,
                                                                       connection, screen) };
    }

    static NativeWindowHandle from_x11_xlib(uint32_t /*Window*/ window,
                                            unsigned long /*VisualID*/ visual_id,
                                            void /*Display*/ *display, int screen)
    {

        return { cbindgen_private::slint_new_raw_window_handle_x11_xlib(window, visual_id, display,
                                                                        screen) };
    }

    static NativeWindowHandle from_wayland(wl_surface *surface, wl_display *display)
    {

        return { cbindgen_private::slint_new_raw_window_handle_wayland(surface, display) };
    }

#    elif defined(__APPLE__) && !defined(_WIN32) && !defined(_WIN64)

    static NativeWindowHandle from_appkit(void *nsview, void *nswindow)
    {

        return { cbindgen_private::slint_new_raw_window_handle_appkit(nsview, nswindow) };
    }

#    elif !defined(__APPLE__) && (defined(_WIN32) || !defined(_WIN64))

    /// Windows handle
    static NativeWindowHandle from_win32(void *hwnd, void *hinstance)
    {
        return { cbindgen_private::slint_new_raw_window_handle_win32(hwnd, hinstance) };
    }
#    endif
    ~NativeWindowHandle()
    {
        if (inner) {
            cbindgen_private::slint_raw_window_handle_drop(inner);
        }
    }
};

/// Slint's Skia renderer.
///
/// To be used as a template parameter of the WindowAdapter.
///
/// The show() and hide() function must be called from the WindowAdapter's re-implementation
/// of the homonymous functions
///
/// Use render to perform the rendering.
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
    SkiaRenderer(const NativeWindowHandle &window_handle, PhysicalSize initial_size)
    {
        inner = cbindgen_private::slint_skia_renderer_new(window_handle.inner, initial_size);
    }

    void render(const Window &window) const
    {
        cbindgen_private::slint_skia_renderer_render(inner, &window.window_handle().inner);
    }

    void resize(PhysicalSize size) const
    {
        cbindgen_private::slint_skia_renderer_resize(inner, size);
    }

    void hide() const { cbindgen_private::slint_skia_renderer_hide(inner); }

    void show() const { cbindgen_private::slint_skia_renderer_show(inner); }
};

/// Call this function at each iteration of the event loop to call the timer handler and advance
/// the animations.  This should be called before the rendering or processing input events
inline void update_timers_and_animations()
{
    cbindgen_private::slint_platform_update_timers_and_animations();
}

}
}
}
#endif
