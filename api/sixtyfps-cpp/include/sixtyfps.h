/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#pragma once

// In C++17, it is conditionally supported, but still valid for all compiler we care
#pragma GCC diagnostic ignored "-Winvalid-offsetof"

#include <vector>
#include <memory>
#include <algorithm>
#include <iostream> // FIXME: remove: iostream always bring it lots of code so we should not have it in this header

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
extern const cbindgen_private::ItemVTable ImageVTable;
extern const cbindgen_private::ItemVTable PathVTable;
extern const cbindgen_private::ItemVTable FlickableVTable;
extern const cbindgen_private::ItemVTable WindowVTable;
extern const cbindgen_private::ItemVTable TextInputVTable;

extern const cbindgen_private::ItemVTable NativeButtonVTable;
extern const cbindgen_private::ItemVTable NativeCheckBoxVTable;
extern const cbindgen_private::ItemVTable NativeSpinBoxVTable;
extern const cbindgen_private::ItemVTable NativeSliderVTable;
extern const cbindgen_private::ItemVTable NativeGroupBoxVTable;
}
}

// Bring opaque structure in scope
namespace private_api {
using cbindgen_private::ComponentVTable;
using cbindgen_private::ItemVTable;
using ComponentRef = VRef<private_api::ComponentVTable>;
using ItemVisitorRefMut = VRefMut<cbindgen_private::ItemVisitorVTable>;
}
using cbindgen_private::EasingCurve;
using cbindgen_private::PropertyAnimation;
using cbindgen_private::Slice;
using cbindgen_private::TextHorizontalAlignment;
using cbindgen_private::TextVerticalAlignment;
using cbindgen_private::TraversalOrder;

namespace private_api {
using ItemTreeNode = cbindgen_private::ItemTreeNode<uint8_t>;

struct ComponentWindow
{
    ComponentWindow() { cbindgen_private::sixtyfps_component_window_init(&inner); }
    ~ComponentWindow() { cbindgen_private::sixtyfps_component_window_drop(&inner); }
    ComponentWindow(const ComponentWindow &) = delete;
    ComponentWindow(ComponentWindow &&) = delete;
    ComponentWindow &operator=(const ComponentWindow &) = delete;

    template<typename Component>
    void run(Component *c)
    {
        sixtyfps_component_window_run(
                &inner, VRefMut<ComponentVTable> { &Component::component_type, c }, c->root_item());
    }

    float scale_factor() const { return sixtyfps_component_window_get_scale_factor(&inner); }
    void set_scale_factor(float value)
    {
        sixtyfps_component_window_set_scale_factor(&inner, value);
    }

    template<typename Component>
    void free_graphics_resources(Component *c) const
    {
        cbindgen_private::sixtyfps_component_window_free_graphics_resources(
                &inner, VRef<ComponentVTable> { &Component::component_type, c });
    }

private:
    cbindgen_private::ComponentWindowOpaque inner;
};
}

using cbindgen_private::BorderRectangle;
using cbindgen_private::Flickable;
using cbindgen_private::Image;
using cbindgen_private::Path;
using cbindgen_private::Rectangle;
using cbindgen_private::Text;
using cbindgen_private::TextInput;
using cbindgen_private::TouchArea;
using cbindgen_private::Window;

using cbindgen_private::NativeButton;
using cbindgen_private::NativeCheckBox;
using cbindgen_private::NativeGroupBox;
using cbindgen_private::NativeSlider;
using cbindgen_private::NativeSpinBox;

namespace private_api {
constexpr inline ItemTreeNode make_item_node(std::uintptr_t offset,
                                             const cbindgen_private::ItemVTable *vtable,
                                             uint32_t child_count, uint32_t child_index)
{
    return ItemTreeNode { ItemTreeNode::Item_Body {
            ItemTreeNode::Tag::Item, { vtable, offset }, child_count, child_index } };
}

constexpr inline ItemTreeNode make_dyn_node(std::uintptr_t offset)
{
    return ItemTreeNode { ItemTreeNode::DynamicTree_Body { ItemTreeNode::Tag::DynamicTree,
                                                           offset } };
}
}

using cbindgen_private::InputEventResult;
using cbindgen_private::MouseEvent;
using cbindgen_private::sixtyfps_visit_item_tree;
namespace private_api {
template<typename GetDynamic>
inline InputEventResult process_input_event(ComponentRef component, int64_t &mouse_grabber,
                                            MouseEvent mouse_event, Slice<ItemTreeNode> tree,
                                            GetDynamic get_dynamic, const ComponentWindow *window)
{
    if (mouse_grabber != -1) {
        auto item_index = mouse_grabber & 0xffffffff;
        auto rep_index = mouse_grabber >> 32;
        auto offset = cbindgen_private::sixtyfps_item_offset(component, tree, item_index);
        mouse_event.pos = { mouse_event.pos.x - offset.x, mouse_event.pos.y - offset.y };
        const auto &item_node = tree.ptr[item_index];
        InputEventResult result = InputEventResult::EventIgnored;
        switch (item_node.tag) {
        case ItemTreeNode::Tag::Item:
            result = item_node.item.item.vtable->input_event(
                    {
                            item_node.item.item.vtable,
                            reinterpret_cast<char *>(component.instance)
                                    + item_node.item.item.offset,
                    },
                    mouse_event, window);
            break;
        case ItemTreeNode::Tag::DynamicTree: {
            ComponentRef comp = get_dynamic(item_node.dynamic_tree.index, rep_index);
            result = comp.vtable->input_event(comp, mouse_event, window);
        } break;
        }
        if (result != InputEventResult::GrabMouse) {
            mouse_grabber = -1;
        }
        return result;
    } else {
        return cbindgen_private::sixtyfps_process_ungrabbed_mouse_event(component, mouse_event,
                                                                        window, &mouse_grabber);
    }
}
}

// layouts:
using cbindgen_private::grid_layout_info;
using cbindgen_private::GridLayoutCellData;
using cbindgen_private::GridLayoutData;
using cbindgen_private::LayoutInfo;
using cbindgen_private::Padding;
using cbindgen_private::PathLayoutData;
using cbindgen_private::PathLayoutItemData;
using cbindgen_private::solve_grid_layout;
using cbindgen_private::solve_path_layout;

// models
using ModelPeer = std::weak_ptr<bool>;

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
        (void)row;
        notify();
    }
    /// Notify the views that rows were added
    void row_added(int index, int count)
    {
        (void)(index + count);
        notify();
    }
    /// Notify the views that rows were removed
    void row_removed(int index, int count)
    {
        (void)(index + count);
        notify();
    }

private:
    void notify()
    {
        peers.erase(std::remove_if(peers.begin(), peers.end(),
                                   [](const auto &p) {
                                       if (auto pp = p.lock()) {
                                           *pp = true;
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
    void set_row_data(int i, const ModelData &value) override { data[i] = value; }
};

/// Model to be used when we just want to repeat without data.
struct IntModel : Model<int>
{
    IntModel(int d) : data(d) { }
    int data;
    int row_count() const override { return data; }
    int row_data(int value) const override { return value; }
};

/// A Model backed by a SharedArray
template<typename ModelData>
class SharedArrayModel : public Model<ModelData>
{
    SharedArray<ModelData> data;

public:
    SharedArrayModel() = default;
    SharedArrayModel(SharedArray<ModelData> array) : data(std::move(array)) { }
    int row_count() const override { return data.size(); }
    ModelData row_data(int i) const override { return data[i]; }
    void set_row_data(int i, const ModelData &value) override
    {
        data[i] = value;
        this->row_changed(i);
    }

    // Append a new row with the given value
    void push_back(const ModelData &value)
    {
        data.push_back(value);
        this->row_added(data.size() - 1, 1);
    }
};

template<typename C, typename ModelData>
class Repeater
{
    mutable std::shared_ptr<bool> is_dirty;
    Property<std::shared_ptr<Model<ModelData>>> model;

public:
    // FIXME: should be private, but compute_layout uses it.
    std::vector<std::unique_ptr<C>> data;

    template<typename F>
    void set_model_binding(F &&binding) const
    {
        model.set_binding(std::forward<F>(binding));
    }

    template<typename Parent>
    void ensure_updated(const Parent *parent) const
    {
        if (model.is_dirty()) {
            is_dirty = std::make_shared<bool>(true);
            if (auto m = model.get()) {
                m->attach_peer(is_dirty);
            }
        }
        if (is_dirty && *is_dirty) {
            auto &data = const_cast<Repeater *>(this)->data;
            data.clear();
            auto m = model.get();
            auto count = m->row_count();
            for (auto i = 0; i < count; ++i) {
                auto x = std::make_unique<C>();
                x->parent = parent;
                x->update_data(i, m->row_data(i));
                data.push_back(std::move(x));
            }
            *is_dirty = false;
        }
    }

    intptr_t visit(TraversalOrder order, private_api::ItemVisitorRefMut visitor) const
    {
        for (std::size_t i = 0; i < data.size(); ++i) {
            int index = order == TraversalOrder::BackToFront ? i : data.size() - 1 - i;
            auto ref = item_at(index);
            if (ref.vtable->visit_children_item(ref, -1, order, visitor) != -1) {
                return index;
            }
        }
        return -1;
    }

    VRef<private_api::ComponentVTable> item_at(int i) const
    {
        const auto &x = data.at(i);
        return { &C::component_type, x.get() };
    }

    void compute_layout() const
    {
        for (auto &x : data) {
            x->compute_layout({ &C::component_type, x.get() });
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

namespace private_api {
template<int Major, int Minor, int Patch>
struct VersionCheckHelper
{
};
}

} // namespace sixtyfps
