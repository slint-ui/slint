// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "private/slint_internal.h"
#include "private/slint_window.h"

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

using ItemTreeDescriptor = cbindgen_private::ItemTreeDescriptor;
using ItemIndexTables = cbindgen_private::ItemIndexTables;
using RepeaterSpan = cbindgen_private::RepeaterSpan;
using RepeaterSpanKind = cbindgen_private::RepeaterSpanKind;
using LocalRepeaterEntry = cbindgen_private::LocalRepeaterEntry;
using SubComponentTableEntry = cbindgen_private::SubComponentTableEntry;
using ItemTreeWeak = cbindgen_private::ItemTreeWeak;

/// An empty Slice. The pointer must stay non-null (it mirrors Rust's
/// `NonNull::dangling()`), so use the alignment as the address.
template<typename T>
inline cbindgen_private::Slice<T> empty_slice()
{
    return cbindgen_private::Slice<T> { reinterpret_cast<T *>(alignof(T)), 0 };
}

/// A Slice over the bytes of a string literal (must be valid UTF-8).
inline cbindgen_private::Slice<uint8_t> make_str_slice(std::string_view str)
{
    return cbindgen_private::Slice<uint8_t> {
        const_cast<uint8_t *>(reinterpret_cast<const uint8_t *>(str.data())), str.size()
    };
}

/// The `LocalRepeaterEntry` hook implementations for a `Repeater<C>` or
/// `Conditional<C>` field of `Base` (the entry's offset locates the field).
template<typename Base, typename RepeaterType>
struct RepeaterEntryShims
{
    static const RepeaterType &rep(const uint8_t *inst, const LocalRepeaterEntry *e)
    {
        return *reinterpret_cast<const RepeaterType *>(inst + e->offset);
    }
    static cbindgen_private::VisitChildrenResult
    visit(const uint8_t *inst, const LocalRepeaterEntry *e,
          cbindgen_private::TraversalOrder order, ItemVisitorRefMut visitor)
    {
        return rep(inst, e).visit(order, visitor);
    }
    static IndexRange range(const uint8_t *inst, const LocalRepeaterEntry *e)
    {
        const auto &r = rep(inst, e);
        r.track_instance_changes();
        return r.index_range();
    }
    static void instance_at(const uint8_t *inst, const LocalRepeaterEntry *e, uintptr_t index,
                            ItemTreeWeak *result)
    {
        *result = rep(inst, e).instance_at(index);
    }
    static bool ensure(const uint8_t *inst, const LocalRepeaterEntry *e)
    {
        return rep(inst, e).ensure_updated(reinterpret_cast<const Base *>(inst));
    }
};

constexpr inline RepeaterSpan make_local_repeater_span(uint32_t index, LocalRepeaterEntry entry)
{
    return RepeaterSpan { index, index,
                          RepeaterSpanKind { .local = { RepeaterSpanKind::Tag::Local, entry } } };
}

/// A span for a plain `Repeater` or `Conditional` field.
template<typename Base, typename RepeaterType>
constexpr inline RepeaterSpan make_repeater_span(uint32_t index, uintptr_t offset)
{
    using Shims = RepeaterEntryShims<Base, RepeaterType>;
    return make_local_repeater_span(
            index, LocalRepeaterEntry { offset, &Shims::visit, &Shims::range,
                                        &Shims::instance_at, &Shims::ensure });
}

/// A span for a ListView repeater: visiting and instantiation track the
/// viewport, so those two hooks are component functions.
template<typename Base, typename RepeaterType>
constexpr inline RepeaterSpan make_listview_repeater_span(
        uint32_t index, uintptr_t offset,
        cbindgen_private::VisitChildrenResult (*visit)(const uint8_t *,
                                                       const LocalRepeaterEntry *,
                                                       cbindgen_private::TraversalOrder,
                                                       ItemVisitorRefMut),
        bool (*ensure)(const uint8_t *, const LocalRepeaterEntry *))
{
    using Shims = RepeaterEntryShims<Base, RepeaterType>;
    return make_local_repeater_span(
            index, LocalRepeaterEntry { offset, visit, &Shims::range, &Shims::instance_at,
                                        ensure });
}

/// A constant geometry table field.
constexpr inline cbindgen_private::GeometryField make_geometry_field_fixed(float value)
{
    return cbindgen_private::GeometryField {
        .fixed = { cbindgen_private::GeometryField::Tag::Fixed, value }
    };
}

/// A geometry table field reading the `Property<float>` at `offset` within the
/// component.
constexpr inline cbindgen_private::GeometryField make_geometry_field_offset(uintptr_t offset)
{
    return cbindgen_private::GeometryField {
        .offset = { cbindgen_private::GeometryField::Tag::Offset, offset }
    };
}

/// One row of the geometry table for the item at `index`.
constexpr inline cbindgen_private::GeometryTableEntry
make_geometry_entry(uint32_t index, cbindgen_private::GeometryField x,
                    cbindgen_private::GeometryField y, cbindgen_private::GeometryField width,
                    cbindgen_private::GeometryField height)
{
    return cbindgen_private::GeometryTableEntry { index,
                                                  cbindgen_private::GeometryOffsets {
                                                          x, y, width, height } };
}

/// A span for a `ComponentContainer` item at `offset` within the component.
constexpr inline RepeaterSpan make_container_span(uint32_t index, uintptr_t offset)
{
    return RepeaterSpan {
        index, index, RepeaterSpanKind { .container = { RepeaterSpanKind::Tag::Container, offset } }
    };
}

/// A span forwarding the dynamic indices `start..=end` to a nested
/// sub-component's own table, rebased to `start`.
constexpr inline RepeaterSpan make_sub_component_span(uint32_t start, uint32_t end,
                                                      uintptr_t offset,
                                                      cbindgen_private::Slice<RepeaterSpan> table)
{
    return RepeaterSpan {
        start, end, RepeaterSpanKind { .sub = { RepeaterSpanKind::Tag::Sub, offset, table } }
    };
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
