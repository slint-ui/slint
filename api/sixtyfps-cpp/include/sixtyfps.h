/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
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

namespace sixtyfps::cbindgen_private {
// Workaround https://github.com/eqrion/cbindgen/issues/43
struct ComponentVTable;
struct ItemVTable;
}
#include "sixtyfps_internal.h"
#include "sixtyfps_default_backend_internal.h"
#include "sixtyfps_qt_internal.h"

namespace sixtyfps {

namespace private_api {
extern "C" {
extern const cbindgen_private::ItemVTable RectangleVTable;
extern const cbindgen_private::ItemVTable BorderRectangleVTable;
extern const cbindgen_private::ItemVTable TextVTable;
extern const cbindgen_private::ItemVTable TouchAreaVTable;
extern const cbindgen_private::ItemVTable FocusScopeVTable;
extern const cbindgen_private::ItemVTable ImageVTable;
extern const cbindgen_private::ItemVTable ClippedImageVTable;
extern const cbindgen_private::ItemVTable PathVTable;
extern const cbindgen_private::ItemVTable FlickableVTable;
extern const cbindgen_private::ItemVTable WindowVTable;
extern const cbindgen_private::ItemVTable TextInputVTable;
extern const cbindgen_private::ItemVTable ClipVTable;
extern const cbindgen_private::ItemVTable BoxShadowVTable;
extern const cbindgen_private::ItemVTable RotateVTable;

extern const cbindgen_private::ItemVTable NativeButtonVTable;
extern const cbindgen_private::ItemVTable NativeCheckBoxVTable;
extern const cbindgen_private::ItemVTable NativeSpinBoxVTable;
extern const cbindgen_private::ItemVTable NativeSliderVTable;
extern const cbindgen_private::ItemVTable NativeGroupBoxVTable;
extern const cbindgen_private::ItemVTable NativeLineEditVTable;
extern const cbindgen_private::ItemVTable NativeScrollViewVTable;
extern const cbindgen_private::ItemVTable NativeStandardListViewItemVTable;
extern const cbindgen_private::ItemVTable NativeComboBoxVTable;
}
}

// Bring opaque structure in scope
namespace private_api {
using cbindgen_private::ComponentVTable;
using cbindgen_private::ItemVTable;
using ComponentRef = vtable::VRef<private_api::ComponentVTable>;
using ItemRef = vtable::VRef<private_api::ItemVTable>;
using ItemVisitorRefMut = vtable::VRefMut<cbindgen_private::ItemVisitorVTable>;
using cbindgen_private::ItemWeak;
}
using cbindgen_private::ComponentRc;
using cbindgen_private::EasingCurve;
using cbindgen_private::PropertyAnimation;
using cbindgen_private::Slice;
using cbindgen_private::TextHorizontalAlignment;
using cbindgen_private::TextVerticalAlignment;
using cbindgen_private::TextOverflow;
using cbindgen_private::TextWrap;
using cbindgen_private::TraversalOrder;
using cbindgen_private::ImageFit;
using cbindgen_private::KeyEvent;
using cbindgen_private::EventResult;
using cbindgen_private::KeyboardModifiers;
using cbindgen_private::FillRule;

namespace private_api {
using ItemTreeNode = cbindgen_private::ItemTreeNode<uint8_t>;

class ComponentWindow
{
public:
    ComponentWindow() { cbindgen_private::sixtyfps_component_window_init(&inner); }
    ~ComponentWindow() { cbindgen_private::sixtyfps_component_window_drop(&inner); }
    ComponentWindow(const ComponentWindow &other)
    {
        cbindgen_private::sixtyfps_component_window_clone(&other.inner, &inner);
    }
    ComponentWindow(ComponentWindow &&) = delete;
    ComponentWindow &operator=(const ComponentWindow &) = delete;

    void show() const { sixtyfps_component_window_show(&inner); }
    void hide() const { sixtyfps_component_window_hide(&inner); }

    float scale_factor() const { return sixtyfps_component_window_get_scale_factor(&inner); }
    void set_scale_factor(float value) const
    {
        sixtyfps_component_window_set_scale_factor(&inner, value);
    }

    void free_graphics_resources(const sixtyfps::Slice<ItemRef> &items) const
    {
        cbindgen_private::sixtyfps_component_window_free_graphics_resources(&inner, &items);
    }

    void set_focus_item(const ComponentRc &component_rc, uintptr_t item_index)
    {
        cbindgen_private::ItemRc item_rc{component_rc, item_index};
        cbindgen_private::sixtyfps_component_window_set_focus_item(&inner, &item_rc);
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
        sixtyfps_component_window_set_component(&inner, &self_rc);
    }

    template<typename Component, typename Parent>
    void show_popup(const Parent *parent_component, cbindgen_private::Point p) const
    {
        auto popup = Component::create(parent_component).into_dyn();
        cbindgen_private::sixtyfps_component_window_show_popup(&inner, &popup, p);
    }

private:
    cbindgen_private::ComponentWindowOpaque inner;
};
}

using cbindgen_private::BorderRectangle;
using cbindgen_private::Clip;
using cbindgen_private::ClippedImage;
using cbindgen_private::Flickable;
using cbindgen_private::FocusScope;
using cbindgen_private::Image;
using cbindgen_private::Path;
using cbindgen_private::Rectangle;
using cbindgen_private::Text;
using cbindgen_private::TextInput;
using cbindgen_private::TouchArea;
using cbindgen_private::Window;
using cbindgen_private::BoxShadow;
using cbindgen_private::Rotate;

using cbindgen_private::NativeButton;
using cbindgen_private::NativeCheckBox;
using cbindgen_private::NativeComboBox;
using cbindgen_private::NativeGroupBox;
using cbindgen_private::NativeLineEdit;
using cbindgen_private::NativeScrollView;
using cbindgen_private::NativeSlider;
using cbindgen_private::NativeSpinBox;
using cbindgen_private::NativeStandardListViewItem;
using cbindgen_private::NativeStyleMetrics;

namespace private_api {
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
    return ItemTreeNode { ItemTreeNode::DynamicTree_Body {
        ItemTreeNode::Tag::DynamicTree, offset, parent_index } };
}

inline ItemRef get_item_ref(ComponentRef component, Slice<ItemTreeNode> item_tree, int index)
{
    const auto &item = item_tree.ptr[index].item.item;
    return ItemRef { item.vtable, reinterpret_cast<char *>(component.instance) + item.offset };
}

inline ItemWeak parent_item(cbindgen_private::ComponentWeak component,
                                Slice<ItemTreeNode> item_tree, int index)
{
    const auto &node = item_tree.ptr[index];
    if (node.tag == ItemTreeNode::Tag::Item) {
        return { component, node.item.parent_index };
    } else {
        return { component, node.dynamic_tree.parent_index };
    }
}

}

using cbindgen_private::FocusEvent;
using cbindgen_private::InputEventResult;
using cbindgen_private::KeyEvent;
using cbindgen_private::KeyEventResult;
using cbindgen_private::MouseEvent;
using cbindgen_private::sixtyfps_visit_item_tree;
namespace private_api {

void dealloc(const ComponentVTable *, uint8_t *ptr, vtable::Layout layout)
{
#ifdef __cpp_sized_deallocation
    ::operator delete(reinterpret_cast<void *>(ptr), layout.size,
                      static_cast<std::align_val_t>(layout.align));
#else
    ::operator delete(reinterpret_cast<void *>(ptr), static_cast<std::align_val_t>(layout.align));
#endif
}

template<typename T>
vtable::Layout drop_in_place(ComponentRef component)
{
    reinterpret_cast<T *>(component.instance)->~T();
    return vtable::Layout { sizeof(T), alignof(T) };
}

template<typename T> struct ReturnWrapper {
    ReturnWrapper(T val) : value(std::move(val)) {}
    T value;
};
template<> struct ReturnWrapper<void> {};
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

    const T *operator->() const { return inner.operator->(); }
    const T &operator*() const { return inner.operator*(); }

    /// internal function that returns the internal handle
    vtable::VRc<private_api::ComponentVTable> into_dyn() const {
        return inner.into_dyn();
    }
};

/// A weak reference to the component. Can be constructed from a `ComponentHandle<T>`
template<typename T>
class ComponentWeakHandle
{
    vtable::VWeak<private_api::ComponentVTable, T> inner;

public:
    ComponentWeakHandle() = default;
    ComponentWeakHandle(const ComponentHandle<T> &other) : inner(other.inner) { }
    std::optional<ComponentHandle<T>> lock() const
    {
        if (auto l = inner.lock()) {
            return { ComponentHandle(*l) };
        } else {
            return {};
        }
    }
};

/// A Timer that can call a callback at repeated interval
///
/// Use the static single_shot function to make a single shot timer
struct Timer {
    /// Construct a timer which will repeat the callback every `duration` milliseconds until
    /// the destructor of the timer is called.
    template<typename F>
    Timer(std::chrono::milliseconds duration, F callback)
        : id(cbindgen_private::sixtyfps_timer_start(
            duration.count(),
            [](void *data) { (*reinterpret_cast<F*>(data))(); },
            new F(std::move(callback)),
            [](void *data) { delete reinterpret_cast<F*>(data); }))
        {}
    Timer(const Timer&) = delete;
    Timer &operator=(const Timer&) = delete;
    ~Timer() {
        cbindgen_private::sixtyfps_timer_stop(id);
    }

    /// Call the callback after the given duration.
    template<typename F>
    static void single_shot(std::chrono::milliseconds duration, F callback) {
        cbindgen_private::sixtyfps_timer_singleshot(
            duration.count(),
            [](void *data) { (*reinterpret_cast<F*>(data))(); },
            new F(std::move(callback)),
            [](void *data) { delete reinterpret_cast<F*>(data); });
    }

private:
    int64_t id;
};


// layouts:
using cbindgen_private::sixtyfps_box_layout_info;
using cbindgen_private::BoxLayoutCellData;
using cbindgen_private::BoxLayoutData;
using cbindgen_private::sixtyfps_grid_layout_info;
using cbindgen_private::GridLayoutCellData;
using cbindgen_private::GridLayoutData;
using cbindgen_private::LayoutAlignment;
using cbindgen_private::LayoutInfo;
using cbindgen_private::Padding;
using cbindgen_private::PathLayoutData;
using cbindgen_private::PathLayoutItemData;
using cbindgen_private::Rect;
using cbindgen_private::sixtyfps_solve_box_layout;
using cbindgen_private::sixtyfps_solve_grid_layout;
using cbindgen_private::sixtyfps_solve_path_layout;

inline LayoutInfo LayoutInfo::merge(const LayoutInfo &other) const
{
    // Note: This "logic" is duplicated from LayoutInfo::merge in layout.rs.
    return LayoutInfo { std::max(min_width, other.min_width),
                        std::min(max_width, other.max_width),
                        std::max(min_height, other.min_height),
                        std::min(max_height, other.max_height),
                        std::max(min_width_percent, other.min_width_percent),
                        std::min(max_width_percent, other.max_width_percent),
                        std::max(min_height_percent, other.min_height_percent),
                        std::min(max_height_percent, other.max_height_percent),
                        std::min(horizontal_stretch, other.horizontal_stretch),
                        std::min(vertical_stretch, other.vertical_stretch) };
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
    virtual ModelData row_data(int i) const = 0;
    /// Sets the data for a particular row. This function should be called with `row < row_count()`.
    /// If the model cannot support data changes, then it is ok to do nothing (default
    /// implementation). If the model can update the data, the implmentation should also call
    /// row_changed.
    virtual void set_row_data(int, const ModelData &) {};

    /// Internal function called by the view to register itself
    void attach_peer(ModelPeer p) { peers.push_back(std::move(p)); }

protected:
    /// Notify the views that a specific row was changed
    void row_changed(int row)
    {
        for_each_peers([=](auto peer) { peer->row_changed(row); });
    }
    /// Notify the views that rows were added
    void row_added(int index, int count)
    {
        for_each_peers([=](auto peer) { peer->row_added(index, count); });
    }
    /// Notify the views that rows were removed
    void row_removed(int index, int count)
    {
        for_each_peers([=](auto peer) { peer->row_removed(index, count); });
    }

private:
    template<typename F>
    void for_each_peers(const F &f)
    {
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
    std::vector<ModelPeer> peers;
};

/// A Model backed by an array of constant size
template<int Count, typename ModelData>
class ArrayModel : public Model<ModelData>
{
    std::array<ModelData, Count> data;

public:
    template<typename... A>
    ArrayModel(A &&... a) : data { std::forward<A>(a)... }
    {
    }
    int row_count() const override { return Count; }
    ModelData row_data(int i) const override { return data[i]; }
    void set_row_data(int i, const ModelData &value) override
    {
        data[i] = value;
        this->row_changed(i);
    }
};

/// Model to be used when we just want to repeat without data.
struct IntModel : Model<int>
{
    IntModel(int d) : data(d) { }
    int data;
    int row_count() const override { return data; }
    int row_data(int value) const override { return value; }
};

/// A Model backed by a SharedVector
template<typename ModelData>
class VectorModel : public Model<ModelData>
{
    std::vector<ModelData> data;

public:
    VectorModel() = default;
    VectorModel(std::vector<ModelData> array) : data(std::move(array)) { }
    int row_count() const override { return data.size(); }
    ModelData row_data(int i) const override { return data[i]; }
    void set_row_data(int i, const ModelData &value) override
    {
        data[i] = value;
        this->row_changed(i);
    }

    /// Append a new row with the given value
    void push_back(const ModelData &value)
    {
        data.push_back(value);
        this->row_added(data.size() - 1, 1);
    }

    /// Remove the row at the given index from the model
    void erase(int index)
    {
        data.erase(data.begin() + index);
        this->row_removed(index, 1);
    }
};

template<typename C, typename ModelData>
class Repeater
{
    Property<std::shared_ptr<Model<ModelData>>> model;

    struct RepeaterInner : AbstractRepeaterView
    {
        enum class State { Clean, Dirty };
        struct ComponentWithState
        {
            State state = State::Dirty;
            std::optional<ComponentHandle<C>> ptr;
        };
        std::vector<ComponentWithState> data;
        bool is_dirty = true;

        void row_added(int index, int count) override
        {
            is_dirty = true;
            data.resize(data.size() + count);
            std::rotate(data.begin() + index, data.end() - count, data.end());
        }
        void row_changed(int index) override
        {
            is_dirty = true;
            data[index].state = State::Dirty;
        }
        void row_removed(int index, int count) override
        {
            is_dirty = true;
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

        if (inner && inner->is_dirty) {
            inner->is_dirty = false;
            if (auto m = model.get()) {
                int count = m->row_count();
                inner->data.resize(count);
                for (int i = 0; i < count; ++i) {
                    auto &c = inner->data[i];
                    if (!c.ptr) {
                        c.ptr = C::create(parent);
                    }
                    if (c.state == RepeaterInner::State::Dirty) {
                        (*c.ptr)->update_data(i, m->row_data(i));
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
    void ensure_updated_listview(const Parent *parent, const Property<float> *viewport_width,
                                 const Property<float> *viewport_height,
                                 [[maybe_unused]] const Property<float> *viewport_y,
                                 float listview_width, [[maybe_unused]] float listview_height) const
    {
        // TODO: the rust code in model.rs try to only allocate as many items as visible items
        ensure_updated(parent);

        float h = compute_layout_listview(viewport_width, listview_width);
        viewport_height->set(h);
    }

    intptr_t visit(TraversalOrder order, private_api::ItemVisitorRefMut visitor) const
    {
        for (std::size_t i = 0; i < inner->data.size(); ++i) {
            int index = order == TraversalOrder::BackToFront ? i : inner->data.size() - 1 - i;
            auto ref = item_at(index);
            if (ref.vtable->visit_children_item(ref, -1, order, visitor) != -1) {
                return index;
            }
        }
        return -1;
    }

    vtable::VRef<private_api::ComponentVTable> item_at(int i) const
    {
        const auto &x = inner->data.at(i);
        return { &C::static_vtable, const_cast<C *>(&(**x.ptr)) };
    }

    void compute_layout(cbindgen_private::Rect parent_rect) const
    {
        if (!inner)
            return;
        for (auto &x : inner->data) {
            (*x.ptr)->apply_layout({ &C::static_vtable, const_cast<C *>(&(**x.ptr)) },
                                   parent_rect);
        }
    }

    float compute_layout_listview(const Property<float> *viewport_width, float listview_width) const
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
            m->set_row_data(row, data);
            if (inner && inner->is_dirty) {
                auto &c = inner->data[row];
                if (c.state == RepeaterInner::State::Dirty && c.ptr) {
                    (*c.ptr)->update_data(row, m->row_data(row));
                }
            }
        }
    }
};

Flickable::Flickable()
{
    sixtyfps_flickable_data_init(&data);
}
Flickable::~Flickable()
{
    sixtyfps_flickable_data_free(&data);
}

NativeStyleMetrics::NativeStyleMetrics()
{
    sixtyfps_init_native_style_metrics(this);
}

using cbindgen_private::StandardListViewItem;
namespace cbindgen_private {
bool operator==(const StandardListViewItem &a, const StandardListViewItem &b)
{
    static_assert(sizeof(StandardListViewItem) == sizeof(std::tuple<SharedString>),
                  "must update to cover all fields");
    return a.text == b.text;
}
bool operator!=(const StandardListViewItem &a, const StandardListViewItem &b)
{
    return !(a == b);
}
}

namespace private_api {
template<int Major, int Minor, int Patch>
struct VersionCheckHelper
{
};
}

void run_event_loop()
{
    cbindgen_private::sixtyfps_run_event_loop();
}

/// Registers a font by the specified path. The path must refer to an existing
/// TrueType font font.
/// \returns an empty optional on success, otherwise an error string
std::optional<SharedString> register_font_from_path(const SharedString &path)
{
    SharedString maybe_err;
    cbindgen_private::sixtyfps_register_font_from_path(&path, &maybe_err);
    if (!maybe_err.empty()) {
        return maybe_err;
    } else {
        return {};
    }
}

} // namespace sixtyfps
