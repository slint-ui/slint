
using str = char; //FIXME: this is just required because of something wrong with &str in cbindgen, but one should not have &str anyway
namespace sixtyfps::internal { struct ComponentType; } // Woraround https://github.com/eqrion/cbindgen/issues/43
#include "sixtyfps_internal.h"

namespace sixtyfps {

    // Bring opaque structure in scope
    using internal::ItemTreeNode;
    using internal::ComponentType;

    template<typename Component> void run(Component *c) {
        // FIXME! some static assert that the component is indeed a generated component matching
        // the vtable.  In fact, i think the VTable should be a static member of the Component
        internal::sixtyfps_runtime_run_component(&Component::component_type, reinterpret_cast<internal::ComponentImpl *>(c));
    }

    using internal::Rectangle;
    using internal::RectangleVTable;
    using internal::Image;
    using internal::ImageVTable;

    // the component has static lifetime so it does not need to be destroyed
    // FIXME: we probably need some kind of way to dinstinguish static component and these on the heap
    inline void dummy_destory(const ComponentType *, internal::ComponentImpl *) {}

    constexpr inline ItemTreeNode make_item_node(std::intptr_t offset, const internal::ItemVTable *vtable, uint32_t child_count, uint32_t child_index) {
        return ItemTreeNode{ ItemTreeNode::Tag::Item , { ItemTreeNode::Item_Body{offset, vtable, child_count, child_index} } };
    }
}
