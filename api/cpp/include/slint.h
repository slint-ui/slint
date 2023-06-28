// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

#pragma once

#if defined(__GNUC__) || defined(__clang__)
// In C++17, it is conditionally supported, but still valid for all compiler we care
#    pragma GCC diagnostic ignored "-Winvalid-offsetof"
#endif

#include <vector>
#include <memory>
#include <algorithm>
#include <iostream> // FIXME: remove: iostream always bring it lots of code so we should not have it in this header
#include <chrono>
#include <optional>
#include <thread>
#include <mutex>
#include <condition_variable>
#include <span>
#include <functional>
#include <concepts>

namespace slint::cbindgen_private {
// Workaround https://github.com/eqrion/cbindgen/issues/43
struct ComponentVTable;
struct ItemVTable;
}
#include "slint_internal.h"
#include "slint_size.h"
#include "slint_point.h"
#include "slint_backend_internal.h"
#include "slint_qt_internal.h"

/// \rst
/// The :code:`slint` namespace is the primary entry point into the Slint C++ API.
/// All available types are in this namespace.
///
/// See the :doc:`Overview <../overview>` documentation for the C++ integration how
/// to load :code:`.slint` designs.
/// \endrst
namespace slint {

// Bring opaque structure in scope
namespace private_api {
using cbindgen_private::ComponentVTable;
using cbindgen_private::ItemVTable;
using ComponentRc = vtable::VRc<private_api::ComponentVTable>;
using ComponentRef = vtable::VRef<private_api::ComponentVTable>;
using IndexRange = cbindgen_private::IndexRange;
using ItemRef = vtable::VRef<private_api::ItemVTable>;
using ItemVisitorRefMut = vtable::VRefMut<cbindgen_private::ItemVisitorVTable>;
using cbindgen_private::ComponentWeak;
using cbindgen_private::ItemWeak;
using cbindgen_private::TraversalOrder;
}

#if !defined(DOXYGEN)
namespace experimental {
namespace platform {
class SkiaRenderer;
class SoftwareRenderer;
}
}
#endif

namespace private_api {
using ItemTreeNode = cbindgen_private::ItemTreeNode;
using ItemArrayEntry =
        vtable::VOffset<uint8_t, slint::cbindgen_private::ItemVTable, vtable::AllowPin>;
using ItemArray = slint::cbindgen_private::Slice<ItemArrayEntry>;
using cbindgen_private::KeyboardModifiers;
using cbindgen_private::KeyEvent;
using cbindgen_private::PointerEvent;
using cbindgen_private::TableColumn;

/// Internal function that checks that the API that must be called from the main
/// thread is indeed called from the main thread, or abort the program otherwise
///
/// Most API should be called from the main thread. When using thread one must
/// use slint::invoke_from_event_loop
inline void assert_main_thread()
{
#ifndef NDEBUG
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
#endif
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
                vtable::VRef<ComponentVTable> { &Component::static_vtable, c }, items, &inner);
    }

    void set_focus_item(const ComponentRc &component_rc, uintptr_t item_index)
    {
        cbindgen_private::ItemRc item_rc { component_rc, item_index };
        cbindgen_private::slint_windowrc_set_focus_item(&inner, &item_rc);
    }

    template<typename Component, typename ItemArray>
    void register_component(Component *c, ItemArray items) const
    {
        cbindgen_private::slint_register_component(
                vtable::VRef<ComponentVTable> { &Component::static_vtable, c }, items, &inner);
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

    /// \private
    const cbindgen_private::WindowAdapterRcOpaque &handle() { return inner; }

private:
    friend class slint::experimental::platform::SkiaRenderer;
    friend class slint::experimental::platform::SoftwareRenderer;
    cbindgen_private::WindowAdapterRcOpaque inner;
};

constexpr inline ItemTreeNode make_item_node(uint32_t child_count, uint32_t child_index,
                                             uint32_t parent_index, uint32_t item_array_index,
                                             bool is_accessible)
{
    return ItemTreeNode { ItemTreeNode::Item_Body { ItemTreeNode::Tag::Item, is_accessible,
                                                    child_count, child_index, parent_index,
                                                    item_array_index } };
}

constexpr inline ItemTreeNode make_dyn_node(std::uintptr_t offset, std::uint32_t parent_index)
{
    return ItemTreeNode { ItemTreeNode::DynamicTree_Body { ItemTreeNode::Tag::DynamicTree, offset,
                                                           parent_index } };
}

inline ItemRef get_item_ref(ComponentRef component,
                            const cbindgen_private::Slice<ItemTreeNode> item_tree,
                            const private_api::ItemArray item_array, int index)
{
    const auto item_array_index = item_tree.ptr[index].item.item_array_index;
    const auto item = item_array[item_array_index];
    return ItemRef { item.vtable, reinterpret_cast<char *>(component.instance) + item.offset };
}

inline void dealloc(const ComponentVTable *, uint8_t *ptr, vtable::Layout layout)
{
#ifdef __cpp_sized_deallocation
    ::operator delete(reinterpret_cast<void *>(ptr), layout.size,
                      static_cast<std::align_val_t>(layout.align));
#elif !defined(__APPLE__) || MAC_OS_X_VERSION_MIN_REQUIRED >= MAC_OS_X_VERSION_10_14
    ::operator delete(reinterpret_cast<void *>(ptr), static_cast<std::align_val_t>(layout.align));
#else
    ::operator delete(reinterpret_cast<void *>(ptr));
#endif
}

template<typename T>
inline vtable::Layout drop_in_place(ComponentRef component)
{
    reinterpret_cast<T *>(component.instance)->~T();
    return vtable::Layout { sizeof(T), alignof(T) };
}

#if !defined(DOXYGEN)
#    if defined(_WIN32) || defined(_WIN64)
// On Windows cross-dll data relocations are not supported:
//     https://docs.microsoft.com/en-us/cpp/c-language/rules-and-limitations-for-dllimport-dllexport?view=msvc-160
// so we have a relocation to a function that returns the address we seek. That
// relocation will be resolved to the locally linked stub library, the implementation of
// which will be patched.
#        define SLINT_GET_ITEM_VTABLE(VTableName) slint::private_api::slint_get_##VTableName()
#    else
#        define SLINT_GET_ITEM_VTABLE(VTableName) (&slint::private_api::VTableName)
#    endif
#endif // !defined(DOXYGEN)

template<typename T>
struct ReturnWrapper
{
    ReturnWrapper(T val) : value(std::move(val)) { }
    T value;
};
template<>
struct ReturnWrapper<void>
{
};
} // namespace private_api

template<typename T>
class ComponentWeakHandle;

/// The component handle is like a shared pointer to a component in the generated code.
/// In order to get a component, use `T::create()` where T is the name of the component
/// in the .slint file. This give you a `ComponentHandle<T>`
template<typename T>
class ComponentHandle
{
    vtable::VRc<private_api::ComponentVTable, T> inner;
    friend class ComponentWeakHandle<T>;

public:
    /// internal constructor
    ComponentHandle(const vtable::VRc<private_api::ComponentVTable, T> &inner) : inner(inner) { }

    /// Arrow operator that implements pointer semantics.
    const T *operator->() const
    {
        private_api::assert_main_thread();
        return inner.operator->();
    }
    /// Dereference operator that implements pointer semantics.
    const T &operator*() const
    {
        private_api::assert_main_thread();
        return inner.operator*();
    }
    /// Arrow operator that implements pointer semantics.
    T *operator->()
    {
        private_api::assert_main_thread();
        return inner.operator->();
    }
    /// Dereference operator that implements pointer semantics.
    T &operator*()
    {
        private_api::assert_main_thread();
        return inner.operator*();
    }

    /// internal function that returns the internal handle
    vtable::VRc<private_api::ComponentVTable> into_dyn() const { return inner.into_dyn(); }
};

/// A weak reference to the component. Can be constructed from a `ComponentHandle<T>`
template<typename T>
class ComponentWeakHandle
{
    vtable::VWeak<private_api::ComponentVTable, T> inner;

public:
    /// Constructs a null ComponentWeakHandle. lock() will always return empty.
    ComponentWeakHandle() = default;
    /// Copy-constructs a new ComponentWeakHandle from \a other.
    ComponentWeakHandle(const ComponentHandle<T> &other) : inner(other.inner) { }
    /// Returns a new strong ComponentHandle<T> if the component the weak handle points to is
    /// still referenced by any other ComponentHandle<T>. An empty std::optional is returned
    /// otherwise.
    std::optional<ComponentHandle<T>> lock() const
    {
        private_api::assert_main_thread();
        if (auto l = inner.lock()) {
            return { ComponentHandle(*l) };
        } else {
            return {};
        }
    }
};

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

    /// \private
    private_api::WindowAdapterRc &window_handle() { return inner; }
    /// \private
    const private_api::WindowAdapterRc &window_handle() const { return inner; }

private:
    private_api::WindowAdapterRc inner;
};

/// A Timer that can call a callback at repeated interval
///
/// Use the static single_shot function to make a single shot timer
struct Timer
{
    /// Construct a null timer. Use the start() method to activate the timer with a mode, interval
    /// and callback.
    Timer() : id(-1) { }
    /// Construct a timer which will repeat the callback every `interval` milliseconds until
    /// the destructor of the timer is called.
    ///
    /// This is a convenience function and equivalent to calling
    /// `start(slint::TimerMode::Repeated, interval, callback);` on a default constructed Timer.
    template<std::invocable F>
    Timer(std::chrono::milliseconds interval, F callback)
        : id(cbindgen_private::slint_timer_start(
                -1, TimerMode::Repeated, interval.count(),
                [](void *data) { (*reinterpret_cast<F *>(data))(); }, new F(std::move(callback)),
                [](void *data) { delete reinterpret_cast<F *>(data); }))
    {
    }
    Timer(const Timer &) = delete;
    Timer &operator=(const Timer &) = delete;
    ~Timer() { cbindgen_private::slint_timer_destroy(id); }

    /// Starts the timer with the given \a mode and \a interval, in order for the \a callback to
    /// called when the timer fires. If the timer has been started previously and not fired yet,
    /// then it will be restarted.
    template<std::invocable F>
    void start(TimerMode mode, std::chrono::milliseconds interval, F callback)
    {
        id = cbindgen_private::slint_timer_start(
                id, mode, interval.count(), [](void *data) { (*reinterpret_cast<F *>(data))(); },
                new F(std::move(callback)), [](void *data) { delete reinterpret_cast<F *>(data); });
    }
    /// Stops the previously started timer. Does nothing if the timer has never been started. A
    /// stopped timer cannot be restarted with restart(). Use start() instead.
    void stop() { cbindgen_private::slint_timer_stop(id); }
    /// Restarts the timer. If the timer was previously started by calling [`Self::start()`]
    /// with a duration and callback, then the time when the callback will be next invoked
    /// is re-calculated to be in the specified duration relative to when this function is called.
    ///
    /// Does nothing if the timer was never started.
    void restart() { cbindgen_private::slint_timer_restart(id); }
    /// Returns true if the timer is running; false otherwise.
    bool running() const { return cbindgen_private::slint_timer_running(id); }

    /// Call the callback after the given duration.
    template<std::invocable F>
    static void single_shot(std::chrono::milliseconds duration, F callback)
    {
        cbindgen_private::slint_timer_singleshot(
                duration.count(), [](void *data) { (*reinterpret_cast<F *>(data))(); },
                new F(std::move(callback)), [](void *data) { delete reinterpret_cast<F *>(data); });
    }

private:
    int64_t id;
};

namespace cbindgen_private {
inline LayoutInfo LayoutInfo::merge(const LayoutInfo &other) const
{
    // Note: This "logic" is duplicated from LayoutInfo::merge in layout.rs.
    return LayoutInfo { std::min(max, other.max),
                        std::min(max_percent, other.max_percent),
                        std::max(min, other.min),
                        std::max(min_percent, other.min_percent),
                        std::max(preferred, other.preferred),
                        std::min(stretch, other.stretch) };
}
}

namespace private_api {

inline SharedVector<float> solve_box_layout(const cbindgen_private::BoxLayoutData &data,
                                            cbindgen_private::Slice<int> repeater_indexes)
{
    SharedVector<float> result;
    cbindgen_private::Slice<uint32_t> ri { reinterpret_cast<uint32_t *>(repeater_indexes.ptr),
                                           repeater_indexes.len };
    cbindgen_private::slint_solve_box_layout(&data, ri, &result);
    return result;
}

inline SharedVector<float> solve_grid_layout(const cbindgen_private::GridLayoutData &data)
{
    SharedVector<float> result;
    cbindgen_private::slint_solve_grid_layout(&data, &result);
    return result;
}

inline cbindgen_private::LayoutInfo
grid_layout_info(cbindgen_private::Slice<cbindgen_private::GridLayoutCellData> cells, float spacing,
                 const cbindgen_private::Padding &padding)
{
    return cbindgen_private::slint_grid_layout_info(cells, spacing, &padding);
}

inline cbindgen_private::LayoutInfo
box_layout_info(cbindgen_private::Slice<cbindgen_private::BoxLayoutCellData> cells, float spacing,
                const cbindgen_private::Padding &padding,
                cbindgen_private::LayoutAlignment alignment)
{
    return cbindgen_private::slint_box_layout_info(cells, spacing, &padding, alignment);
}

inline cbindgen_private::LayoutInfo
box_layout_info_ortho(cbindgen_private::Slice<cbindgen_private::BoxLayoutCellData> cells,
                      const cbindgen_private::Padding &padding)
{
    return cbindgen_private::slint_box_layout_info_ortho(cells, &padding);
}

/// Access the layout cache of an item within a repeater
inline float layout_cache_access(const SharedVector<float> &cache, int offset, int repeater_index)
{
    size_t idx = size_t(cache[offset]) + repeater_index * 2;
    return idx < cache.size() ? cache[idx] : 0;
}

// models
struct ModelChangeListener
{
    virtual ~ModelChangeListener() = default;
    virtual void row_added(size_t index, size_t count) = 0;
    virtual void row_removed(size_t index, size_t count) = 0;
    virtual void row_changed(size_t index) = 0;
    virtual void reset() = 0;
};
using ModelPeer = std::weak_ptr<ModelChangeListener>;

template<typename M>
auto access_array_index(const M &model, size_t index)
{
    if (const auto v = model->row_data_tracked(index)) {
        return *v;
    } else {
        return decltype(*v) {};
    }
}

} // namespace private_api

/// \rst
/// A Model is providing Data for
/// `for - in<../../slint/src/reference/repetitions.html>`_ repetitions or
/// `ListView<../../slint/src/builtins/widgets.html#listview>`_ elements of the :code:`.slint`
/// language \endrst
template<typename ModelData>
class Model
{
public:
    virtual ~Model() = default;
    Model() = default;
    Model(const Model &) = delete;
    Model &operator=(const Model &) = delete;

    /// The amount of row in the model
    virtual size_t row_count() const = 0;
    /// Returns the data for a particular row. This function should be called with `row <
    /// row_count()`.
    virtual std::optional<ModelData> row_data(size_t i) const = 0;
    /// Sets the data for a particular row.
    ///
    /// This function should only be called with `row < row_count()`.
    ///
    /// If the model cannot support data changes, then it is ok to do nothing.
    /// The default implementation will print a warning to stderr.
    ///
    /// If the model can update the data, it should also call `row_changed`
    virtual void set_row_data(size_t, const ModelData &)
    {
        std::cerr << "Model::set_row_data was called on a read-only model" << std::endl;
    };

    /// \private
    /// Internal function called by the view to register itself
    void attach_peer(private_api::ModelPeer p) { peers.push_back(std::move(p)); }

    /// \private
    /// Internal function called from within bindings to register with the currently
    /// evaluating dependency and get notified when this model's row count changes.
    void track_row_count_changes() const { model_row_count_dirty_property.get(); }

    /// \private
    /// Internal function called from within bindings to register with the currently
    /// evaluating dependency and get notified when this model's row data changes.
    void track_row_data_changes(size_t row) const
    {
        auto it = std::lower_bound(tracked_rows.begin(), tracked_rows.end(), row);
        if (it == tracked_rows.end() || row < *it) {
            tracked_rows.insert(it, row);
        }
        model_row_data_dirty_property.get();
    }

    /// \private
    /// Convenience function that calls `track_row_data_changes` before returning `row_data`
    std::optional<ModelData> row_data_tracked(size_t row) const
    {
        track_row_data_changes(row);
        return row_data(row);
    }

protected:
    /// Notify the views that a specific row was changed
    void row_changed(size_t row)
    {
        if (std::binary_search(tracked_rows.begin(), tracked_rows.end(), row)) {
            model_row_data_dirty_property.mark_dirty();
        }
        for_each_peers([=](auto peer) { peer->row_changed(row); });
    }
    /// Notify the views that rows were added
    void row_added(size_t index, size_t count)
    {
        model_row_count_dirty_property.mark_dirty();
        tracked_rows.clear();
        model_row_data_dirty_property.mark_dirty();
        for_each_peers([=](auto peer) { peer->row_added(index, count); });
    }
    /// Notify the views that rows were removed
    void row_removed(size_t index, size_t count)
    {
        model_row_count_dirty_property.mark_dirty();
        tracked_rows.clear();
        model_row_data_dirty_property.mark_dirty();
        for_each_peers([=](auto peer) { peer->row_removed(index, count); });
    }

    /// Notify the views that the model has been changed and that everything needs to be reloaded
    void reset()
    {
        model_row_count_dirty_property.mark_dirty();
        tracked_rows.clear();
        model_row_data_dirty_property.mark_dirty();
        for_each_peers([=](auto peer) { peer->reset(); });
    }

private:
    template<typename F>
    void for_each_peers(const F &f)
    {
        private_api::assert_main_thread();
        peers.erase(std::remove_if(peers.begin(), peers.end(),
                                   [&](const auto &p) {
                                       if (auto pp = p.lock()) {
                                           f(pp);
                                           return false;
                                       }
                                       return true;
                                   }),
                    peers.end());
    }
    std::vector<private_api::ModelPeer> peers;
    private_api::Property<bool> model_row_count_dirty_property;
    private_api::Property<bool> model_row_data_dirty_property;
    mutable std::vector<size_t> tracked_rows;
};

namespace private_api {
/// A Model backed by a std::array of constant size
/// \private
template<int Count, typename ModelData>
class ArrayModel : public Model<ModelData>
{
    std::array<ModelData, Count> data;

public:
    /// Constructs a new ArrayModel by forwarding \a to the std::array constructor.
    template<typename... A>
    ArrayModel(A &&...a) : data { std::forward<A>(a)... }
    {
    }
    size_t row_count() const override { return Count; }
    std::optional<ModelData> row_data(size_t i) const override
    {
        if (i >= row_count())
            return {};
        return data[i];
    }
    void set_row_data(size_t i, const ModelData &value) override
    {
        if (i < row_count()) {
            data[i] = value;
            this->row_changed(i);
        }
    }
};

/// Model to be used when we just want to repeat without data.
struct UIntModel : Model<int>
{
    /// Constructs a new IntModel with \a d rows.
    UIntModel(uint32_t d) : data(d) { }
    /// \private
    uint32_t data;
    /// \copydoc Model::row_count
    size_t row_count() const override { return data; }
    std::optional<int> row_data(size_t value) const override
    {
        if (value >= row_count())
            return {};
        return static_cast<int>(value);
    }
};
} // namespace private_api

/// A Model backed by a SharedVector
template<typename ModelData>
class VectorModel : public Model<ModelData>
{
    std::vector<ModelData> data;

public:
    /// Constructs a new empty VectorModel.
    VectorModel() = default;
    /// Constructs a new VectorModel from \a array.
    VectorModel(std::vector<ModelData> array) : data(std::move(array)) { }
    size_t row_count() const override { return data.size(); }
    std::optional<ModelData> row_data(size_t i) const override
    {
        if (i >= row_count())
            return {};
        return std::optional<ModelData> { data[i] };
    }
    void set_row_data(size_t i, const ModelData &value) override
    {
        if (i < row_count()) {
            data[i] = value;
            this->row_changed(i);
        }
    }

    /// Append a new row with the given value
    void push_back(const ModelData &value)
    {
        data.push_back(value);
        this->row_added(data.size() - 1, 1);
    }

    /// Remove the row at the given index from the model
    void erase(size_t index)
    {
        data.erase(data.begin() + index);
        this->row_removed(index, 1);
    }

    /// Inserts the given value as a new row at the specified index
    void insert(size_t index, const ModelData &value)
    {
        data.insert(data.begin() + index, value);
        this->row_added(index, 1);
    }
};

template<typename ModelData>
class FilterModel;

namespace private_api {
template<typename ModelData>
struct FilterModelInner : private_api::ModelChangeListener
{
    FilterModelInner(std::shared_ptr<slint::Model<ModelData>> source_model,
                     std::function<bool(const ModelData &)> filter_fn,
                     slint::FilterModel<ModelData> &target_model)
        : source_model(source_model), filter_fn(filter_fn), target_model(target_model)
    {
        update_mapping();
    }

    void row_added(size_t index, size_t count) override
    {
        if (count == 0) {
            return;
        }

        std::vector<size_t> added_accepted_rows;
        for (auto i = index; i < index + count; ++i) {
            if (auto data = source_model->row_data(i)) {
                if (filter_fn(*data)) {
                    added_accepted_rows.push_back(i);
                }
            }
        }

        if (added_accepted_rows.empty()) {
            return;
        }

        auto insertion_point = std::lower_bound(accepted_rows.begin(), accepted_rows.end(), index);

        insertion_point = accepted_rows.insert(insertion_point, added_accepted_rows.begin(),
                                               added_accepted_rows.end());

        for (auto it = insertion_point + added_accepted_rows.size(); it != accepted_rows.end();
             ++it)
            (*it) += count;

        target_model.row_added(insertion_point - accepted_rows.begin(), added_accepted_rows.size());
    }
    void row_changed(size_t index) override
    {
        auto existing_row = std::lower_bound(accepted_rows.begin(), accepted_rows.end(), index);
        auto existing_row_index = std::distance(accepted_rows.begin(), existing_row);
        bool is_contained = existing_row != accepted_rows.end() && *existing_row == index;
        auto accepted_updated_row = filter_fn(*source_model->row_data(index));

        if (is_contained && accepted_updated_row) {
            target_model.row_changed(existing_row_index);
        } else if (!is_contained && accepted_updated_row) {
            accepted_rows.insert(existing_row, index);
            target_model.row_added(existing_row_index, 1);
        } else if (is_contained && !accepted_updated_row) {
            accepted_rows.erase(existing_row);
            target_model.row_removed(existing_row_index, 1);
        }
    }
    void row_removed(size_t index, size_t count) override
    {
        auto mapped_row_start = std::lower_bound(accepted_rows.begin(), accepted_rows.end(), index);
        auto mapped_row_end =
                std::lower_bound(accepted_rows.begin(), accepted_rows.end(), index + count);

        auto mapped_removed_len = std::distance(mapped_row_start, mapped_row_end);

        auto mapped_removed_index =
                (mapped_row_start != accepted_rows.end() && *mapped_row_start == index)
                ? std::optional<int>(mapped_row_start - accepted_rows.begin())
                : std::nullopt;

        auto it = accepted_rows.erase(mapped_row_start, mapped_row_end);
        for (; it != accepted_rows.end(); ++it) {
            *it -= count;
        }

        if (mapped_removed_index) {
            target_model.row_removed(*mapped_removed_index, mapped_removed_len);
        }
    }
    void reset() override
    {
        update_mapping();
        target_model.reset();
    }

    void update_mapping()
    {
        accepted_rows.clear();
        for (size_t i = 0, count = source_model->row_count(); i < count; ++i) {
            if (auto data = source_model->row_data(i)) {
                if (filter_fn(*data)) {
                    accepted_rows.push_back(i);
                }
            }
        }
    }

    std::shared_ptr<slint::Model<ModelData>> source_model;
    std::function<bool(const ModelData &)> filter_fn;
    std::vector<size_t> accepted_rows;
    slint::FilterModel<ModelData> &target_model;
};
}

/// The FilterModel acts as an adapter model for a given source model by applying a filter
/// function. The filter function is called for each row on the source model and if the
/// filter accepts the row (i.e. returns true), the row is also visible in the FilterModel.
template<typename ModelData>
class FilterModel : public Model<ModelData>
{
    friend struct private_api::FilterModelInner<ModelData>;

public:
    /// Constructs a new FilterModel that provides a limited view on the \a source_model by applying
    /// \a filter_fn on each row. If the provided function returns true, the row is exposed by the
    /// FilterModel.
    FilterModel(std::shared_ptr<Model<ModelData>> source_model,
                std::function<bool(const ModelData &)> filter_fn)
        : inner(std::make_shared<private_api::FilterModelInner<ModelData>>(
                std::move(source_model), std::move(filter_fn), *this))
    {
        inner->source_model->attach_peer(inner);
    }

    size_t row_count() const override { return inner->accepted_rows.size(); }

    std::optional<ModelData> row_data(size_t i) const override
    {
        if (i >= inner->accepted_rows.size())
            return {};
        return inner->source_model->row_data(inner->accepted_rows[i]);
    }

    void set_row_data(size_t i, const ModelData &value) override
    {
        inner->source_model->set_row_data(inner->accepted_rows[i], value);
    }

    /// Re-applies the model's filter function on each row of the source model. Use this if state
    /// external to the filter function has changed.
    void reset() { inner->reset(); }

    /// Given the \a filtered_row index, this function returns the corresponding row index in the
    /// source model.
    int unfiltered_row(int filtered_row) const { return inner->accepted_rows[filtered_row]; }

    /// Returns the source model of this filter model.
    std::shared_ptr<Model<ModelData>> source_model() const { return inner->source_model; }

private:
    std::shared_ptr<private_api::FilterModelInner<ModelData>> inner;
};

template<typename SourceModelData, typename MappedModelData>
class MapModel;

namespace private_api {
template<typename SourceModelData, typename MappedModelData>
struct MapModelInner : private_api::ModelChangeListener
{
    MapModelInner(slint::MapModel<SourceModelData, MappedModelData> &target_model)
        : target_model(target_model)
    {
    }

    void row_added(size_t index, size_t count) override { target_model.row_added(index, count); }
    void row_changed(size_t index) override { target_model.row_changed(index); }
    void row_removed(size_t index, size_t count) override
    {
        target_model.row_removed(index, count);
    }
    void reset() override { target_model.reset(); }

    slint::MapModel<SourceModelData, MappedModelData> &target_model;
};
}

/// The MapModel acts as an adapter model for a given source model by applying a mapping
/// function. The mapping function is called for each row on the source model and allows
/// transforming the values on the fly. The MapModel has two template parameters: The
/// SourceModelData specifies the data type of the underlying source model, and the
/// MappedModelData the data type of this MapModel. This permits not only changing the
/// values of the underlying source model, but also changing the data type itself. For
/// example a MapModel can be used to adapt a model that provides numbers to be a model
/// that exposes all numbers converted to strings, by calling `std::to_string` on each
/// value given in the mapping lambda expression.
template<typename SourceModelData, typename MappedModelData = SourceModelData>
class MapModel : public Model<MappedModelData>
{
    friend struct private_api::MapModelInner<SourceModelData, MappedModelData>;

public:
    /// Constructs a new MapModel that provides an altered view on the \a source_model by applying
    /// \a map_fn on the data in each row.
    MapModel(std::shared_ptr<Model<SourceModelData>> source_model,
             std::function<MappedModelData(const SourceModelData &)> map_fn)
        : inner(std::make_shared<private_api::MapModelInner<SourceModelData, MappedModelData>>(
                *this)),
          model(source_model),
          map_fn(map_fn)
    {
        model->attach_peer(inner);
    }

    size_t row_count() const override { return model->row_count(); }

    std::optional<MappedModelData> row_data(size_t i) const override
    {
        if (auto source_data = model->row_data(i))
            return map_fn(*source_data);
        else
            return {};
    }

    /// Returns the source model of this filter model.
    std::shared_ptr<Model<SourceModelData>> source_model() const { return model; }

private:
    std::shared_ptr<private_api::MapModelInner<SourceModelData, MappedModelData>> inner;
    std::shared_ptr<slint::Model<SourceModelData>> model;
    std::function<MappedModelData(const SourceModelData &)> map_fn;
};

template<typename ModelData>
class SortModel;

namespace private_api {
template<typename ModelData>
struct SortModelInner : private_api::ModelChangeListener
{
    SortModelInner(std::shared_ptr<slint::Model<ModelData>> source_model,
                   std::function<bool(const ModelData &, const ModelData &)> comp,
                   slint::SortModel<ModelData> &target_model)
        : source_model(source_model), comp(comp), target_model(target_model)
    {
    }

    void row_added(size_t first_inserted_row, size_t count) override
    {
        if (sorted_rows_dirty) {
            reset();
            return;
        }

        // Adjust the existing sorted row indices to match the updated source model
        for (auto &row : sorted_rows) {
            if (row >= first_inserted_row)
                row += count;
        }

        for (size_t row = first_inserted_row; row < first_inserted_row + count; ++row) {

            ModelData inserted_value = *source_model->row_data(row);
            auto insertion_point =
                    std::lower_bound(sorted_rows.begin(), sorted_rows.end(), inserted_value,
                                     [this](size_t sorted_row, const ModelData &inserted_value) {
                                         auto sorted_elem = source_model->row_data(sorted_row);
                                         return comp(*sorted_elem, inserted_value);
                                     });

            insertion_point = sorted_rows.insert(insertion_point, row);
            target_model.row_added(std::distance(sorted_rows.begin(), insertion_point), 1);
        }
    }
    void row_changed(size_t changed_row) override
    {
        if (sorted_rows_dirty) {
            reset();
            return;
        }

        auto removed_row_it =
                sorted_rows.erase(std::find(sorted_rows.begin(), sorted_rows.end(), changed_row));
        auto removed_row = std::distance(sorted_rows.begin(), removed_row_it);

        ModelData changed_value = *source_model->row_data(changed_row);
        auto insertion_point =
                std::lower_bound(sorted_rows.begin(), sorted_rows.end(), changed_value,
                                 [this](size_t sorted_row, const ModelData &changed_value) {
                                     auto sorted_elem = source_model->row_data(sorted_row);
                                     return comp(*sorted_elem, changed_value);
                                 });

        insertion_point = sorted_rows.insert(insertion_point, changed_row);
        auto inserted_row = std::distance(sorted_rows.begin(), insertion_point);

        if (inserted_row == removed_row) {
            target_model.row_changed(removed_row);
        } else {
            target_model.row_removed(removed_row, 1);
            target_model.row_added(inserted_row, 1);
        }
    }
    void row_removed(size_t first_removed_row, size_t count) override
    {
        if (sorted_rows_dirty) {
            reset();
            return;
        }

        std::vector<size_t> removed_rows;
        removed_rows.reserve(count);

        for (auto it = sorted_rows.begin(); it != sorted_rows.end();) {
            if (*it >= first_removed_row) {
                if (*it < first_removed_row + count) {
                    removed_rows.push_back(std::distance(sorted_rows.begin(), it));
                    it = sorted_rows.erase(it);
                    continue;
                } else {
                    *it -= count;
                }
            }
            ++it;
        }

        for (auto removed_row : removed_rows) {
            target_model.row_removed(removed_row, 1);
        }
    }
    void reset() override
    {
        sorted_rows_dirty = true;
        target_model.reset();
    }

    void ensure_sorted()
    {
        if (!sorted_rows_dirty) {
            return;
        }

        sorted_rows.resize(source_model->row_count());
        for (size_t i = 0; i < sorted_rows.size(); ++i)
            sorted_rows[i] = i;

        std::sort(sorted_rows.begin(), sorted_rows.end(), [this](auto lhs_index, auto rhs_index) {
            auto lhs_elem = source_model->row_data(lhs_index);
            auto rhs_elem = source_model->row_data(rhs_index);
            return comp(*lhs_elem, *rhs_elem);
        });

        sorted_rows_dirty = false;
    }

    std::shared_ptr<slint::Model<ModelData>> source_model;
    std::function<bool(const ModelData &, const ModelData &)> comp;
    slint::SortModel<ModelData> &target_model;
    std::vector<size_t> sorted_rows;
    bool sorted_rows_dirty = true;
};
}

/// The SortModel acts as an adapter model for a given source model by sorting all rows
/// with by order provided by the given sorting function. The sorting function is called for
/// pairs of elements of the source model.
template<typename ModelData>
class SortModel : public Model<ModelData>
{
    friend struct private_api::SortModelInner<ModelData>;

public:
    /// Constructs a new SortModel that provides a sorted view on the \a source_model by applying
    /// the order given by the specified \a comp.
    SortModel(std::shared_ptr<Model<ModelData>> source_model,
              std::function<bool(const ModelData &, const ModelData &)> comp)
        : inner(std::make_shared<private_api::SortModelInner<ModelData>>(std::move(source_model),
                                                                         std::move(comp), *this))
    {
        inner->source_model->attach_peer(inner);
    }

    size_t row_count() const override { return inner->source_model->row_count(); }

    std::optional<ModelData> row_data(size_t i) const override
    {
        inner->ensure_sorted();
        return inner->source_model->row_data(inner->sorted_rows[i]);
    }

    void set_row_data(size_t i, const ModelData &value) override
    {
        inner->source_model->set_row_data(inner->sorted_rows[i], value);
    }
    /// Re-applies the model's sort function on each row of the source model. Use this if state
    /// external to the sort function has changed.
    void reset() { inner->reset(); }

    /// Given the \a sorted_row_index, this function returns the corresponding row index in the
    /// source model.
    int unsorted_row(int sorted_row_index) const
    {
        inner->ensure_sorted();
        return inner->sorted_rows[sorted_row_index];
    }

    /// Returns the source model of this filter model.
    std::shared_ptr<Model<ModelData>> source_model() const { return inner->source_model; }

private:
    std::shared_ptr<private_api::SortModelInner<ModelData>> inner;
};

namespace private_api {

template<typename C, typename ModelData>
class Repeater
{
    private_api::Property<std::shared_ptr<Model<ModelData>>> model;

    struct RepeaterInner : ModelChangeListener
    {
        enum class State { Clean, Dirty };
        struct ComponentWithState
        {
            State state = State::Dirty;
            std::optional<ComponentHandle<C>> ptr;
        };
        std::vector<ComponentWithState> data;
        private_api::Property<bool> is_dirty { true };

        void row_added(size_t index, size_t count) override
        {
            is_dirty.set(true);
            data.resize(data.size() + count);
            std::rotate(data.begin() + index, data.end() - count, data.end());
        }
        void row_changed(size_t index) override
        {
            is_dirty.set(true);
            data[index].state = State::Dirty;
        }
        void row_removed(size_t index, size_t count) override
        {
            is_dirty.set(true);
            data.erase(data.begin() + index, data.begin() + index + count);
            for (std::size_t i = index; i < data.size(); ++i) {
                // all the indexes are dirty
                data[i].state = State::Dirty;
            }
        }
        void reset() override
        {
            is_dirty.set(true);
            data.clear();
        }
    };

public:
    // FIXME: should be private, but layouting code uses it.
    mutable std::shared_ptr<RepeaterInner> inner;

    template<typename F>
    void set_model_binding(F &&binding) const
    {
        model.set_binding(std::forward<F>(binding));
    }

    template<typename Parent>
    void ensure_updated(const Parent *parent) const
    {
        if (model.is_dirty()) {
            auto preserved_data = inner ? std::make_optional(std::move(inner->data)) : std::nullopt;
            inner = std::make_shared<RepeaterInner>();
            if (auto data = preserved_data) {
                inner->data = std::move(*data);
                for (auto &&compo_with_state : inner->data) {
                    compo_with_state.state = RepeaterInner::State::Dirty;
                }
            }
            if (auto m = model.get()) {
                m->attach_peer(inner);
            }
        }

        if (inner && inner->is_dirty.get()) {
            inner->is_dirty.set(false);
            if (auto m = model.get()) {
                auto count = m->row_count();
                inner->data.resize(count);
                for (size_t i = 0; i < count; ++i) {
                    auto &c = inner->data[i];
                    bool created = false;
                    if (!c.ptr) {
                        c.ptr = C::create(parent);
                        created = true;
                    }
                    if (c.state == RepeaterInner::State::Dirty) {
                        (*c.ptr)->update_data(i, *m->row_data(i));
                    }
                    if (created) {
                        (*c.ptr)->init();
                    }
                }
            } else {
                inner->data.clear();
            }
        } else {
            // just do a get() on the model to register dependencies so that, for example, the
            // layout property tracker becomes dirty.
            model.get();
        }
    }

    template<typename Parent>
    void ensure_updated_listview(const Parent *parent,
                                 const private_api::Property<float> *viewport_width,
                                 const private_api::Property<float> *viewport_height,
                                 [[maybe_unused]] const private_api::Property<float> *viewport_y,
                                 float listview_width, [[maybe_unused]] float listview_height) const
    {
        // TODO: the rust code in model.rs try to only allocate as many items as visible items
        ensure_updated(parent);

        float h = compute_layout_listview(viewport_width, listview_width);
        viewport_height->set(h);
    }

    uint64_t visit(TraversalOrder order, private_api::ItemVisitorRefMut visitor) const
    {
        for (std::size_t i = 0; i < inner->data.size(); ++i) {
            auto index = order == TraversalOrder::BackToFront ? i : inner->data.size() - 1 - i;
            auto ref = item_at(index);
            if (ref.vtable->visit_children_item(ref, -1, order, visitor)
                != std::numeric_limits<uint64_t>::max()) {
                return index;
            }
        }
        return std::numeric_limits<uint64_t>::max();
    }

    vtable::VRef<private_api::ComponentVTable> item_at(int i) const
    {
        const auto &x = inner->data.at(i);
        return { &C::static_vtable, const_cast<C *>(&(**x.ptr)) };
    }

    vtable::VWeak<private_api::ComponentVTable> component_at(int i) const
    {
        const auto &x = inner->data.at(i);
        return vtable::VWeak<private_api::ComponentVTable> { x.ptr->into_dyn() };
    }

    private_api::IndexRange index_range() const
    {
        return private_api::IndexRange { 0, inner->data.size() };
    }

    float compute_layout_listview(const private_api::Property<float> *viewport_width,
                                  float listview_width) const
    {
        float offset = 0;
        viewport_width->set(listview_width);
        if (!inner)
            return offset;
        for (auto &x : inner->data) {
            (*x.ptr)->listview_layout(&offset, viewport_width);
        }
        return offset;
    }

    void model_set_row_data(size_t row, const ModelData &data) const
    {
        if (model.is_dirty()) {
            std::abort();
        }
        if (auto m = model.get()) {
            if (row < m->row_count()) {
                m->set_row_data(row, data);
                if (inner && inner->is_dirty.get()) {
                    auto &c = inner->data[row];
                    if (c.state == RepeaterInner::State::Dirty && c.ptr) {
                        (*c.ptr)->update_data(row, *m->row_data(row));
                    }
                }
            }
        }
    }
};

inline SharedString translate(const SharedString &original, const SharedString &context,
                              const SharedString &domain,
                              cbindgen_private::Slice<SharedString> arguments, int n,
                              const SharedString &plural)
{
    SharedString result = original;
    cbindgen_private::slint_translate(&result, &context, &domain, arguments, n, &plural);
    return result;
}

} // namespace private_api

#if !defined(DOXYGEN)
cbindgen_private::Flickable::Flickable()
{
    slint_flickable_data_init(&data);
}
cbindgen_private::Flickable::~Flickable()
{
    slint_flickable_data_free(&data);
}

cbindgen_private::NativeStyleMetrics::NativeStyleMetrics(void *)
{
    slint_native_style_metrics_init(this);
}

cbindgen_private::NativeStyleMetrics::~NativeStyleMetrics()
{
    slint_native_style_metrics_deinit(this);
}
#endif // !defined(DOXYGEN)

namespace private_api {
template<int Major, int Minor, int Patch>
struct VersionCheckHelper
{
};
}

/// Enters the main event loop. This is necessary in order to receive
/// events from the windowing system in order to render to the screen
/// and react to user input.
inline void run_event_loop()
{
    private_api::assert_main_thread();
    cbindgen_private::slint_run_event_loop();
}

/// Schedules the main event loop for termination. This function is meant
/// to be called from callbacks triggered by the UI. After calling the function,
/// it will return immediately and once control is passed back to the event loop,
/// the initial call to slint::run_event_loop() will return.
inline void quit_event_loop()
{
    cbindgen_private::slint_quit_event_loop();
}

/// Adds the specified functor to an internal queue, notifies the event loop to wake up.
/// Once woken up, any queued up functors will be invoked.
/// This function is thread-safe and can be called from any thread, including the one
/// running the event loop. The provided functors will only be invoked from the thread
/// that started the event loop.
///
/// You can use this to set properties or use any other Slint APIs from other threads,
/// by collecting the code in a functor and queuing it up for invocation within the event loop.
///
/// The following example assumes that a status message received from a network thread is
/// shown in the UI:
///
/// ```
/// #include "my_application_ui.h"
/// #include <thread>
///
/// int main(int argc, char **argv)
/// {
///     auto ui = NetworkStatusUI::create();
///     ui->set_status_label("Pending");
///
///     slint::ComponentWeakHandle<NetworkStatusUI> weak_ui_handle(ui);
///     std::thread network_thread([=]{
///         std::string message = read_message_blocking_from_network();
///         slint::invoke_from_event_loop([&]() {
///             if (auto ui = weak_ui_handle.lock()) {
///                 ui->set_status_label(message);
///             }
///         });
///     });
///     ...
///     ui->run();
///     ...
/// }
/// ```
///
/// See also blocking_invoke_from_event_loop() for a blocking version of this function
template<std::invocable Functor>
void invoke_from_event_loop(Functor f)
{
    cbindgen_private::slint_post_event(
            [](void *data) { (*reinterpret_cast<Functor *>(data))(); }, new Functor(std::move(f)),
            [](void *data) { delete reinterpret_cast<Functor *>(data); });
}

/// Blocking version of invoke_from_event_loop()
///
/// Just like invoke_from_event_loop(), this will run the specified functor from the thread running
/// the slint event loop. But it will block until the execution of the functor is finished,
/// and return that value.
///
/// This function must be called from a different thread than the thread that runs the event loop
/// otherwise it will result in a deadlock. Calling this function if the event loop is not running
/// will also block forever or until the event loop is started in another thread.
///
/// The following example is reading the message property from a thread
///
/// ```
/// #include "my_application_ui.h"
/// #include <thread>
///
/// int main(int argc, char **argv)
/// {
///     auto ui = MyApplicationUI::create();
///     ui->set_status_label("Pending");
///
///     std::thread worker_thread([ui]{
///         while (...) {
///             auto message = slint::blocking_invoke_from_event_loop([ui]() {
///                return ui->get_message();
///             }
///             do_something(message);
///             ...
///         });
///     });
///     ...
///     ui->run();
///     ...
/// }
/// ```
template<std::invocable Functor>
auto blocking_invoke_from_event_loop(Functor f) -> std::invoke_result_t<Functor>
{
    std::optional<std::invoke_result_t<Functor>> result;
    std::mutex mtx;
    std::condition_variable cv;
    invoke_from_event_loop([&] {
        auto r = f();
        std::unique_lock lock(mtx);
        result = std::move(r);
        cv.notify_one();
    });
    std::unique_lock lock(mtx);
    cv.wait(lock, [&] { return result.has_value(); });
    return std::move(*result);
}

#if !defined(DOXYGEN) // Doxygen doesn't see this as an overload of the previous one
// clang-format off
template<std::invocable Functor>
    requires(std::is_void_v<std::invoke_result_t<Functor>>)
void blocking_invoke_from_event_loop(Functor f)
// clang-format on
{
    std::mutex mtx;
    std::condition_variable cv;
    bool ok = false;
    invoke_from_event_loop([&] {
        f();
        std::unique_lock lock(mtx);
        ok = true;
        cv.notify_one();
    });
    std::unique_lock lock(mtx);
    cv.wait(lock, [&] { return ok; });
}
#endif

} // namespace slint
