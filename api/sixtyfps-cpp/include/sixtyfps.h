
#include "sixtyfps_internal.h"

namespace sixtyfps {

    // Bring opaque structure in scope
    using internal::ItemTreeNode;
    using internal::ComponentType;

    template<typename Component> void run(Component *c, const ComponentType *t) {
        // FIXME! some static assert that the component is indeed a generated component matching
        // the vtable.  In fact, i think the VTable should be a static member of the Component
        internal::sixtyfps_runtime_run_component(t, reinterpret_cast<internal::ComponentImpl *>(c));
    }

    using internal::Rectangle;
    using internal::RectangleVTable;
}
