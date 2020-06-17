#pragma once

#include <vector>
#include <memory>

namespace sixtyfps::internal {
// Workaround https://github.com/eqrion/cbindgen/issues/43
struct ComponentVTable;
struct ItemVTable;
}
#include "sixtyfps_internal.h"
#include "sixtyfps_gl_internal.h"

namespace sixtyfps {

extern "C" {
extern const internal::ItemVTable RectangleVTable;
extern const internal::ItemVTable TextVTable;
extern const internal::ItemVTable TouchAreaVTable;
extern const internal::ItemVTable ImageVTable;
}

// Bring opaque structure in scope
using internal::ComponentVTable;
using internal::ItemTreeNode;
using ComponentRef = VRef<ComponentVTable>;
using ItemVisitorRefMut = VRefMut<internal::ItemVisitorVTable>;

template<typename Component>
void run(Component *c)
{
    // FIXME! some static assert that the component is indeed a generated
    // component matching the vtable.  In fact, i think the VTable should be a
    // static member of the Component
    internal::sixtyfps_runtime_run_component_with_gl_renderer(
            VRefMut<ComponentVTable> { &Component::component_type, c });
}

using internal::EvaluationContext;
using internal::Image;
using internal::Rectangle;
using internal::Text;
using internal::TouchArea;

// the component has static lifetime so it does not need to be destroyed
// FIXME: we probably need some kind of way to dinstinguish static component and
// these on the heap
inline void dummy_destory(ComponentRef) { }

constexpr inline ItemTreeNode<uint8_t> make_item_node(std::uintptr_t offset,
                                             const internal::ItemVTable *vtable,
                                             uint32_t child_count, uint32_t child_index)
{
    return ItemTreeNode<uint8_t> { ItemTreeNode<uint8_t>::Item_Body {
        ItemTreeNode<uint8_t>::Tag::Item, {vtable, offset}, child_count, child_index } };
}

constexpr inline ItemTreeNode<uint8_t> make_dyn_node(std::uintptr_t offset)
{
    return ItemTreeNode<uint8_t> { ItemTreeNode<uint8_t>::DynamicTree_Body {
        ItemTreeNode<uint8_t>::Tag::DynamicTree, offset } };
}

using internal::sixtyfps_visit_item_tree;

// layouts:
using internal::Slice;
using internal::solve_grid_layout;
using internal::GridLayoutCellData;
using internal::GridLayoutData;
using internal::Constraint;

// models

template<typename C>
struct Repeater {
    std::vector<std::unique_ptr<C>> data;

    // FIXME: use array_view (aka Slice)
    void update_model(void *, int count) {
        data.clear();
        for (auto i = 0; i < count; ++i) {
            auto x = std::make_unique<C>();
            x->update_data(i, nullptr);
            data.push_back(std::move(x));
        }
    }

    void visit(ItemVisitorRefMut visitor) const {
        for (const auto &x : data) {
            VRef<ComponentVTable> ref{&C::component_type, x.get()};
            ref.vtable->visit_children_item(ref, -1, visitor);
        }
    }
};


} // namespace sixtyfps
