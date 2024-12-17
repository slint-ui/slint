// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#if defined(__GNUC__) || defined(__clang__)
// In C++17, it is conditionally supported, but still valid for all compiler we care
#    pragma GCC diagnostic ignored "-Winvalid-offsetof"
#endif

#include "slint_internal.h"
#include "slint_size.h"
#include "slint_point.h"
#include "slint_platform_internal.h"
#include "slint_qt_internal.h"
#include "slint_window.h"

#include <vector>
#include <memory>
#include <algorithm>
#include <chrono>
#include <optional>
#include <span>
#include <functional>
#include <concepts>

#ifndef SLINT_FEATURE_FREESTANDING
#    include <mutex>
#    include <condition_variable>
#endif

/// \rst
/// The :code:`slint` namespace is the primary entry point into the Slint C++ API.
/// All available types are in this namespace.
///
/// See the :doc:`Overview <../overview>` documentation for the C++ integration how
/// to load :code:`.slint` designs.
/// \endrst
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

/// Convert a slint `{height: length, width: length, x: length, y: length}` to a Rect
inline cbindgen_private::Rect convert_anonymous_rect(std::tuple<float, float, float, float> tuple)
{
    // alphabetical order
    auto [h, w, x, y] = tuple;
    return cbindgen_private::Rect { .x = x, .y = y, .width = w, .height = h };
}

inline void dealloc(const ItemTreeVTable *, uint8_t *ptr, [[maybe_unused]] vtable::Layout layout)
{
#ifdef __cpp_sized_deallocation
    ::operator delete(reinterpret_cast<void *>(ptr), layout.size,
                      static_cast<std::align_val_t>(layout.align));
#elif !defined(__APPLE__) || MAC_OS_X_VERSION_MIN_REQUIRED >= MAC_OS_X_VERSION_10_14
    ::operator delete(reinterpret_cast<void *>(ptr), static_cast<std::align_val_t>(layout.align));
#else
    ::operator delete(reinterpret_cast<void *>(ptr));
#endif
}

template<typename T>
inline vtable::Layout drop_in_place(ItemTreeRef item_tree)
{
    reinterpret_cast<T *>(item_tree.instance)->~T();
    return vtable::Layout { sizeof(T), alignof(T) };
}

#if !defined(DOXYGEN)
#    if defined(_WIN32) || defined(_WIN64)
// On Windows cross-dll data relocations are not supported:
//     https://docs.microsoft.com/en-us/cpp/c-language/rules-and-limitations-for-dllimport-dllexport?view=msvc-160
// so we have a relocation to a function that returns the address we seek. That
// relocation will be resolved to the locally linked stub library, the implementation of
// which will be patched.
#        define SLINT_GET_ITEM_VTABLE(VTableName) slint::private_api::slint_get_##VTableName()
#    else
#        define SLINT_GET_ITEM_VTABLE(VTableName) (&slint::private_api::VTableName)
#    endif
#endif // !defined(DOXYGEN)

inline std::optional<cbindgen_private::ItemRc>
upgrade_item_weak(const cbindgen_private::ItemWeak &item_weak)
{
    if (auto item_tree_strong = item_weak.item_tree.lock()) {
        return { { *item_tree_strong, item_weak.index } };
    } else {
        return std::nullopt;
    }
}

inline void debug(const SharedString &str)
{
    cbindgen_private::slint_debug(&str);
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
inline bool operator==(const EasingCurve &a, const EasingCurve &b)
{
    if (a.tag != b.tag) {
        return false;
    } else if (a.tag == EasingCurve::Tag::CubicBezier) {
        return std::equal(a.cubic_bezier._0, a.cubic_bezier._0 + 4, b.cubic_bezier._0);
    }
    return true;
}
}

namespace private_api {

inline static void register_item_tree(const vtable::VRc<ItemTreeVTable> *c,
                                      const std::optional<slint::Window> &maybe_window)
{
    const cbindgen_private::WindowAdapterRcOpaque *window_ptr =
            maybe_window.has_value() ? &maybe_window->window_handle().handle() : nullptr;
    cbindgen_private::slint_register_item_tree(c, window_ptr);
}

inline SharedVector<float> solve_box_layout(const cbindgen_private::BoxLayoutData &data,
                                            cbindgen_private::Slice<int> repeater_indexes)
{
    SharedVector<float> result;
    cbindgen_private::Slice<uint32_t> ri { reinterpret_cast<uint32_t *>(repeater_indexes.ptr),
                                           repeater_indexes.len };
    cbindgen_private::slint_solve_box_layout(&data, ri, &result);
    return result;
}

inline SharedVector<float> solve_grid_layout(const cbindgen_private::GridLayoutData &data)
{
    SharedVector<float> result;
    cbindgen_private::slint_solve_grid_layout(&data, &result);
    return result;
}

inline cbindgen_private::LayoutInfo
grid_layout_info(cbindgen_private::Slice<cbindgen_private::GridLayoutCellData> cells, float spacing,
                 const cbindgen_private::Padding &padding)
{
    return cbindgen_private::slint_grid_layout_info(cells, spacing, &padding);
}

inline cbindgen_private::LayoutInfo
box_layout_info(cbindgen_private::Slice<cbindgen_private::BoxLayoutCellData> cells, float spacing,
                const cbindgen_private::Padding &padding,
                cbindgen_private::LayoutAlignment alignment)
{
    return cbindgen_private::slint_box_layout_info(cells, spacing, &padding, alignment);
}

inline cbindgen_private::LayoutInfo
box_layout_info_ortho(cbindgen_private::Slice<cbindgen_private::BoxLayoutCellData> cells,
                      const cbindgen_private::Padding &padding)
{
    return cbindgen_private::slint_box_layout_info_ortho(cells, &padding);
}

/// Access the layout cache of an item within a repeater
inline float layout_cache_access(const SharedVector<float> &cache, int offset, int repeater_index)
{
    size_t idx = size_t(cache[offset]) + repeater_index * 2;
    return idx < cache.size() ? cache[idx] : 0;
}

// models
struct ModelChangeListener
{
    virtual ~ModelChangeListener() = default;
    virtual void row_added(size_t index, size_t count) = 0;
    virtual void row_removed(size_t index, size_t count) = 0;
    virtual void row_changed(size_t index) = 0;
    virtual void reset() = 0;
};
using ModelPeer = std::weak_ptr<ModelChangeListener>;

template<typename M>
auto access_array_index(const std::shared_ptr<M> &model, size_t index)
{
    if (!model) {
        return decltype(*model->row_data_tracked(index)) {};
    } else if (const auto v = model->row_data_tracked(index)) {
        return *v;
    } else {
        return decltype(*v) {};
    }
}

template<typename M>
long int model_length(const std::shared_ptr<M> &model)
{
    if (!model) {
        return 0;
    } else {
        model->track_row_count_changes();
        return model->row_count();
    }
}

} // namespace private_api

/// \rst
/// A Model is providing Data for Slint |Models|_ or |ListView|_ elements of the
/// :code:`.slint` language
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
    virtual size_t row_count() const = 0;
    /// Returns the data for a particular row. This function should be called with `row <
    /// row_count()`.
    virtual std::optional<ModelData> row_data(size_t i) const = 0;
    /// Sets the data for a particular row.
    ///
    /// This function should only be called with `row < row_count()`.
    ///
    /// If the model cannot support data changes, then it is ok to do nothing.
    /// The default implementation will print a warning to stderr.
    ///
    /// If the model can update the data, it should also call `row_changed`
    virtual void set_row_data(size_t, const ModelData &)
    {
#ifndef SLINT_FEATURE_FREESTANDING
        std::cerr << "Model::set_row_data was called on a read-only model" << std::endl;
#endif
    };

    /// \private
    /// Internal function called by the view to register itself
    void attach_peer(private_api::ModelPeer p) { peers.push_back(std::move(p)); }

    /// \private
    /// Internal function called from within bindings to register with the currently
    /// evaluating dependency and get notified when this model's row count changes.
    void track_row_count_changes() const { model_row_count_dirty_property.get(); }

    /// \private
    /// Internal function called from within bindings to register with the currently
    /// evaluating dependency and get notified when this model's row data changes.
    void track_row_data_changes(size_t row) const
    {
        auto it = std::lower_bound(tracked_rows.begin(), tracked_rows.end(), row);
        if (it == tracked_rows.end() || row < *it) {
            tracked_rows.insert(it, row);
        }
        model_row_data_dirty_property.get();
    }

    /// \private
    /// Convenience function that calls `track_row_data_changes` before returning `row_data`
    std::optional<ModelData> row_data_tracked(size_t row) const
    {
        track_row_data_changes(row);
        return row_data(row);
    }

protected:
    /// Notify the views that a specific row was changed
    void row_changed(size_t row)
    {
        if (std::binary_search(tracked_rows.begin(), tracked_rows.end(), row)) {
            model_row_data_dirty_property.mark_dirty();
        }
        for_each_peers([=](auto peer) { peer->row_changed(row); });
    }
    /// Notify the views that rows were added
    void row_added(size_t index, size_t count)
    {
        model_row_count_dirty_property.mark_dirty();
        tracked_rows.clear();
        model_row_data_dirty_property.mark_dirty();
        for_each_peers([=](auto peer) { peer->row_added(index, count); });
    }
    /// Notify the views that rows were removed
    void row_removed(size_t index, size_t count)
    {
        model_row_count_dirty_property.mark_dirty();
        tracked_rows.clear();
        model_row_data_dirty_property.mark_dirty();
        for_each_peers([=](auto peer) { peer->row_removed(index, count); });
    }

    /// Notify the views that the model has been changed and that everything needs to be reloaded
    void reset()
    {
        model_row_count_dirty_property.mark_dirty();
        tracked_rows.clear();
        model_row_data_dirty_property.mark_dirty();
        for_each_peers([=](auto peer) { peer->reset(); });
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
    mutable std::vector<size_t> tracked_rows;
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
    size_t row_count() const override { return Count; }
    std::optional<ModelData> row_data(size_t i) const override
    {
        if (i >= row_count())
            return {};
        return data[i];
    }
    void set_row_data(size_t i, const ModelData &value) override
    {
        if (i < row_count()) {
            data[i] = value;
            this->row_changed(i);
        }
    }
};

// Specialize for the empty array. We can't have a Model<void>, but `int` will work for our purpose
template<>
class ArrayModel<0, void> : public Model<int>
{
public:
    size_t row_count() const override { return 0; }
    std::optional<int> row_data(size_t) const override { return {}; }
};

/// Model to be used when we just want to repeat without data.
struct UIntModel : Model<int>
{
    /// Constructs a new IntModel with \a d rows.
    UIntModel(uint32_t d) : data(d) { }
    /// \private
    uint32_t data;
    /// \copydoc Model::row_count
    size_t row_count() const override { return data; }
    std::optional<int> row_data(size_t value) const override
    {
        if (value >= row_count())
            return {};
        return static_cast<int>(value);
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
    size_t row_count() const override { return data.size(); }
    std::optional<ModelData> row_data(size_t i) const override
    {
        if (i >= row_count())
            return {};
        return std::optional<ModelData> { data[i] };
    }
    void set_row_data(size_t i, const ModelData &value) override
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
        this->row_added(data.size() - 1, 1);
    }

    /// Remove the row at the given index from the model
    void erase(size_t index)
    {
        data.erase(data.begin() + index);
        this->row_removed(index, 1);
    }

    /// Inserts the given value as a new row at the specified index
    void insert(size_t index, const ModelData &value)
    {
        data.insert(data.begin() + index, value);
        this->row_added(index, 1);
    }

    /// Erases all rows from the VectorModel.
    void clear()
    {
        if (!data.empty()) {
            data.clear();
            this->reset();
        }
    }

    /// Replaces the underlying VectorModel's vector with \a array.
    void set_vector(std::vector<ModelData> array)
    {
        data = std::move(array);
        this->reset();
    }
};

template<typename ModelData>
class FilterModel;

namespace private_api {
template<typename ModelData>
struct FilterModelInner : private_api::ModelChangeListener
{
    FilterModelInner(std::shared_ptr<slint::Model<ModelData>> source_model,
                     std::function<bool(const ModelData &)> filter_fn,
                     slint::FilterModel<ModelData> &target_model)
        : source_model(source_model), filter_fn(filter_fn), target_model(target_model)
    {
    }

    void row_added(size_t index, size_t count) override
    {
        if (filtered_rows_dirty) {
            reset();
            return;
        }

        if (count == 0) {
            return;
        }

        std::vector<size_t> added_accepted_rows;
        for (auto i = index; i < index + count; ++i) {
            if (auto data = source_model->row_data(i)) {
                if (filter_fn(*data)) {
                    added_accepted_rows.push_back(i);
                }
            }
        }

        if (added_accepted_rows.empty()) {
            return;
        }

        auto insertion_point = std::lower_bound(accepted_rows.begin(), accepted_rows.end(), index);

        insertion_point = accepted_rows.insert(insertion_point, added_accepted_rows.begin(),
                                               added_accepted_rows.end());

        for (auto it = insertion_point + added_accepted_rows.size(); it != accepted_rows.end();
             ++it)
            (*it) += count;

        target_model.row_added(insertion_point - accepted_rows.begin(), added_accepted_rows.size());
    }
    void row_changed(size_t index) override
    {
        if (filtered_rows_dirty) {
            reset();
            return;
        }

        auto existing_row = std::lower_bound(accepted_rows.begin(), accepted_rows.end(), index);
        auto existing_row_index = std::distance(accepted_rows.begin(), existing_row);
        bool is_contained = existing_row != accepted_rows.end() && *existing_row == index;
        auto accepted_updated_row = filter_fn(*source_model->row_data(index));

        if (is_contained && accepted_updated_row) {
            target_model.row_changed(existing_row_index);
        } else if (!is_contained && accepted_updated_row) {
            accepted_rows.insert(existing_row, index);
            target_model.row_added(existing_row_index, 1);
        } else if (is_contained && !accepted_updated_row) {
            accepted_rows.erase(existing_row);
            target_model.row_removed(existing_row_index, 1);
        }
    }
    void row_removed(size_t index, size_t count) override
    {
        if (filtered_rows_dirty) {
            reset();
            return;
        }

        auto mapped_row_start = std::lower_bound(accepted_rows.begin(), accepted_rows.end(), index);
        auto mapped_row_end =
                std::lower_bound(accepted_rows.begin(), accepted_rows.end(), index + count);

        auto mapped_removed_len = std::distance(mapped_row_start, mapped_row_end);

        auto mapped_removed_index =
                (mapped_row_start != accepted_rows.end() && *mapped_row_start == index)
                ? std::optional<int>(mapped_row_start - accepted_rows.begin())
                : std::nullopt;

        auto it = accepted_rows.erase(mapped_row_start, mapped_row_end);
        for (; it != accepted_rows.end(); ++it) {
            *it -= count;
        }

        if (mapped_removed_index) {
            target_model.row_removed(*mapped_removed_index, mapped_removed_len);
        }
    }
    void reset() override
    {
        filtered_rows_dirty = true;
        update_mapping();
        target_model.Model<ModelData>::reset();
    }

    void update_mapping()
    {
        if (!filtered_rows_dirty) {
            return;
        }

        accepted_rows.clear();
        for (size_t i = 0, count = source_model->row_count(); i < count; ++i) {
            if (auto data = source_model->row_data(i)) {
                if (filter_fn(*data)) {
                    accepted_rows.push_back(i);
                }
            }
        }

        filtered_rows_dirty = false;
    }

    bool filtered_rows_dirty = true;
    std::shared_ptr<slint::Model<ModelData>> source_model;
    std::function<bool(const ModelData &)> filter_fn;
    std::vector<size_t> accepted_rows;
    slint::FilterModel<ModelData> &target_model;
};
}

/// The FilterModel acts as an adapter model for a given source model by applying a filter
/// function. The filter function is called for each row on the source model and if the
/// filter accepts the row (i.e. returns true), the row is also visible in the FilterModel.
template<typename ModelData>
class FilterModel : public Model<ModelData>
{
    friend struct private_api::FilterModelInner<ModelData>;

public:
    /// Constructs a new FilterModel that provides a limited view on the \a source_model by applying
    /// \a filter_fn on each row. If the provided function returns true, the row is exposed by the
    /// FilterModel.
    FilterModel(std::shared_ptr<Model<ModelData>> source_model,
                std::function<bool(const ModelData &)> filter_fn)
        : inner(std::make_shared<private_api::FilterModelInner<ModelData>>(
                  std::move(source_model), std::move(filter_fn), *this))
    {
        inner->source_model->attach_peer(inner);
    }

    size_t row_count() const override
    {
        inner->update_mapping();
        return inner->accepted_rows.size();
    }

    std::optional<ModelData> row_data(size_t i) const override
    {
        inner->update_mapping();
        if (i >= inner->accepted_rows.size())
            return {};
        return inner->source_model->row_data(inner->accepted_rows[i]);
    }

    void set_row_data(size_t i, const ModelData &value) override
    {
        inner->update_mapping();
        inner->source_model->set_row_data(inner->accepted_rows[i], value);
    }

    /// Re-applies the model's filter function on each row of the source model. Use this if state
    /// external to the filter function has changed.
    void reset() { inner->reset(); }

    /// Given the \a filtered_row index, this function returns the corresponding row index in the
    /// source model.
    int unfiltered_row(int filtered_row) const
    {
        inner->update_mapping();
        return inner->accepted_rows[filtered_row];
    }

    /// Returns the source model of this filter model.
    std::shared_ptr<Model<ModelData>> source_model() const { return inner->source_model; }

private:
    std::shared_ptr<private_api::FilterModelInner<ModelData>> inner;
};

template<typename SourceModelData, typename MappedModelData>
class MapModel;

namespace private_api {
template<typename SourceModelData, typename MappedModelData>
struct MapModelInner : private_api::ModelChangeListener
{
    MapModelInner(slint::MapModel<SourceModelData, MappedModelData> &target_model)
        : target_model(target_model)
    {
    }

    void row_added(size_t index, size_t count) override { target_model.row_added(index, count); }
    void row_changed(size_t index) override { target_model.row_changed(index); }
    void row_removed(size_t index, size_t count) override
    {
        target_model.row_removed(index, count);
    }
    void reset() override { target_model.Model<MappedModelData>::reset(); }

    slint::MapModel<SourceModelData, MappedModelData> &target_model;
};
}

/// The MapModel acts as an adapter model for a given source model by applying a mapping
/// function. The mapping function is called for each row on the source model and allows
/// transforming the values on the fly. The MapModel has two template parameters: The
/// SourceModelData specifies the data type of the underlying source model, and the
/// MappedModelData the data type of this MapModel. This permits not only changing the
/// values of the underlying source model, but also changing the data type itself. For
/// example a MapModel can be used to adapt a model that provides numbers to be a model
/// that exposes all numbers converted to strings, by calling `std::to_string` on each
/// value given in the mapping lambda expression.
template<typename SourceModelData, typename MappedModelData = SourceModelData>
class MapModel : public Model<MappedModelData>
{
    friend struct private_api::MapModelInner<SourceModelData, MappedModelData>;

public:
    /// Constructs a new MapModel that provides an altered view on the \a source_model by applying
    /// \a map_fn on the data in each row.
    MapModel(std::shared_ptr<Model<SourceModelData>> source_model,
             std::function<MappedModelData(const SourceModelData &)> map_fn)
        : inner(std::make_shared<private_api::MapModelInner<SourceModelData, MappedModelData>>(
                  *this)),
          model(source_model),
          map_fn(map_fn)
    {
        model->attach_peer(inner);
    }

    size_t row_count() const override { return model->row_count(); }

    std::optional<MappedModelData> row_data(size_t i) const override
    {
        if (auto source_data = model->row_data(i))
            return map_fn(*source_data);
        else
            return {};
    }

    /// Returns the source model of this filter model.
    std::shared_ptr<Model<SourceModelData>> source_model() const { return model; }

    /// Re-applies the model's mapping function on each row of the source model. Use this if state
    /// external to the mapping function has changed.
    void reset() { inner->reset(); }

private:
    std::shared_ptr<private_api::MapModelInner<SourceModelData, MappedModelData>> inner;
    std::shared_ptr<slint::Model<SourceModelData>> model;
    std::function<MappedModelData(const SourceModelData &)> map_fn;
};

template<typename ModelData>
class SortModel;

namespace private_api {
template<typename ModelData>
struct SortModelInner : private_api::ModelChangeListener
{
    SortModelInner(std::shared_ptr<slint::Model<ModelData>> source_model,
                   std::function<bool(const ModelData &, const ModelData &)> comp,
                   slint::SortModel<ModelData> &target_model)
        : source_model(source_model), comp(comp), target_model(target_model)
    {
    }

    void row_added(size_t first_inserted_row, size_t count) override
    {
        if (sorted_rows_dirty) {
            reset();
            return;
        }

        // Adjust the existing sorted row indices to match the updated source model
        for (auto &row : sorted_rows) {
            if (row >= first_inserted_row)
                row += count;
        }

        for (size_t row = first_inserted_row; row < first_inserted_row + count; ++row) {

            ModelData inserted_value = *source_model->row_data(row);
            auto insertion_point =
                    std::lower_bound(sorted_rows.begin(), sorted_rows.end(), inserted_value,
                                     [this](size_t sorted_row, const ModelData &inserted_value) {
                                         auto sorted_elem = source_model->row_data(sorted_row);
                                         return comp(*sorted_elem, inserted_value);
                                     });

            insertion_point = sorted_rows.insert(insertion_point, row);
            target_model.row_added(std::distance(sorted_rows.begin(), insertion_point), 1);
        }
    }
    void row_changed(size_t changed_row) override
    {
        if (sorted_rows_dirty) {
            reset();
            return;
        }

        auto removed_row_it =
                sorted_rows.erase(std::find(sorted_rows.begin(), sorted_rows.end(), changed_row));
        auto removed_row = std::distance(sorted_rows.begin(), removed_row_it);

        ModelData changed_value = *source_model->row_data(changed_row);
        auto insertion_point =
                std::lower_bound(sorted_rows.begin(), sorted_rows.end(), changed_value,
                                 [this](size_t sorted_row, const ModelData &changed_value) {
                                     auto sorted_elem = source_model->row_data(sorted_row);
                                     return comp(*sorted_elem, changed_value);
                                 });

        insertion_point = sorted_rows.insert(insertion_point, changed_row);
        auto inserted_row = std::distance(sorted_rows.begin(), insertion_point);

        if (inserted_row == removed_row) {
            target_model.row_changed(removed_row);
        } else {
            target_model.row_removed(removed_row, 1);
            target_model.row_added(inserted_row, 1);
        }
    }
    void row_removed(size_t first_removed_row, size_t count) override
    {
        if (sorted_rows_dirty) {
            reset();
            return;
        }

        std::vector<size_t> removed_rows;
        removed_rows.reserve(count);

        for (auto it = sorted_rows.begin(); it != sorted_rows.end();) {
            if (*it >= first_removed_row) {
                if (*it < first_removed_row + count) {
                    removed_rows.push_back(std::distance(sorted_rows.begin(), it));
                    it = sorted_rows.erase(it);
                    continue;
                } else {
                    *it -= count;
                }
            }
            ++it;
        }

        for (auto removed_row : removed_rows) {
            target_model.row_removed(removed_row, 1);
        }
    }

    void reset() override
    {
        sorted_rows_dirty = true;
        target_model.Model<ModelData>::reset();
    }

    void ensure_sorted()
    {
        if (!sorted_rows_dirty) {
            return;
        }

        sorted_rows.resize(source_model->row_count());
        for (size_t i = 0; i < sorted_rows.size(); ++i)
            sorted_rows[i] = i;

        std::sort(sorted_rows.begin(), sorted_rows.end(), [this](auto lhs_index, auto rhs_index) {
            auto lhs_elem = source_model->row_data(lhs_index);
            auto rhs_elem = source_model->row_data(rhs_index);
            return comp(*lhs_elem, *rhs_elem);
        });

        sorted_rows_dirty = false;
    }

    std::shared_ptr<slint::Model<ModelData>> source_model;
    std::function<bool(const ModelData &, const ModelData &)> comp;
    slint::SortModel<ModelData> &target_model;
    std::vector<size_t> sorted_rows;
    bool sorted_rows_dirty = true;
};
}

/// The SortModel acts as an adapter model for a given source model by sorting all rows
/// with by order provided by the given sorting function. The sorting function is called for
/// pairs of elements of the source model.
template<typename ModelData>
class SortModel : public Model<ModelData>
{
    friend struct private_api::SortModelInner<ModelData>;

public:
    /// Constructs a new SortModel that provides a sorted view on the \a source_model by applying
    /// the order given by the specified \a comp.
    SortModel(std::shared_ptr<Model<ModelData>> source_model,
              std::function<bool(const ModelData &, const ModelData &)> comp)
        : inner(std::make_shared<private_api::SortModelInner<ModelData>>(std::move(source_model),
                                                                         std::move(comp), *this))
    {
        inner->source_model->attach_peer(inner);
    }

    size_t row_count() const override { return inner->source_model->row_count(); }

    std::optional<ModelData> row_data(size_t i) const override
    {
        inner->ensure_sorted();
        return inner->source_model->row_data(inner->sorted_rows[i]);
    }

    void set_row_data(size_t i, const ModelData &value) override
    {
        inner->source_model->set_row_data(inner->sorted_rows[i], value);
    }

    /// Re-applies the model's sort function on each row of the source model. Use this if state
    /// external to the sort function has changed.
    void reset() { inner->reset(); }

    /// Given the \a sorted_row_index, this function returns the corresponding row index in the
    /// source model.
    int unsorted_row(int sorted_row_index) const
    {
        inner->ensure_sorted();
        return inner->sorted_rows[sorted_row_index];
    }

    /// Returns the source model of this filter model.
    std::shared_ptr<Model<ModelData>> source_model() const { return inner->source_model; }

private:
    std::shared_ptr<private_api::SortModelInner<ModelData>> inner;
};

template<typename ModelData>
class ReverseModel;

namespace private_api {
template<typename ModelData>
struct ReverseModelInner : private_api::ModelChangeListener
{
    ReverseModelInner(std::shared_ptr<slint::Model<ModelData>> source_model,
                      slint::ReverseModel<ModelData> &target_model)
        : source_model(source_model), target_model(target_model)
    {
    }

    void row_added(size_t first_inserted_row, size_t count) override
    {
        auto row_count = source_model->row_count();
        auto old_row_count = row_count - count;
        auto row = old_row_count - first_inserted_row;

        target_model.row_added(row, count);
    }

    void row_changed(size_t changed_row) override
    {
        target_model.row_changed(source_model->row_count() - 1 - changed_row);
    }

    void row_removed(size_t first_removed_row, size_t count) override
    {
        target_model.row_removed(source_model->row_count() - first_removed_row, count);
    }

    void reset() override { target_model.reset(); }

    std::shared_ptr<slint::Model<ModelData>> source_model;
    slint::ReverseModel<ModelData> &target_model;
};
}

/// The ReverseModel acts as an adapter model for a given source model by reserving all rows.
/// This means that the first row in the source model is the last row of this model, the second
/// row is the second last, and so on.
template<typename ModelData>
class ReverseModel : public Model<ModelData>
{
    friend struct private_api::ReverseModelInner<ModelData>;

public:
    /// Constructs a new ReverseModel that provides a reversed view on the \a source_model.
    ReverseModel(std::shared_ptr<Model<ModelData>> source_model)
        : inner(std::make_shared<private_api::ReverseModelInner<ModelData>>(std::move(source_model),
                                                                            *this))
    {
        inner->source_model->attach_peer(inner);
    }

    size_t row_count() const override { return inner->source_model->row_count(); }

    std::optional<ModelData> row_data(size_t i) const override
    {
        auto count = inner->source_model->row_count();
        return inner->source_model->row_data(count - i - 1);
    }

    void set_row_data(size_t i, const ModelData &value) override
    {
        auto count = inner->source_model->row_count();
        inner->source_model->set_row_data(count - i - 1, value);
    }

    /// Returns the source model of this reserve model.
    std::shared_ptr<Model<ModelData>> source_model() const { return inner->source_model; }

private:
    std::shared_ptr<private_api::ReverseModelInner<ModelData>> inner;
};

namespace private_api {

template<typename C, typename ModelData>
class Repeater
{
    private_api::Property<std::shared_ptr<Model<ModelData>>> model;

    struct RepeaterInner : ModelChangeListener
    {
        enum class State { Clean, Dirty };
        struct RepeatedInstanceWithState
        {
            State state = State::Dirty;
            std::optional<ComponentHandle<C>> ptr;
        };
        std::vector<RepeatedInstanceWithState> data;
        private_api::Property<bool> is_dirty { true };
        std::shared_ptr<Model<ModelData>> model;

        void row_added(size_t index, size_t count) override
        {
            is_dirty.set(true);
            data.resize(data.size() + count);
            std::rotate(data.begin() + index, data.end() - count, data.end());
            for (std::size_t i = index; i < data.size(); ++i) {
                // all the indexes are dirty
                data[i].state = State::Dirty;
            }
        }
        void row_changed(size_t index) override
        {
            auto &c = data[index];
            if (model && c.ptr) {
                (*c.ptr)->update_data(index, *model->row_data(index));
                c.state = State::Clean;
            } else {
                c.state = State::Dirty;
            }
        }
        void row_removed(size_t index, size_t count) override
        {
            is_dirty.set(true);
            data.erase(data.begin() + index, data.begin() + index + count);
            for (std::size_t i = index; i < data.size(); ++i) {
                // all the indexes are dirty
                data[i].state = State::Dirty;
            }
        }
        void reset() override
        {
            is_dirty.set(true);
            data.clear();
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
                inner->model = m;
                m->attach_peer(inner);
            }
        }

        if (inner && inner->is_dirty.get()) {
            inner->is_dirty.set(false);
            if (auto m = model.get()) {
                auto count = m->row_count();
                inner->data.resize(count);
                for (size_t i = 0; i < count; ++i) {
                    auto &c = inner->data[i];
                    bool created = false;
                    if (!c.ptr) {
                        c.ptr = C::create(parent);
                        created = true;
                    }
                    if (c.state == RepeaterInner::State::Dirty) {
                        (*c.ptr)->update_data(i, *m->row_data(i));
                    }
                    if (created) {
                        (*c.ptr)->init();
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

    uint64_t visit(TraversalOrder order, private_api::ItemVisitorRefMut visitor) const
    {
        for (std::size_t i = 0; i < inner->data.size(); ++i) {
            auto index = order == TraversalOrder::BackToFront ? i : inner->data.size() - 1 - i;
            auto ref = item_at(index);
            if (ref.vtable->visit_children_item(ref, -1, order, visitor)
                != std::numeric_limits<uint64_t>::max()) {
                return index;
            }
        }
        return std::numeric_limits<uint64_t>::max();
    }

    vtable::VRef<private_api::ItemTreeVTable> item_at(int i) const
    {
        const auto &x = inner->data.at(i);
        return { &C::static_vtable, const_cast<C *>(&(**x.ptr)) };
    }

    vtable::VWeak<private_api::ItemTreeVTable> instance_at(std::size_t i) const
    {
        if (i >= inner->data.size()) {
            return {};
        }
        const auto &x = inner->data.at(i);
        return vtable::VWeak<private_api::ItemTreeVTable> { x.ptr->into_dyn() };
    }

    private_api::IndexRange index_range() const
    {
        return private_api::IndexRange { 0, inner->data.size() };
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

    void model_set_row_data(size_t row, const ModelData &data) const
    {
        if (model.is_dirty()) {
            std::abort();
        }
        if (auto m = model.get()) {
            if (row < m->row_count()) {
                m->set_row_data(row, data);
            }
        }
    }
};

inline SharedString translate(const SharedString &original, const SharedString &context,
                              const SharedString &domain,
                              cbindgen_private::Slice<SharedString> arguments, int n,
                              const SharedString &plural)
{
    SharedString result = original;
    cbindgen_private::slint_translate(&result, &context, &domain, arguments, n, &plural);
    return result;
}

inline SharedString translate_from_bundle(std::span<const char8_t *const> strs,
                                          cbindgen_private::Slice<SharedString> arguments)
{
    SharedString result;
    cbindgen_private::slint_translate_from_bundle(
            cbindgen_private::Slice<const char *>(
                    const_cast<char const **>(reinterpret_cast<char const *const *>(strs.data())),
                    strs.size()),
            arguments, &result);
    return result;
}
inline SharedString
translate_from_bundle_with_plural(std::span<const char8_t *const> strs,
                                  std::span<const uint32_t> indices,
                                  std::span<uintptr_t (*const)(int32_t)> plural_rules,
                                  cbindgen_private::Slice<SharedString> arguments, int n)
{
    SharedString result;
    cbindgen_private::Slice<const char *> strs_slice(
            const_cast<char const **>(reinterpret_cast<char const *const *>(strs.data())),
            strs.size());
    cbindgen_private::Slice<uint32_t> indices_slice(
            const_cast<uint32_t *>(reinterpret_cast<const uint32_t *>(indices.data())),
            indices.size());
    cbindgen_private::Slice<uintptr_t (*)(int32_t)> plural_rules_slice(
            const_cast<uintptr_t (**)(int32_t)>(
                    reinterpret_cast<uintptr_t (*const *)(int32_t)>(plural_rules.data())),
            plural_rules.size());
    cbindgen_private::slint_translate_from_bundle_with_plural(
            strs_slice, indices_slice, plural_rules_slice, arguments, n, &result);
    return result;
}

} // namespace private_api

#ifdef SLINT_FEATURE_GETTEXT
/// Forces all the strings that are translated with `@tr(...)` to be re-evaluated.
/// This is useful if the language is changed at runtime.
/// The function is only available when Slint is compiled with `SLINT_FEATURE_GETTEXT`.
///
/// Example
/// ```cpp
///     my_ui->global<LanguageSettings>().on_french_selected([] {
///        setenv("LANGUAGE", langs[l], true);
///        slint::update_all_translations();
///    });
/// ```
inline void update_all_translations()
{
    cbindgen_private::slint_translations_mark_dirty();
}
#endif

/// Select the current translation language when using bundled translations.
/// This function requires that the application's `.slint` file was compiled with bundled
/// translations. It must be called after creating the first component. Returns true if the language
/// was selected; false if the language was not found in the list of bundled translations.
inline bool select_bundled_translation(std::string_view language)
{
    return cbindgen_private::slint_translate_select_bundled_translation(
            slint::private_api::string_to_slice(language));
}

#if !defined(DOXYGEN)
cbindgen_private::Flickable::Flickable()
{
    slint_flickable_data_init(&data);
}
cbindgen_private::Flickable::~Flickable()
{
    slint_flickable_data_free(&data);
}

cbindgen_private::NativeStyleMetrics::NativeStyleMetrics(void *)
{
    slint_native_style_metrics_init(this);
}

cbindgen_private::NativeStyleMetrics::~NativeStyleMetrics()
{
    slint_native_style_metrics_deinit(this);
}

cbindgen_private::NativePalette::NativePalette(void *)
{
    slint_native_palette_init(this);
}

cbindgen_private::NativePalette::~NativePalette()
{
    slint_native_palette_deinit(this);
}
#endif // !defined(DOXYGEN)

namespace private_api {
// Was used in Slint <= 1.1.0 to have an error message in case of mismatch
template<int Major, int Minor, int Patch>
struct [[deprecated]] VersionCheckHelper
{
};
}

/// Enum for the event loop mode parameter of the slint::run_event_loop() function.
/// It is used to determine when the event loop quits.
enum class EventLoopMode {
    /// The event loop will quit when the last window is closed
    /// or when slint::quit_event_loop() is called.
    QuitOnLastWindowClosed,

    /// The event loop will keep running until slint::quit_event_loop() is called,
    /// even when all windows are closed.
    RunUntilQuit
};

/// Enters the main event loop. This is necessary in order to receive
/// events from the windowing system in order to render to the screen
/// and react to user input.
///
/// The mode parameter determines the behavior of the event loop when all windows are closed.
/// By default, it is set to QuitOnLastWindowClose, which means the event loop will
/// quit when the last window is closed.
inline void run_event_loop(EventLoopMode mode = EventLoopMode::QuitOnLastWindowClosed)
{
    private_api::assert_main_thread();
    cbindgen_private::slint_run_event_loop(mode == EventLoopMode::QuitOnLastWindowClosed);
}

/// Schedules the main event loop for termination. This function is meant
/// to be called from callbacks triggered by the UI. After calling the function,
/// it will return immediately and once control is passed back to the event loop,
/// the initial call to slint::run_event_loop() will return.
inline void quit_event_loop()
{
    cbindgen_private::slint_quit_event_loop();
}

/// Adds the specified functor to an internal queue, notifies the event loop to wake up.
/// Once woken up, any queued up functors will be invoked.
/// This function is thread-safe and can be called from any thread, including the one
/// running the event loop. The provided functors will only be invoked from the thread
/// that started the event loop.
///
/// You can use this to set properties or use any other Slint APIs from other threads,
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
///     slint::ComponentWeakHandle<NetworkStatusUI> weak_ui_handle(ui);
///     std::thread network_thread([=]{
///         std::string message = read_message_blocking_from_network();
///         slint::invoke_from_event_loop([&]() {
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
template<std::invocable Functor>
void invoke_from_event_loop(Functor f)
{
    cbindgen_private::slint_post_event(
            [](void *data) { (*reinterpret_cast<Functor *>(data))(); }, new Functor(std::move(f)),
            [](void *data) { delete reinterpret_cast<Functor *>(data); });
}

#if !defined(SLINT_FEATURE_FREESTANDING) || defined(DOXYGEN)

/// Blocking version of invoke_from_event_loop()
///
/// Just like invoke_from_event_loop(), this will run the specified functor from the thread running
/// the slint event loop. But it will block until the execution of the functor is finished,
/// and return that value.
///
/// This function must be called from a different thread than the thread that runs the event loop
/// otherwise it will result in a deadlock. Calling this function if the event loop is not running
/// will also block forever or until the event loop is started in another thread.
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
///             auto message = slint::blocking_invoke_from_event_loop([ui]() {
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
template<std::invocable Functor>
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

#    if !defined(DOXYGEN) // Doxygen doesn't see this as an overload of the previous one
// clang-format off
template<std::invocable Functor>
    requires(std::is_void_v<std::invoke_result_t<Functor>>)
void blocking_invoke_from_event_loop(Functor f)
// clang-format on
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
#    endif
#endif

/// Sets the application id for use on Wayland or X11 with
/// [xdg](https://specifications.freedesktop.org/desktop-entry-spec/latest/) compliant window
/// managers. This must be set before the window is shown.
inline void set_xdg_app_id(std::string_view xdg_app_id)
{
    private_api::assert_main_thread();
    SharedString s = xdg_app_id;
    cbindgen_private::slint_set_xdg_app_id(&s);
}

} // namespace slint
