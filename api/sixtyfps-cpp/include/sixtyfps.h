#pragma once
namespace sixtyfps::internal {
// Workaround https://github.com/eqrion/cbindgen/issues/43
struct ComponentVTable;
struct ItemVTable;
}
#include "sixtyfps_internal.h"
#include "sixtyfps_gl_internal.h"

namespace sixtyfps {

// Bring opaque structure in scope
using internal::ComponentVTable;
using internal::ItemTreeNode;
using ComponentRef = VRefMut<ComponentVTable>;

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
using internal::ImageVTable;
using internal::Rectangle;
using internal::RectangleVTable;
using internal::Text;
using internal::TextVTable;
using internal::TouchArea;
using internal::TouchAreaVTable;

// the component has static lifetime so it does not need to be destroyed
// FIXME: we probably need some kind of way to dinstinguish static component and
// these on the heap
inline void dummy_destory(ComponentRef) { }

constexpr inline ItemTreeNode make_item_node(std::intptr_t offset,
                                             const internal::ItemVTable *vtable,
                                             uint32_t child_count, uint32_t child_index)
{
    return ItemTreeNode { ItemTreeNode::Tag::Item,
                          { ItemTreeNode::Item_Body { offset, vtable, child_count,
                                                      child_index } } };
}
} // namespace sixtyfps
