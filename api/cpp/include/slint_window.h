// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "slint_internal.h"

#include <utility>

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
#if !defined(DOXYGEN)
namespace platform {
class SkiaRenderer;
class SoftwareRenderer;
}
#endif

namespace private_api {
using ItemTreeRc = vtable::VRc<cbindgen_private::ItemTreeVTable>;
using slint::LogicalPosition;

/// Looking forward for C++23 std::optional::transform
template<typename T, typename F>
auto optional_transform(const std::optional<T> &o, F &&f) -> decltype(std::optional(f(*o)))
{
    if (o) {
        return std::optional(f(*o));
    }
    return std::nullopt;
}

template<typename T, typename F>
void optional_then(const std::optional<T> &o, F &&f)
{
    if (o) {
        f(*o);
    }
}

/// Waiting for C++23 std::optional::and_then
template<typename T, typename F>
auto optional_and_then(const std::optional<T> &o, F &&f) -> decltype(f(*o))
{
    if (o) {
        return f(*o);
    }
    return std::nullopt;
}

template<typename T>
T optional_or_default(const std::optional<T> &o)
{
    if (o) {
        return *o;
    }
    return {};
}

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
    void set_const_scale_factor(float value) const
    {
        slint_windowrc_set_const_scale_factor(&inner, value);
    }

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

#ifdef SLINT_FEATURE_RAW_WINDOW_HANDLE_06
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

    /// Returns the wl_surface from this NativeWindowHandle.
    wl_surface *wayland_surface() const
    {
        if (inner) {
            return static_cast<wl_surface *>(
                    cbindgen_private::slint_raw_window_handle_wayland(inner));
        }
        return nullptr;
    }

    /// Returns the wl_display from this NativeWindowHandle.
    wl_display *wayland_display() const
    {
        if (inner) {
            return static_cast<wl_display *>(
                    cbindgen_private::slint_raw_display_handle_wayland(inner));
        }
        return nullptr;
    }

#    endif
#    if (defined(__APPLE__) && !defined(_WIN32) && !defined(_WIN64)) || defined(DOXYGEN)

    /// Creates a new NativeWindowHandle from the given \a nsview, and \a nswindow.
    static NativeWindowHandle from_appkit(NSView *nsview, NSWindow *nswindow)
    {

        return { cbindgen_private::slint_new_raw_window_handle_appkit(nsview, nswindow) };
    }

    /// Returns the NSView from this NativeWindowHandle.
    NSView *appkit_view() const
    {
        if (inner) {
            return static_cast<NSView *>(cbindgen_private::slint_raw_view_handle_appkit(inner));
        }
        return nullptr;
    }

#    endif
#    if (!defined(__APPLE__) && (defined(_WIN32) || defined(_WIN64))) || defined(DOXYGEN)

    /// Creates a new NativeWindowHandle from the given HWND \a hwnd, and HINSTANCE \a hinstance.
    static NativeWindowHandle from_win32(void *hwnd, void *hinstance)
    {
        return { cbindgen_private::slint_new_raw_window_handle_win32(hwnd, hinstance) };
    }

    /// Returns the HWND from this NativeWindowHandle.
    void const *win32_hwnd() const
    {
        if (inner) {
            return cbindgen_private::slint_raw_hwnd_handle_win32(inner);
        }
        return nullptr;
    }

    /// Returns the HINSTANCE from this NativeWindowHandle.
    void const *win32_instance() const
    {
        if (inner) {
            return cbindgen_private::slint_raw_hinstance_handle_win32(inner);
        }
        return nullptr;
    }

#    endif
    /// Destroys the NativeWindowHandle.
    ~NativeWindowHandle()
    {
        if (inner) {
            cbindgen_private::slint_raw_window_handle_drop(inner);
        }
    }

protected:
    NativeWindowHandle(cbindgen_private::CppRawHandleOpaque inner) : inner(inner) { }

    friend class Window;
};
#endif

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
                slint::cbindgen_private::MouseEvent::Pressed({ pos.x, pos.y }, button, 0, false));
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
                slint::cbindgen_private::MouseEvent::Released({ pos.x, pos.y }, button, 0, false));
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
        inner.dispatch_pointer_event(
                slint::cbindgen_private::MouseEvent::Moved({ pos.x, pos.y }, false));
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

    NativeWindowHandle native_window_handle() const
    {
        return cbindgen_private::slint_windowrc_window_handle(&inner.handle());
    }

    /// \private
    private_api::WindowAdapterRc &window_handle() { return inner; }
    /// \private
    const private_api::WindowAdapterRc &window_handle() const { return inner; }

private:
    private_api::WindowAdapterRc inner;
};

}
