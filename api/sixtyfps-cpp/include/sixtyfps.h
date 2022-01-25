// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

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

namespace sixtyfps::cbindgen_private {
// Workaround https://github.com/eqrion/cbindgen/issues/43
struct ComponentVTable;
struct ItemVTable;
}
#include "sixtyfps_internal.h"
#include "sixtyfps_backend_internal.h"
#include "sixtyfps_qt_internal.h"

/// \rst
/// The :code:`sixtyfps` namespace is the primary entry point into the SixtyFPS C++ API.
/// All available types are in this namespace.
///
/// See the :doc:`Overview <../overview>` documentation for the C++ integration how
/// to load :code:`.60` designs.
/// \endrst
namespace sixtyfps {

// Bring opaque structure in scope
namespace private_api {
using cbindgen_private::ComponentVTable;
using cbindgen_private::ItemVTable;
using ComponentRef = vtable::VRef<private_api::ComponentVTable>;
using ItemRef = vtable::VRef<private_api::ItemVTable>;
using ItemVisitorRefMut = vtable::VRefMut<cbindgen_private::ItemVisitorVTable>;
using cbindgen_private::ComponentRc;
using cbindgen_private::ItemWeak;
using cbindgen_private::TraversalOrder;
}

namespace private_api {
using ItemTreeNode = cbindgen_private::ItemTreeNode<uint8_t>;
using cbindgen_private::KeyboardModifiers;
using cbindgen_private::KeyEvent;
using cbindgen_private::PointerEvent;
using cbindgen_private::StandardListViewItem;

/// Internal function that checks that the API that must be called from the main
/// thread is indeed called from the main thread, or abort the program otherwise
///
/// Most API should be called from the main thread. When using thread one must
/// use sixtyfps::invoke_from_event_loop
inline void assert_main_thread()
{
#ifndef NDEBUG
    static auto main_thread_id = std::this_thread::get_id();
    if (main_thread_id != std::this_thread::get_id()) {
        std::cerr << "A function that should be only called from the main thread was called from a "
                     "thread."
                  << std::endl;
        std::cerr << "Most API should be called from the main thread. When using thread one must "
                     "use sixtyfps::invoke_from_event_loop."
                  << std::endl;
        std::abort();
    }
#endif
}

class WindowRc
{
public:
    explicit WindowRc(cbindgen_private::WindowRcOpaque adopted_inner) : inner(adopted_inner) { }
    WindowRc() { cbindgen_private::sixtyfps_windowrc_init(&inner); }
    ~WindowRc() { cbindgen_private::sixtyfps_windowrc_drop(&inner); }
    WindowRc(const WindowRc &other)
    {
        assert_main_thread();
        cbindgen_private::sixtyfps_windowrc_clone(&other.inner, &inner);
    }
    WindowRc(WindowRc &&) = delete;
    WindowRc &operator=(WindowRc &&) = delete;
    WindowRc &operator=(const WindowRc &other)
    {
        assert_main_thread();
        if (this != &other) {
            cbindgen_private::sixtyfps_windowrc_drop(&inner);
            cbindgen_private::sixtyfps_windowrc_clone(&other.inner, &inner);
        }
        return *this;
    }

    void show() const { sixtyfps_windowrc_show(&inner); }
    void hide() const { sixtyfps_windowrc_hide(&inner); }

    float scale_factor() const { return sixtyfps_windowrc_get_scale_factor(&inner); }
    void set_scale_factor(float value) const { sixtyfps_windowrc_set_scale_factor(&inner, value); }

    template<typename Component, typename ItemTree>
    void free_graphics_resources(Component *c, ItemTree items) const
    {
        cbindgen_private::sixtyfps_component_free_item_graphics_resources(
                vtable::VRef<ComponentVTable> { &Component::static_vtable, c }, items, &inner);
    }

    void set_focus_item(const ComponentRc &component_rc, uintptr_t item_index)
    {
        cbindgen_private::ItemRc item_rc { component_rc, item_index };
        cbindgen_private::sixtyfps_windowrc_set_focus_item(&inner, &item_rc);
    }

    template<typename Component, typename ItemTree>
    void init_items(Component *c, ItemTree items) const
    {
        cbindgen_private::sixtyfps_component_init_items(
                vtable::VRef<ComponentVTable> { &Component::static_vtable, c }, items, &inner);
    }

    template<typename Component>
    void set_component(const Component &c) const
    {
        auto self_rc = c.self_weak.lock().value().into_dyn();
        sixtyfps_windowrc_set_component(&inner, &self_rc);
    }

    template<typename Component, typename Parent>
    void show_popup(const Parent *parent_component, cbindgen_private::Point p,
                    cbindgen_private::ItemRc parent_item) const
    {
        auto popup = Component::create(parent_component).into_dyn();
        cbindgen_private::sixtyfps_windowrc_show_popup(&inner, &popup, p, &parent_item);
    }

private:
    cbindgen_private::WindowRcOpaque inner;
};

constexpr inline ItemTreeNode make_item_node(std::uintptr_t offset,
                                             const cbindgen_private::ItemVTable *vtable,
                                             uint32_t child_count, uint32_t child_index,
                                             uint32_t parent_index)
{
    return ItemTreeNode { ItemTreeNode::Item_Body {
            ItemTreeNode::Tag::Item, { vtable, offset }, child_count, child_index, parent_index } };
}

constexpr inline ItemTreeNode make_dyn_node(std::uintptr_t offset, std::uint32_t parent_index)
{
    return ItemTreeNode { ItemTreeNode::DynamicTree_Body { ItemTreeNode::Tag::DynamicTree, offset,
                                                           parent_index } };
}

inline ItemRef get_item_ref(ComponentRef component, cbindgen_private::Slice<ItemTreeNode> item_tree, int index)
{
    const auto &item = item_tree.ptr[index].item.item;
    return ItemRef { item.vtable, reinterpret_cast<char *>(component.instance) + item.offset };
}

inline ItemWeak parent_item(cbindgen_private::ComponentWeak component,
                            cbindgen_private::Slice<ItemTreeNode> item_tree, int index)
{
    const auto &node = item_tree.ptr[index];
    if (node.tag == ItemTreeNode::Tag::Item) {
        return { component, node.item.parent_index };
    } else {
        return { component, node.dynamic_tree.parent_index };
    }
}

inline void dealloc(const ComponentVTable *, uint8_t *ptr, vtable::Layout layout)
{
#ifdef __cpp_sized_deallocation
    ::operator delete(reinterpret_cast<void *>(ptr), layout.size,
                      static_cast<std::align_val_t>(layout.align));
#else
    ::operator delete(reinterpret_cast<void *>(ptr), static_cast<std::align_val_t>(layout.align));
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
#        define SIXTYFPS_GET_ITEM_VTABLE(VTableName)                                               \
            sixtyfps::private_api::sixtyfps_get_##VTableName()
#    else
#        define SIXTYFPS_GET_ITEM_VTABLE(VTableName) (&sixtyfps::private_api::VTableName)
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
/// in the .60 file. This give you a `ComponentHandle<T>`
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
    explicit Window(const private_api::WindowRc &windowrc) : inner(windowrc) { }
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

    /// \private
    private_api::WindowRc &window_handle() { return inner; }
    /// \private
    const private_api::WindowRc &window_handle() const { return inner; }

private:
    private_api::WindowRc inner;
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
    /// `start(sixtyfps::TimerMode::Repeated, interval, callback);` on a default constructed Timer.
    template<typename F>
    Timer(std::chrono::milliseconds interval, F callback)
        : id(cbindgen_private::sixtyfps_timer_start(
                -1, TimerMode::Repeated, interval.count(),
                [](void *data) { (*reinterpret_cast<F *>(data))(); }, new F(std::move(callback)),
                [](void *data) { delete reinterpret_cast<F *>(data); }))
    {
    }
    Timer(const Timer &) = delete;
    Timer &operator=(const Timer &) = delete;
    ~Timer() { cbindgen_private::sixtyfps_timer_destroy(id); }

    /// Starts the timer with the given \a mode and \a interval, in order for the \a callback to
    /// called when the timer fires. If the timer has been started previously and not fired yet,
    /// then it will be restarted.
    template<typename F>
    void start(TimerMode mode, std::chrono::milliseconds interval, F callback)
    {
        id = cbindgen_private::sixtyfps_timer_start(
                id, mode, interval.count(), [](void *data) { (*reinterpret_cast<F *>(data))(); },
                new F(std::move(callback)), [](void *data) { delete reinterpret_cast<F *>(data); });
    }
    /// Stops the previously started timer. Does nothing if the timer has never been started. A
    /// stopped timer cannot be restarted with restart() -- instead you need to call start().
    void stop() { cbindgen_private::sixtyfps_timer_stop(id); }
    /// Restarts the timer. If the timer was previously started by calling [`Self::start()`]
    /// with a duration and callback, then the time when the callback will be next invoked
    /// is re-calculated to be in the specified duration relative to when this function is called.
    ///
    /// Does nothing if the timer was never started.
    void restart() { cbindgen_private::sixtyfps_timer_restart(id); }
    /// Returns true if the timer is running; false otherwise.
    bool running() const { return cbindgen_private::sixtyfps_timer_running(id); }

    /// Call the callback after the given duration.
    template<typename F>
    static void single_shot(std::chrono::milliseconds duration, F callback)
    {
        cbindgen_private::sixtyfps_timer_singleshot(
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
    cbindgen_private::Slice<uint32_t> ri { reinterpret_cast<uint32_t *>(repeater_indexes.ptr), repeater_indexes.len };
    cbindgen_private::sixtyfps_solve_box_layout(&data, ri, &result);
    return result;
}

inline SharedVector<float> solve_grid_layout(const cbindgen_private::GridLayoutData &data)
{
    SharedVector<float> result;
    cbindgen_private::sixtyfps_solve_grid_layout(&data, &result);
    return result;
}

inline cbindgen_private::LayoutInfo
grid_layout_info(cbindgen_private::Slice<cbindgen_private::GridLayoutCellData> cells, float spacing,
                 const cbindgen_private::Padding &padding)
{
    return cbindgen_private::sixtyfps_grid_layout_info(cells, spacing, &padding);
}

inline cbindgen_private::LayoutInfo
box_layout_info(cbindgen_private::Slice<cbindgen_private::BoxLayoutCellData> cells, float spacing,
                const cbindgen_private::Padding &padding,
                cbindgen_private::LayoutAlignment alignment)
{
    return cbindgen_private::sixtyfps_box_layout_info(cells, spacing, &padding, alignment);
}

inline cbindgen_private::LayoutInfo
box_layout_info_ortho(cbindgen_private::Slice<cbindgen_private::BoxLayoutCellData> cells,
                      const cbindgen_private::Padding &padding)
{
    return cbindgen_private::sixtyfps_box_layout_info_ortho(cells, &padding);
}

inline SharedVector<float> solve_path_layout(const cbindgen_private::PathLayoutData &data,
                                             cbindgen_private::Slice<int> repeater_indexes)
{
    SharedVector<float> result;
    cbindgen_private::Slice<uint32_t> ri { reinterpret_cast<uint32_t *>(repeater_indexes.ptr), repeater_indexes.len };
    cbindgen_private::sixtyfps_solve_path_layout(&data, ri, &result);
    return result;
}

/// Access the layout cache of an item within a repeater
inline float layout_cache_access(const SharedVector<float> &cache, int offset, int repeater_index)
{
    size_t idx = size_t(cache[offset]) + repeater_index * 2;
    return idx < cache.size() ? cache[idx] : 0;
}

// models
struct AbstractRepeaterView
{
    virtual ~AbstractRepeaterView() = default;
    virtual void row_added(int index, int count) = 0;
    virtual void row_removed(int index, int count) = 0;
    virtual void row_changed(int index) = 0;
};
using ModelPeer = std::weak_ptr<AbstractRepeaterView>;

} // namespace private_api

/// \rst
/// A Model is providing Data for |Repetition|_ repetitions or |ListView|_ elements of the
/// :code:`.60` language
/// \endrst
template<typename ModelData>
class Model
{
public:
    virtual ~Model() = default;
    Model() = default;
    Model(const Model &) = delete;
    Model &operator=(const Model &) = delete;

    /// The amount of row in the model
    virtual int row_count() const = 0;
    /// Returns the data for a particular row. This function should be called with `row <
    /// row_count()`.
    virtual std::optional<ModelData> row_data(int i) const = 0;
    /// Sets the data for a particular row.
    ///
    /// This function should only be called with `row < row_count()`.
    ///
    /// If the model cannot support data changes, then it is ok to do nothing.
    /// The default implementation will print a warning to stderr.
    ///
    /// If the model can update the data, it should also call `row_changed`
    virtual void set_row_data(int, const ModelData &)
    {
        std::cerr << "Model::set_row_data was called on a read-only model" << std::endl;
    };

    /// \private
    /// Internal function called by the view to register itself
    void attach_peer(private_api::ModelPeer p) { peers.push_back(std::move(p)); }

    /// \private
    /// Internal function called from within bindings to register with the currently
    /// evaluating dependency and get notified when this model's row count changes.
    void track_row_count_changes() { model_row_count_dirty_property.get(); }

    /// \private
    /// Internal function called from within bindings to register with the currently
    /// evaluating dependency and get notified when this model's row data changes.
    void track_row_data_changes(int row)
    {
        auto it = std::lower_bound(tracked_rows.begin(), tracked_rows.end(), row);
        if (it == tracked_rows.end() || row < *it) {
            tracked_rows.insert(it, row);
        }
        model_row_data_dirty_property.get();
    }

protected:
    /// Notify the views that a specific row was changed
    void row_changed(int row)
    {
        if (std::binary_search(tracked_rows.begin(), tracked_rows.end(), row)) {
            model_row_data_dirty_property.mark_dirty();
        }
        for_each_peers([=](auto peer) { peer->row_changed(row); });
    }
    /// Notify the views that rows were added
    void row_added(int index, int count)
    {
        model_row_count_dirty_property.mark_dirty();
        tracked_rows.clear();
        model_row_data_dirty_property.mark_dirty();
        for_each_peers([=](auto peer) { peer->row_added(index, count); });
    }
    /// Notify the views that rows were removed
    void row_removed(int index, int count)
    {
        model_row_count_dirty_property.mark_dirty();
        tracked_rows.clear();
        model_row_data_dirty_property.mark_dirty();
        for_each_peers([=](auto peer) { peer->row_removed(index, count); });
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
    std::vector<int> tracked_rows;
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
    int row_count() const override { return Count; }
    std::optional<ModelData> row_data(int i) const override
    {
        if (i >= row_count())
            return {};
        return data[i];
    }
    void set_row_data(int i, const ModelData &value) override
    {
        if (i < row_count()) {
            data[i] = value;
            this->row_changed(i);
        }
    }
};

/// Model to be used when we just want to repeat without data.
struct IntModel : Model<int>
{
    /// Constructs a new IntModel with \a d rows.
    IntModel(int d) : data(d) { }
    /// \private
    int data;
    /// \copydoc Model::row_count
    int row_count() const override { return data; }
    std::optional<int> row_data(int value) const override
    {
        if (value >= row_count())
            return {};
        return value;
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
    int row_count() const override { return int(data.size()); }
    std::optional<ModelData> row_data(int i) const override
    {
        if (i >= row_count())
            return {};
        return std::optional<ModelData> { data[i] };
    }
    void set_row_data(int i, const ModelData &value) override
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
        this->row_added(int(data.size()) - 1, 1);
    }

    /// Remove the row at the given index from the model
    void erase(int index)
    {
        data.erase(data.begin() + index);
        this->row_removed(index, 1);
    }

    /// Inserts the given value as a new row at the specified index
    void insert(size_t index, const ModelData &value)
    {
        data.insert(data.begin() + index, value);
        this->row_added(int(index), 1);
    }
};

namespace private_api {

template<typename C, typename ModelData>
class Repeater
{
    private_api::Property<std::shared_ptr<Model<ModelData>>> model;

    struct RepeaterInner : AbstractRepeaterView
    {
        enum class State { Clean, Dirty };
        struct ComponentWithState
        {
            State state = State::Dirty;
            std::optional<ComponentHandle<C>> ptr;
        };
        std::vector<ComponentWithState> data;
        private_api::Property<bool> is_dirty { true };

        void row_added(int index, int count) override
        {
            is_dirty.set(true);
            data.resize(data.size() + count);
            std::rotate(data.begin() + index, data.end() - count, data.end());
        }
        void row_changed(int index) override
        {
            is_dirty.set(true);
            data[index].state = State::Dirty;
        }
        void row_removed(int index, int count) override
        {
            is_dirty.set(true);
            data.erase(data.begin() + index, data.begin() + index + count);
            for (std::size_t i = index; i < data.size(); ++i) {
                // all the indexes are dirty
                data[i].state = State::Dirty;
            }
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
            inner = std::make_shared<RepeaterInner>();
            if (auto m = model.get()) {
                m->attach_peer(inner);
            }
        }

        if (inner && inner->is_dirty.get()) {
            inner->is_dirty.set(false);
            if (auto m = model.get()) {
                int count = m->row_count();
                inner->data.resize(count);
                for (int i = 0; i < count; ++i) {
                    auto &c = inner->data[i];
                    if (!c.ptr) {
                        c.ptr = C::create(parent);
                    }
                    if (c.state == RepeaterInner::State::Dirty) {
                        (*c.ptr)->update_data(i, *m->row_data(i));
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

    uintptr_t visit(TraversalOrder order, private_api::ItemVisitorRefMut visitor) const
    {
        for (std::size_t i = 0; i < inner->data.size(); ++i) {
            int index = order == TraversalOrder::BackToFront ? i : inner->data.size() - 1 - i;
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

    void model_set_row_data(int row, const ModelData &data) const
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

} // namespace private_api

#if !defined(DOXYGEN)
cbindgen_private::Flickable::Flickable()
{
    sixtyfps_flickable_data_init(&data);
}
cbindgen_private::Flickable::~Flickable()
{
    sixtyfps_flickable_data_free(&data);
}

cbindgen_private::NativeStyleMetrics::NativeStyleMetrics()
{
    sixtyfps_native_style_metrics_init(this);
}

cbindgen_private::NativeStyleMetrics::~NativeStyleMetrics()
{
    sixtyfps_native_style_metrics_deinit(this);
}
#endif // !defined(DOXYGEN)

namespace private_api {
// Code generated by SixtyFPS <= 0.1.5 uses this enum with VersionCheckHelper
enum class [[deprecated]] VersionCheck { Major = SIXTYFPS_VERSION_MAJOR,
                                         Minor = SIXTYFPS_VERSION_MINOR,
                                         Patch = SIXTYFPS_VERSION_PATCH };
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
    cbindgen_private::sixtyfps_run_event_loop();
}

/// Schedules the main event loop for termination. This function is meant
/// to be called from callbacks triggered by the UI. After calling the function,
/// it will return immediately and once control is passed back to the event loop,
/// the initial call to sixtyfps::run_event_loop() will return.
inline void quit_event_loop()
{
    cbindgen_private::sixtyfps_quit_event_loop();
}

/// Adds the specified functor to an internal queue, notifies the event loop to wake up.
/// Once woken up, any queued up functors will be invoked.
/// This function is thread-safe and can be called from any thread, including the one
/// running the event loop. The provided functors will only be invoked from the thread
/// that started the event loop.
///
/// You can use this to set properties or use any other SixtyFPS APIs from other threads,
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
///     sixtyfps::ComponentWeakHandle<NetworkStatusUI> weak_ui_handle(ui);
///     std::thread network_thread([=]{
///         std::string message = read_message_blocking_from_network();
///         sixtyfps::invoke_from_event_loop([&]() {
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
template<typename Functor>
void invoke_from_event_loop(Functor f)
{
    cbindgen_private::sixtyfps_post_event(
            [](void *data) { (*reinterpret_cast<Functor *>(data))(); }, new Functor(std::move(f)),
            [](void *data) { delete reinterpret_cast<Functor *>(data); });
}

/// Blocking version of invoke_from_event_loop()
///
/// Just like invoke_from_event_loop(), this will run the specified functor from the thread running
/// the sixtyfps event loop. But it will block until the execution of the functor is finished,
/// and return that value.
///
/// This function must be called from a different thread than the thread that runs the event loop
/// otherwise it will result in a deadlock. Calling this function if the event loop is not running
/// will also block foerver or until the event loop is started in another thread.
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
///             auto message = sixtyfps::blocking_invoke_from_event_loop([ui]() {
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
template<typename Functor,
         typename = std::enable_if_t<!std::is_void_v<std::invoke_result_t<Functor>>>>
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

template<typename Functor,
         typename = std::enable_if_t<std::is_void_v<std::invoke_result_t<Functor>>>>
auto blocking_invoke_from_event_loop(Functor f) -> void
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

namespace private_api {

/// Registers a font by the specified path. The path must refer to an existing
/// TrueType font.
/// \returns an empty optional on success, otherwise an error string
inline std::optional<SharedString> register_font_from_path(const SharedString &path)
{
    SharedString maybe_err;
    cbindgen_private::sixtyfps_register_font_from_path(&path, &maybe_err);
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
    cbindgen_private::sixtyfps_register_font_from_data({ const_cast<uint8_t *>(data), len },
                                                       &maybe_err);
    if (!maybe_err.empty()) {
        return maybe_err;
    } else {
        return {};
    }
}

}

} // namespace sixtyfps
