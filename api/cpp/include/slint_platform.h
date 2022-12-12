// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#pragma once

#ifndef SLINT_FEATURE_EXPERIMENTAL
#    warning "slint_platform.h API only available when SLINT_FEATURE_EXPERIMENTAL is activated"
#else

#    include "slint.h"

namespace slint {

/// This namespace contains experimental API.
/// No compatibility across version.
///
/// \private
namespace experimental {

/// Namespace to be used when you implement your own Platform
namespace platform {

/// The Renderer is one of the Type provided by Slint to do the rendering of a scene.
///
/// See for example SoftwareRenderer

template<typename R>
concept Renderer = requires(R r)
{
    r.init(static_cast<const cbindgen_private::WindowAdapterRcOpaque *>(nullptr));
    cbindgen_private::RendererPtr { r.renderer_handle() };
};

template<Renderer R>
class Platform;

/// Base class for the layer between a slint::Window and the internal window from the platform
///
/// Re-implement this class to do the link between the two.
///
/// The R template parameter is the Renderer which is one of the renderer type provided by Slint

template<Renderer R>
class WindowAdapter
{
    friend class Platform<R>;
    // This is a pointer to the rust window that own us.
    // Note that we do not have ownership (there is no reference increase for this)
    // because it would otherwise be a reference loop
    cbindgen_private::WindowAdapterRcOpaque self {};
    // Whether this WindowAdapter was already given to the slint runtime
    const R m_renderer;
    bool was_initialized = false;

public:
    /// Construct a WindowAdapter.  The arguments are forwarded to initialize the renderer
    template<typename... Args>
    explicit WindowAdapter(Args... a) : m_renderer(std::forward<Args>(a)...)
    {
    }
    virtual ~WindowAdapter() = default;
    WindowAdapter(const WindowAdapter &) = delete;
    WindowAdapter &operator=(const WindowAdapter &) = delete;

    /// Return a reference to the renderer that can be used to do the rendering.
    const R &renderer() const { return m_renderer; }

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

    /// Return the slint::Window associated with this window.
    ///
    /// Note that this function can only be called if the window was initialized, which is only
    /// the case after it has been returned from a call to Platform::create_window_adaptor
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

    /// Send a key event to this window
    // Note: in rust, this is on the Window. FIXME: use a public event type
    void dispatch_key_event(const cbindgen_private::KeyInputEvent &event)
    {
        private_api::assert_main_thread();
        if (was_initialized) {
            cbindgen_private::slint_windowrc_dispatch_key_event(&self, event);
        }
    }

    /// Returns true if the window is currently animating
    bool has_active_animations() const
    {
        return cbindgen_private::slint_windowrc_has_active_animations(&self);
    }
};

/// The platform is acting like a factory to create a WindowAdapter<R>
///
/// Platform::register_platform() need to be called before any other Slint handle
/// are created, and if it is called, it will use the WindowAdapter provided by the
/// create_window_adaptor function.
template<Renderer R>
class Platform
{
public:
    virtual ~Platform() = default;
    Platform(const Platform &) = delete;
    Platform &operator=(const Platform &) = delete;
    Platform() = default;

    /// Returns a new WindowAdapter
    virtual std::unique_ptr<WindowAdapter<R>> create_window_adaptor() const = 0;

    /// Register the platform to Slint. Must be called before Slint window are created. Can only
    /// be called once in an application.
    static void register_platform(std::unique_ptr<Platform> platform)
    {
        using WA = WindowAdapter<R>;
        cbindgen_private::slint_platform_register(
                platform.release(), [](void *p) { delete reinterpret_cast<const Platform *>(p); },
                [](void *p, cbindgen_private::WindowAdapterRcOpaque *out) {
                    auto w = reinterpret_cast<const Platform *>(p)->create_window_adaptor();
                    auto w_ptr = w.release();
                    cbindgen_private::slint_window_adapter_new(
                            w_ptr, [](void *wa) { delete reinterpret_cast<const WA *>(wa); },
                            [](void *wa) {
                                return reinterpret_cast<const WA *>(wa)
                                        ->m_renderer.renderer_handle();
                            },
                            [](void *wa) { reinterpret_cast<const WA *>(wa)->show(); },
                            [](void *wa) { reinterpret_cast<const WA *>(wa)->hide(); },
                            [](void *wa) { reinterpret_cast<const WA *>(wa)->request_redraw(); },
                            out);
                    w_ptr->self = *out;
                    w_ptr->m_renderer.init(out);
                    w_ptr->was_initialized = true;
                });
    }
};

/// Slint's software renderer.
///
/// To be used as a template parameter of the WindowAdapter.
///
/// Use the render() function to render in a buffer
///
/// The MAX_BUFFER_AGE parameter specifies how many buffers are being re-used.
/// This means that the buffer passed to the render functions still contains a rendering of
/// the window that was refreshed as least that amount of frame ago.
/// It will impact how much of the screen needs to be redrawn.
template<int MAX_BUFFER_AGE = 0>
class SoftwareRenderer
{
    mutable cbindgen_private::SoftwareRendererOpaque inner;

public:
    virtual ~SoftwareRenderer()
    {
        if (inner) {
            cbindgen_private::slint_software_renderer_drop(MAX_BUFFER_AGE, inner);
        }
    };
    SoftwareRenderer(const SoftwareRenderer &) = delete;
    SoftwareRenderer &operator=(const SoftwareRenderer &) = delete;
    SoftwareRenderer() = default;

    /// \private
    void init(const cbindgen_private::WindowAdapterRcOpaque *win) const
    {
        if (inner) {
            cbindgen_private::slint_software_renderer_drop(MAX_BUFFER_AGE, inner);
        }
        inner = cbindgen_private::slint_software_renderer_new(MAX_BUFFER_AGE, win);
    }

    /// \private
    cbindgen_private::RendererPtr renderer_handle() const
    {
        return cbindgen_private::slint_software_renderer_handle(MAX_BUFFER_AGE, inner);
    }

    /// Render the window scene into a pixel buffer
    ///
    /// The buffer must be at least as large as the associated slint::Window
    ///
    /// The stride is the amount of pixels between two lines in the buffer.
    /// It is must be at least as large as the width of the window.
    void render(std::span<slint::cbindgen_private::Rgb8Pixel> buffer, std::size_t stride) const
    {
        cbindgen_private::slint_software_renderer_render_rgb8(MAX_BUFFER_AGE, inner, buffer.data(),
                                                              buffer.size(), stride);
    }
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
