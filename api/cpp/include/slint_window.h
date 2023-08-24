// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#pragma once

#include "slint_internal.h"

#ifndef SLINT_FEATURE_FREESTANDING
#    include <thread>
#    include <iostream>
#endif

namespace slint {
#if !defined(DOXYGEN)
namespace platform {
class SkiaRenderer;
class SoftwareRenderer;
}
#endif

namespace private_api {
/// Internal function that checks that the API that must be called from the main
/// thread is indeed called from the main thread, or abort the program otherwise
///
/// Most API should be called from the main thread. When using thread one must
/// use slint::invoke_from_event_loop
inline void assert_main_thread()
{
#ifndef SLINT_FEATURE_FREESTANDING
#    ifndef NDEBUG
    static auto main_thread_id = std::this_thread::get_id();
    if (main_thread_id != std::this_thread::get_id()) {
        std::cerr << "A function that should be only called from the main thread was called from a "
                     "thread."
                  << std::endl;
        std::cerr << "Most API should be called from the main thread. When using thread one must "
                     "use slint::invoke_from_event_loop."
                  << std::endl;
        std::abort();
    }
#    endif
#endif
}

using ComponentRc = vtable::VRc<cbindgen_private::ComponentVTable>;

class WindowAdapterRc
{
public:
    explicit WindowAdapterRc(cbindgen_private::WindowAdapterRcOpaque adopted_inner)
    {
        assert_main_thread();
        cbindgen_private::slint_windowrc_clone(&adopted_inner, &inner);
    }
    WindowAdapterRc() { cbindgen_private::slint_windowrc_init(&inner); }
    ~WindowAdapterRc() { cbindgen_private::slint_windowrc_drop(&inner); }
    WindowAdapterRc(const WindowAdapterRc &other) : WindowAdapterRc(other.inner) { }
    WindowAdapterRc(WindowAdapterRc &&) = delete;
    WindowAdapterRc &operator=(WindowAdapterRc &&) = delete;
    WindowAdapterRc &operator=(const WindowAdapterRc &other)
    {
        assert_main_thread();
        if (this != &other) {
            cbindgen_private::slint_windowrc_drop(&inner);
            cbindgen_private::slint_windowrc_clone(&other.inner, &inner);
        }
        return *this;
    }

    void show() const { slint_windowrc_show(&inner); }
    void hide() const { slint_windowrc_hide(&inner); }
    bool is_visible() const { return slint_windowrc_is_visible(&inner); }

    float scale_factor() const { return slint_windowrc_get_scale_factor(&inner); }
    void set_scale_factor(float value) const { slint_windowrc_set_scale_factor(&inner, value); }

    bool dark_color_scheme() const { return slint_windowrc_dark_color_scheme(&inner); }

    bool text_input_focused() const { return slint_windowrc_get_text_input_focused(&inner); }
    void set_text_input_focused(bool value) const
    {
        slint_windowrc_set_text_input_focused(&inner, value);
    }

    template<typename Component, typename ItemArray>
    void unregister_component(Component *c, ItemArray items) const
    {
        cbindgen_private::slint_unregister_component(
                vtable::VRef<cbindgen_private::ComponentVTable> { &Component::static_vtable, c },
                items, &inner);
    }

    void set_focus_item(const ComponentRc &component_rc, uintptr_t item_index)
    {
        cbindgen_private::ItemRc item_rc { component_rc, item_index };
        cbindgen_private::slint_windowrc_set_focus_item(&inner, &item_rc);
    }

    template<typename Component>
    void set_component(const Component &c) const
    {
        auto self_rc = (*c.self_weak.lock()).into_dyn();
        slint_windowrc_set_component(&inner, &self_rc);
    }

    template<typename Component, typename Parent>
    void show_popup(const Parent *parent_component, cbindgen_private::Point p, bool close_on_click,
                    cbindgen_private::ItemRc parent_item) const
    {
        auto popup = Component::create(parent_component).into_dyn();
        cbindgen_private::slint_windowrc_show_popup(&inner, &popup, p, close_on_click,
                                                    &parent_item);
    }

    void close_popup() const { cbindgen_private::slint_windowrc_close_popup(&inner); }

    template<std::invocable<RenderingState, GraphicsAPI> F>
    std::optional<SetRenderingNotifierError> set_rendering_notifier(F callback) const
    {
        auto actual_cb = [](RenderingState state, GraphicsAPI graphics_api, void *data) {
            (*reinterpret_cast<F *>(data))(state, graphics_api);
        };
        SetRenderingNotifierError err;
        if (cbindgen_private::slint_windowrc_set_rendering_notifier(
                    &inner, actual_cb,
                    [](void *user_data) { delete reinterpret_cast<F *>(user_data); },
                    new F(std::move(callback)), &err)) {
            return {};
        } else {
            return err;
        }
    }

    // clang-format off
    template<std::invocable F>
        requires(std::is_convertible_v<std::invoke_result_t<F>, CloseRequestResponse>)
    void on_close_requested(F callback) const
    // clang-format on
    {
        auto actual_cb = [](void *data) { return (*reinterpret_cast<F *>(data))(); };
        cbindgen_private::slint_windowrc_on_close_requested(
                &inner, actual_cb, [](void *user_data) { delete reinterpret_cast<F *>(user_data); },
                new F(std::move(callback)));
    }

    void request_redraw() const { cbindgen_private::slint_windowrc_request_redraw(&inner); }

    slint::PhysicalPosition position() const
    {
        slint::PhysicalPosition pos;
        cbindgen_private::slint_windowrc_position(&inner, &pos);
        return pos;
    }

    void set_logical_position(const slint::LogicalPosition &pos)
    {
        cbindgen_private::slint_windowrc_set_logical_position(&inner, &pos);
    }

    void set_physical_position(const slint::PhysicalPosition &pos)
    {
        cbindgen_private::slint_windowrc_set_physical_position(&inner, &pos);
    }

    slint::PhysicalSize size() const
    {
        return slint::PhysicalSize(cbindgen_private::slint_windowrc_size(&inner));
    }

    void set_logical_size(const slint::LogicalSize &size)
    {
        cbindgen_private::slint_windowrc_set_logical_size(&inner, &size);
    }

    void set_physical_size(const slint::PhysicalSize &size)
    {
        cbindgen_private::slint_windowrc_set_physical_size(&inner, &size);
    }

    void dispatch_key_event(const cbindgen_private::KeyInputEvent &event)
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_windowrc_dispatch_key_event(&inner, &event);
    }

    /// Send a pointer event to this window
    void dispatch_pointer_event(const cbindgen_private::MouseEvent &event)
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_windowrc_dispatch_pointer_event(&inner, event);
    }

    /// Registers a font by the specified path. The path must refer to an existing
    /// TrueType font.
    /// \returns an empty optional on success, otherwise an error string
    inline std::optional<SharedString> register_font_from_path(const SharedString &path)
    {
        SharedString maybe_err;
        cbindgen_private::slint_register_font_from_path(&inner, &path, &maybe_err);
        if (!maybe_err.empty()) {
            return maybe_err;
        } else {
            return {};
        }
    }

    /// Registers a font by the data. The data must be valid TrueType font data.
    /// \returns an empty optional on success, otherwise an error string
    inline std::optional<SharedString> register_font_from_data(const uint8_t *data, std::size_t len)
    {
        SharedString maybe_err;
        cbindgen_private::slint_register_font_from_data(
                &inner, { const_cast<uint8_t *>(data), len }, &maybe_err);
        if (!maybe_err.empty()) {
            return maybe_err;
        } else {
            return {};
        }
    }

    /// Registers a bitmap font for use with the software renderer.
    inline void register_bitmap_font(const cbindgen_private::BitmapFont &font)
    {
        cbindgen_private::slint_register_bitmap_font(&inner, &font);
    }

    /// \private
    const cbindgen_private::WindowAdapterRcOpaque &handle() const { return inner; }

private:
    friend class slint::platform::SkiaRenderer;
    friend class slint::platform::SoftwareRenderer;
    cbindgen_private::WindowAdapterRcOpaque inner;
};

}

/// This class represents a window towards the windowing system, that's used to render the
/// scene of a component. It provides API to control windowing system specific aspects such
/// as the position on the screen.
class Window
{
public:
    /// \private
    /// Internal function used by the generated code to construct a new instance of this
    /// public API wrapper.
    explicit Window(const private_api::WindowAdapterRc &windowrc) : inner(windowrc) { }
    Window(const Window &other) = delete;
    Window &operator=(const Window &other) = delete;
    Window(Window &&other) = delete;
    Window &operator=(Window &&other) = delete;
    /// Destroys this window. Window instances are explicitly shared and reference counted.
    /// If this window instance is the last one referencing the window towards the windowing
    /// system, then it will also become hidden and destroyed.
    ~Window() = default;

    /// Registers the window with the windowing system in order to make it visible on the screen.
    void show() { inner.show(); }
    /// De-registers the window from the windowing system, therefore hiding it.
    void hide() { inner.hide(); }

    /// Returns the visibility state of the window. This function can return false even if you
    /// previously called show() on it, for example if the user minimized the window.
    bool is_visible() const { return inner.is_visible(); }

    /// This function allows registering a callback that's invoked during the different phases of
    /// rendering. This allows custom rendering on top or below of the scene.
    ///
    /// The provided callback must be callable with a slint::RenderingState and the
    /// slint::GraphicsAPI argument.
    ///
    /// On success, the function returns a std::optional without value. On error, the function
    /// returns the error code as value in the std::optional.
    template<std::invocable<RenderingState, GraphicsAPI> F>
    std::optional<SetRenderingNotifierError> set_rendering_notifier(F &&callback) const
    {
        return inner.set_rendering_notifier(std::forward<F>(callback));
    }

    /// This function allows registering a callback that's invoked when the user tries to close
    /// a window.
    /// The callback has to return a CloseRequestResponse.
    // clang-format off
    template<std::invocable F>
        requires(std::is_convertible_v<std::invoke_result_t<F>, CloseRequestResponse>)
    void on_close_requested(F &&callback) const
    // clang-format on
    {
        return inner.on_close_requested(std::forward<F>(callback));
    }

    /// This function issues a request to the windowing system to redraw the contents of the window.
    void request_redraw() const { inner.request_redraw(); }

    /// Returns the position of the window on the screen, in physical screen coordinates and
    /// including a window frame (if present).
    slint::PhysicalPosition position() const { return inner.position(); }

    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    /// Note that on some windowing systems, such as Wayland, this functionality is not available.
    void set_position(const slint::LogicalPosition &pos) { inner.set_logical_position(pos); }
    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    /// Note that on some windowing systems, such as Wayland, this functionality is not available.
    void set_position(const slint::PhysicalPosition &pos) { inner.set_physical_position(pos); }

    /// Returns the size of the window on the screen, in physical screen coordinates and excluding
    /// a window frame (if present).
    slint::PhysicalSize size() const { return inner.size(); }

    /// Resizes the window to the specified size on the screen, in logical pixels and excluding
    /// a window frame (if present).
    void set_size(const slint::LogicalSize &size) { inner.set_logical_size(size); }
    /// Resizes the window to the specified size on the screen, in physical pixels and excluding
    /// a window frame (if present).
    void set_size(const slint::PhysicalSize &size) { inner.set_physical_size(size); }

    /// This function returns the scale factor that allows converting between logical and
    /// physical pixels.
    float scale_factor() const { return inner.scale_factor(); }

    /// Dispatch a key press event to the scene.
    ///
    /// Use this when you're implementing your own backend and want to forward user input events.
    ///
    /// The \a text is the unicode representation of the key.
    void dispatch_key_press_event(const SharedString &text)
    {
        cbindgen_private::KeyInputEvent event { text, cbindgen_private::KeyEventType::KeyPressed, 0,
                                                0 };
        inner.dispatch_key_event(event);
    }

    /// Dispatch a key release event to the scene.
    ///
    /// Use this when you're implementing your own backend and want to forward user input events.
    ///
    /// The \a text is the unicode representation of the key.
    void dispatch_key_release_event(const SharedString &text)
    {
        cbindgen_private::KeyInputEvent event { text, cbindgen_private::KeyEventType::KeyReleased,
                                                0, 0 };
        inner.dispatch_key_event(event);
    }

    /// Dispatches a pointer or mouse press event to the scene.
    ///
    /// Use this function when you're implementing your own backend and want to forward user
    /// pointer/mouse events.
    ///
    /// \a pos represents the logical position of the pointer relative to the window.
    /// \a button is the button that was pressed.
    void dispatch_pointer_press_event(LogicalPosition pos, PointerEventButton button)
    {
        using slint::cbindgen_private::MouseEvent;
        MouseEvent event { .tag = MouseEvent::Tag::Pressed,
                           .pressed = MouseEvent::Pressed_Body { .position = { pos.x, pos.y },
                                                                 .button = button,
                                                                 .click_count = 0 } };
        inner.dispatch_pointer_event(event);
    }
    /// Dispatches a pointer or mouse release event to the scene.
    ///
    /// Use this function when you're implementing your own backend and want to forward user
    /// pointer/mouse events.
    ///
    /// \a pos represents the logical position of the pointer relative to the window.
    /// \a button is the button that was released.
    void dispatch_pointer_release_event(LogicalPosition pos, PointerEventButton button)
    {
        using slint::cbindgen_private::MouseEvent;
        MouseEvent event { .tag = MouseEvent::Tag::Released,
                           .released = MouseEvent::Released_Body { .position = { pos.x, pos.y },
                                                                   .button = button,
                                                                   .click_count = 0 } };
        inner.dispatch_pointer_event(event);
    }
    /// Dispatches a pointer exit event to the scene.
    ///
    /// Use this function when you're implementing your own backend and want to forward user
    /// pointer/mouse events.
    ///
    /// This event is triggered when the pointer exits the window.
    void dispatch_pointer_exit_event()
    {
        using slint::cbindgen_private::MouseEvent;
        MouseEvent event { .tag = MouseEvent::Tag::Exit, .moved = {} };
        inner.dispatch_pointer_event(event);
    }

    /// Dispatches a pointer move event to the scene.
    ///
    /// Use this function when you're implementing your own backend and want to forward user
    /// pointer/mouse events.
    ///
    /// \a pos represents the logical position of the pointer relative to the window.
    void dispatch_pointer_move_event(LogicalPosition pos)
    {
        using slint::cbindgen_private::MouseEvent;
        MouseEvent event { .tag = MouseEvent::Tag::Moved,
                           .moved = MouseEvent::Moved_Body { .position = { pos.x, pos.y } } };
        inner.dispatch_pointer_event(event);
    }

    /// Dispatches a scroll (or wheel) event to the scene.
    ///
    /// Use this function when you're implementing your own backend and want to forward user wheel
    /// events.
    ///
    /// \a parameter represents the logical position of the pointer relative to the window.
    /// \a delta_x and \a delta_y represent the scroll delta values in the X and Y
    /// directions in logical pixels.
    void dispatch_pointer_scroll_event(LogicalPosition pos, float delta_x, float delta_y)
    {
        using slint::cbindgen_private::MouseEvent;
        MouseEvent event { .tag = MouseEvent::Tag::Wheel,
                           .wheel = MouseEvent::Wheel_Body { .position = { pos.x, pos.y },
                                                             .delta_x = delta_x,
                                                             .delta_y = delta_y } };
        inner.dispatch_pointer_event(event);
    }

    /// Set the logical size of this window after a resize event
    ///
    /// The backend must send this event to ensure that the `width` and `height` property of the
    /// root Window element are properly set.
    void dispatch_resize_event(slint::LogicalSize s)
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_windowrc_dispatch_resize_event(&inner.handle(), s.width, s.height);
    }

    /// The window's scale factor has changed. This can happen for example when the display's
    /// resolution changes, the user selects a new scale factor in the system settings, or the
    /// window is moved to a different screen. Platform implementations should dispatch this event
    /// also right after the initial window creation, to set the initial scale factor the windowing
    /// system provided for the window.
    void dispatch_scale_factor_change_event(float factor)
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_windowrc_dispatch_scale_factor_change_event(&inner.handle(),
                                                                            factor);
    }

    /// Returns true if there is an animation currently active on any property in the Window.
    bool has_active_animations() const
    {
        private_api::assert_main_thread();
        return cbindgen_private::slint_windowrc_has_active_animations(&inner.handle());
    }

    /// \private
    private_api::WindowAdapterRc &window_handle() { return inner; }
    /// \private
    const private_api::WindowAdapterRc &window_handle() const { return inner; }

private:
    private_api::WindowAdapterRc inner;
};

}