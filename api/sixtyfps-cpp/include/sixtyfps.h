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

namespace sixtyfps::cbindgen_private {
// Workaround https://github.com/eqrion/cbindgen/issues/43
struct ComponentVTable;
struct ItemVTable;
}
#include "sixtyfps_internal.h"
#include "sixtyfps_gl_internal.h"

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
}
}

// Bring opaque structure in scope
namespace private_api {
using cbindgen_private::ComponentVTable;
using cbindgen_private::ItemVTable;
}
using ComponentRef = VRef<private_api::ComponentVTable>;
using ItemVisitorRefMut = VRefMut<cbindgen_private::ItemVisitorVTable>;
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
    ComponentWindow() { cbindgen_private::sixtyfps_component_window_gl_renderer_init(&inner); }
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
using cbindgen_private::TouchArea;
using cbindgen_private::Window;

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
                                            GetDynamic get_dynamic)
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
                    mouse_event);
            break;
        case ItemTreeNode::Tag::DynamicTree: {
            ComponentRef comp = get_dynamic(item_node.dynamic_tree.index, rep_index);
            result = comp.vtable->input_event(comp, mouse_event);
        } break;
        }
        if (result != InputEventResult::GrabMouse) {
            mouse_grabber = -1;
        }
        return result;
    } else {
        return cbindgen_private::sixtyfps_process_ungrabbed_mouse_event(component, mouse_event,
                                                                        &mouse_grabber);
    }
}
}

// layouts:
using cbindgen_private::grid_layout_info;
using cbindgen_private::GridLayoutCellData;
using cbindgen_private::GridLayoutData;
using cbindgen_private::LayoutInfo;
using cbindgen_private::PathLayoutData;
using cbindgen_private::PathLayoutItemData;
using cbindgen_private::solve_grid_layout;
using cbindgen_private::solve_path_layout;

// models

struct Model
{
    virtual ~Model() = default;
    Model() = default;
    Model(const Model &) = delete;
    Model &operator=(const Model &) = delete;
    virtual int count() const = 0;
    virtual const void *get(int i) const = 0;
};

template<int Count, typename ModelData>
struct ArrayModel : Model
{
    std::array<ModelData, Count> data;
    template<typename... A>
    ArrayModel(A &&... a) : data { std::forward<A>(a)... }
    {
    }
    ArrayModel(int x) { }
    int count() const override { return Count; }
    const void *get(int i) const override { return &data[i]; }
};

struct IntModel : Model
{
    IntModel(int d) : data(d) { }
    int data;
    int count() const override { return data; }
    const void *get(int) const override { return &data; }
};

template<typename C>
struct Repeater
{
    std::vector<std::unique_ptr<C>> data;

    template<typename Parent>
    void update_model(Model *model, const Parent *parent) const
    {
        auto &data = const_cast<Repeater *>(this)->data;
        data.clear();
        auto count = model->count();
        for (auto i = 0; i < count; ++i) {
            auto x = std::make_unique<C>();
            x->parent = parent;
            x->update_data(i, model->get(i));
            data.push_back(std::move(x));
        }
    }

    intptr_t visit(TraversalOrder order, ItemVisitorRefMut visitor) const
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
