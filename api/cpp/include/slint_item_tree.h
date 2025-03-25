// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "slint_internal.h"
#include "slint_window.h"

namespace slint {
namespace private_api {
// Bring opaque structure in scope
using namespace cbindgen_private;
using ItemTreeRef = vtable::VRef<private_api::ItemTreeVTable>;
using IndexRange = cbindgen_private::IndexRange;
using ItemRef = vtable::VRef<private_api::ItemVTable>;
using ItemVisitorRefMut = vtable::VRefMut<cbindgen_private::ItemVisitorVTable>;
using ItemTreeNode = cbindgen_private::ItemTreeNode;
using ItemArrayEntry =
        vtable::VOffset<uint8_t, slint::cbindgen_private::ItemVTable, vtable::AllowPin>;
using ItemArray = slint::cbindgen_private::Slice<ItemArrayEntry>;

constexpr inline ItemTreeNode make_item_node(uint32_t child_count, uint32_t child_index,
                                             uint32_t parent_index, uint32_t item_array_index,
                                             bool is_accessible)
{
    return ItemTreeNode { ItemTreeNode::Item_Body { ItemTreeNode::Tag::Item, is_accessible,
                                                    child_count, child_index, parent_index,
                                                    item_array_index } };
}

constexpr inline ItemTreeNode make_dyn_node(std::uint32_t offset, std::uint32_t parent_index)
{
    return ItemTreeNode { ItemTreeNode::DynamicTree_Body { ItemTreeNode::Tag::DynamicTree, offset,
                                                           parent_index } };
}

inline ItemRef get_item_ref(ItemTreeRef item_tree,
                            const cbindgen_private::Slice<ItemTreeNode> item_tree_array,
                            const private_api::ItemArray item_array, int index)
{
    const auto item_array_index = item_tree_array.ptr[index].item.item_array_index;
    const auto item = item_array[item_array_index];
    return ItemRef { item.vtable, reinterpret_cast<char *>(item_tree.instance) + item.offset };
}

} // namespace private_api

template<typename T>
class ComponentWeakHandle;

/// The component handle is like a shared pointer to a component in the generated code.
/// In order to get a component, use `T::create()` where T is the name of the component
/// in the .slint file. This give you a `ComponentHandle<T>`
template<typename T>
class ComponentHandle
{
    vtable::VRc<private_api::ItemTreeVTable, T> inner;
    friend class ComponentWeakHandle<T>;

public:
    /// internal constructor
    ComponentHandle(const vtable::VRc<private_api::ItemTreeVTable, T> &inner) : inner(inner) { }

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
    vtable::VRc<private_api::ItemTreeVTable> into_dyn() const { return inner.into_dyn(); }
};

/// A weak reference to the component. Can be constructed from a `ComponentHandle<T>`
template<typename T>
class ComponentWeakHandle
{
    vtable::VWeak<private_api::ItemTreeVTable, T> inner;

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

}
