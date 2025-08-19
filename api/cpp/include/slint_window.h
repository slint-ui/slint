// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "slint_internal.h"

namespace slint {
#if !defined(DOXYGEN)
namespace platform {
class SkiaRenderer;
class SoftwareRenderer;
}
#endif

namespace private_api {
using ItemTreeRc = vtable::VRc<cbindgen_private::ItemTreeVTable>;
using slint::LogicalPosition;

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

    cbindgen_private::ColorScheme color_scheme() const
    {
        return slint_windowrc_color_scheme(&inner);
    }
    bool supports_native_menu_bar() const
    {
        return slint_windowrc_supports_native_menu_bar(&inner);
    }

    bool text_input_focused() const { return slint_windowrc_get_text_input_focused(&inner); }
    void set_text_input_focused(bool value) const
    {
        slint_windowrc_set_text_input_focused(&inner, value);
    }

    template<typename Component, typename ItemArray>
    void unregister_item_tree(Component *c, ItemArray items) const
    {
        cbindgen_private::slint_unregister_item_tree(
                vtable::VRef<cbindgen_private::ItemTreeVTable> { &Component::static_vtable, c },
                items, &inner);
    }

    void set_focus_item(const ItemTreeRc &component_rc, uint32_t item_index, bool set_focus,
                        cbindgen_private::FocusReason reason)
    {
        cbindgen_private::ItemRc item_rc { component_rc, item_index };
        cbindgen_private::slint_windowrc_set_focus_item(&inner, &item_rc, set_focus, reason);
    }

    void set_component(const cbindgen_private::ItemTreeWeak &weak) const
    {
        auto item_tree_rc = (*weak.lock()).into_dyn();
        slint_windowrc_set_component(&inner, &item_tree_rc);
    }

    template<typename Component, typename Parent, typename PosGetter>
    uint32_t show_popup(const Parent *parent_component, PosGetter pos,
                        cbindgen_private::PopupClosePolicy close_policy,
                        cbindgen_private::ItemRc parent_item) const
    {
        auto popup = Component::create(parent_component);
        auto p = pos(popup);
        auto popup_dyn = popup.into_dyn();
        auto id = cbindgen_private::slint_windowrc_show_popup(&inner, &popup_dyn, p, close_policy,
                                                              &parent_item, false);
        popup->user_init();
        return id;
    }

    void close_popup(uint32_t popup_id) const
    {
        if (popup_id > 0) {
            cbindgen_private::slint_windowrc_close_popup(&inner, popup_id);
        }
    }

    template<typename Component, typename SharedGlobals, typename InitFn>
    uint32_t show_popup_menu(
            SharedGlobals *globals, LogicalPosition pos, cbindgen_private::ItemRc context_menu_rc,
            InitFn init,
            std::optional<vtable::VRc<cbindgen_private::MenuVTable>> menu = std::nullopt) const
    {
        if (menu) {
            if (cbindgen_private::slint_windowrc_show_native_popup_menu(&inner, &menu.value(), pos,
                                                                        &context_menu_rc)) {
                return 0;
            }
        }

        auto popup = Component::create(globals);
        init(&*popup);
        auto popup_dyn = popup.into_dyn();
        auto id = cbindgen_private::slint_windowrc_show_popup(
                &inner, &popup_dyn, pos, cbindgen_private::PopupClosePolicy::CloseOnClickOutside,
                &context_menu_rc, true);
        popup->user_init();
        return id;
    }

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

    /// Send a pointer event to this window
    void dispatch_pointer_event(const cbindgen_private::MouseEvent &event)
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_windowrc_dispatch_pointer_event(&inner, &event);
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
        cbindgen_private::slint_register_font_from_data(&inner, make_slice(data, len), &maybe_err);
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

    /// Shows the window on the screen. An additional strong reference on the
    /// associated component is maintained while the window is visible.
    ///
    /// Call hide() to make the window invisible again, and drop the additional
    /// strong reference.
    void show()
    {
        private_api::assert_main_thread();
        inner.show();
    }
    /// Hides the window, so that it is not visible anymore. The additional strong
    /// reference on the associated component, that was created when show() was called, is
    /// dropped.
    void hide()
    {
        private_api::assert_main_thread();
        inner.hide();
    }

    /// Returns the visibility state of the window. This function can return false even if you
    /// previously called show() on it, for example if the user minimized the window.
    bool is_visible() const
    {
        private_api::assert_main_thread();
        return inner.is_visible();
    }

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
        private_api::assert_main_thread();
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
        private_api::assert_main_thread();
        return inner.on_close_requested(std::forward<F>(callback));
    }

    /// This function issues a request to the windowing system to redraw the contents of the window.
    void request_redraw() const
    {
        private_api::assert_main_thread();
        inner.request_redraw();
    }

    /// Returns the position of the window on the screen, in physical screen coordinates and
    /// including a window frame (if present).
    slint::PhysicalPosition position() const
    {
        private_api::assert_main_thread();
        return inner.position();
    }

    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    /// Note that on some windowing systems, such as Wayland, this functionality is not available.
    void set_position(const slint::LogicalPosition &pos)
    {
        private_api::assert_main_thread();
        inner.set_logical_position(pos);
    }
    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    /// Note that on some windowing systems, such as Wayland, this functionality is not available.
    void set_position(const slint::PhysicalPosition &pos)
    {
        private_api::assert_main_thread();
        inner.set_physical_position(pos);
    }

    /// Returns the size of the window on the screen, in physical screen coordinates and excluding
    /// a window frame (if present).
    slint::PhysicalSize size() const
    {
        private_api::assert_main_thread();
        return inner.size();
    }

    /// Resizes the window to the specified size on the screen, in logical pixels and excluding
    /// a window frame (if present).
    void set_size(const slint::LogicalSize &size)
    {
        private_api::assert_main_thread();
        inner.set_logical_size(size);
    }
    /// Resizes the window to the specified size on the screen, in physical pixels and excluding
    /// a window frame (if present).
    void set_size(const slint::PhysicalSize &size)
    {
        private_api::assert_main_thread();
        inner.set_physical_size(size);
    }

    /// This function returns the scale factor that allows converting between logical and
    /// physical pixels.
    float scale_factor() const
    {
        private_api::assert_main_thread();
        return inner.scale_factor();
    }

    /// Returns if the window is currently fullscreen
    bool is_fullscreen() const
    {
        private_api::assert_main_thread();
        return cbindgen_private::slint_windowrc_is_fullscreen(&inner.handle());
    }
    /// Set or unset the window to display fullscreen.
    void set_fullscreen(bool fullscreen)
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_windowrc_set_fullscreen(&inner.handle(), fullscreen);
    }

    /// Returns if the window is currently maximized
    bool is_maximized() const
    {
        private_api::assert_main_thread();
        return cbindgen_private::slint_windowrc_is_maximized(&inner.handle());
    }
    /// Maximize or unmaximize the window.
    void set_maximized(bool maximized)
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_windowrc_set_maximized(&inner.handle(), maximized);
    }

    /// Returns if the window is currently minimized
    bool is_minimized() const
    {
        private_api::assert_main_thread();
        return cbindgen_private::slint_windowrc_is_minimized(&inner.handle());
    }
    /// Minimize or unminimze the window.
    void set_minimized(bool minimized)
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_windowrc_set_minimized(&inner.handle(), minimized);
    }

    /// Dispatch a key press event to the scene.
    ///
    /// Use this when you're implementing your own backend and want to forward user input events.
    ///
    /// The \a text is the unicode representation of the key.
    void dispatch_key_press_event(const SharedString &text)
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_windowrc_dispatch_key_event(
                &inner.handle(), cbindgen_private::KeyEventType::KeyPressed, &text, false);
    }

    /// Dispatch an auto-repeated key press event to the scene.
    ///
    /// Use this when you're implementing your own backend and want to forward user input events.
    ///
    /// The \a text is the unicode representation of the key.
    void dispatch_key_press_repeat_event(const SharedString &text)
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_windowrc_dispatch_key_event(
                &inner.handle(), cbindgen_private::KeyEventType::KeyPressed, &text, true);
    }

    /// Dispatch a key release event to the scene.
    ///
    /// Use this when you're implementing your own backend and want to forward user input events.
    ///
    /// The \a text is the unicode representation of the key.
    void dispatch_key_release_event(const SharedString &text)
    {
        private_api::assert_main_thread();
        cbindgen_private::slint_windowrc_dispatch_key_event(
                &inner.handle(), cbindgen_private::KeyEventType::KeyReleased, &text, false);
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
        private_api::assert_main_thread();
        inner.dispatch_pointer_event(
                slint::cbindgen_private::MouseEvent::Pressed({ pos.x, pos.y }, button, 0));
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
        private_api::assert_main_thread();
        inner.dispatch_pointer_event(
                slint::cbindgen_private::MouseEvent::Released({ pos.x, pos.y }, button, 0));
    }
    /// Dispatches a pointer exit event to the scene.
    ///
    /// Use this function when you're implementing your own backend and want to forward user
    /// pointer/mouse events.
    ///
    /// This event is triggered when the pointer exits the window.
    void dispatch_pointer_exit_event()
    {
        private_api::assert_main_thread();
        inner.dispatch_pointer_event(slint::cbindgen_private::MouseEvent::Exit());
    }

    /// Dispatches a pointer move event to the scene.
    ///
    /// Use this function when you're implementing your own backend and want to forward user
    /// pointer/mouse events.
    ///
    /// \a pos represents the logical position of the pointer relative to the window.
    void dispatch_pointer_move_event(LogicalPosition pos)
    {
        private_api::assert_main_thread();
        inner.dispatch_pointer_event(slint::cbindgen_private::MouseEvent::Moved({ pos.x, pos.y }));
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
        private_api::assert_main_thread();
        inner.dispatch_pointer_event(
                slint::cbindgen_private::MouseEvent::Wheel({ pos.x, pos.y }, delta_x, delta_y));
    }

    /// Set the logical size of this window after a resize event
    ///
    /// The backend must send this event to ensure that the `width` and `height` property of the
    /// root Window element are properly set.
    void dispatch_resize_event(slint::LogicalSize s)
    {
        private_api::assert_main_thread();
        using slint::cbindgen_private::WindowEvent;
        WindowEvent event { .resized =
                                    WindowEvent::Resized_Body { .tag = WindowEvent::Tag::Resized,
                                                                .size = { s.width, s.height } } };
        cbindgen_private::slint_windowrc_dispatch_event(&inner.handle(), &event);
    }

    /// The window's scale factor has changed. This can happen for example when the display's
    /// resolution changes, the user selects a new scale factor in the system settings, or the
    /// window is moved to a different screen. Platform implementations should dispatch this event
    /// also right after the initial window creation, to set the initial scale factor the windowing
    /// system provided for the window.
    void dispatch_scale_factor_change_event(float factor)
    {
        private_api::assert_main_thread();
        using slint::cbindgen_private::WindowEvent;
        WindowEvent event { .scale_factor_changed = WindowEvent::ScaleFactorChanged_Body {
                                    .tag = WindowEvent::Tag::ScaleFactorChanged,
                                    .scale_factor = factor } };
        cbindgen_private::slint_windowrc_dispatch_event(&inner.handle(), &event);
    }

    /// The Window was activated or de-activated.
    ///
    /// The backend should dispatch this event with true when the window gains focus
    /// and false when the window loses focus.
    void dispatch_window_active_changed_event(bool active)
    {
        private_api::assert_main_thread();
        using slint::cbindgen_private::WindowEvent;
        WindowEvent event { .window_active_changed = WindowEvent::WindowActiveChanged_Body {
                                    .tag = WindowEvent::Tag::WindowActiveChanged, ._0 = active } };
        cbindgen_private::slint_windowrc_dispatch_event(&inner.handle(), &event);
    }

    /// The user requested to close the window.
    ///
    /// The backend should send this event when the user tries to close the window,for example by
    /// pressing the close button.
    ///
    /// This will have the effect of invoking the callback set in Window::on_close_requested() and
    /// then hiding the window depending on the return value of the callback.
    void dispatch_close_requested_event()
    {
        private_api::assert_main_thread();
        using slint::cbindgen_private::WindowEvent;
        WindowEvent event { .tag = WindowEvent::Tag::CloseRequested };
        cbindgen_private::slint_windowrc_dispatch_event(&inner.handle(), &event);
    }

    /// Returns true if there is an animation currently active on any property in the Window.
    bool has_active_animations() const
    {
        private_api::assert_main_thread();
        return cbindgen_private::slint_windowrc_has_active_animations(&inner.handle());
    }

    /// Takes a snapshot of the window contents and returns it as RGBA8 encoded pixel buffer.
    ///
    /// Note that this function may be slow to call as it may need to re-render the scene.
    std::optional<SharedPixelBuffer<Rgba8Pixel>> take_snapshot() const
    {
        SharedPixelBuffer<Rgba8Pixel> result;
        if (cbindgen_private::slint_windowrc_take_snapshot(&inner.handle(), &result.m_data,
                                                           &result.m_width, &result.m_height)) {
            return result;
        } else {
            return {};
        }
    }

    /// \private
    private_api::WindowAdapterRc &window_handle() { return inner; }
    /// \private
    const private_api::WindowAdapterRc &window_handle() const { return inner; }

private:
    private_api::WindowAdapterRc inner;
};

}
