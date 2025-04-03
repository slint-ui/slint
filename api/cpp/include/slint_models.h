// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "slint_item_tree.h"

#include <algorithm>
#include <functional>
#include <memory>
#include <optional>

namespace slint {

namespace private_api {

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
///
/// This is typically used in a `std::shared_ptr<slint::Model>`.
/// Model is an abstract class and you can derive from it to provide your own data model,
/// or use one of the provided models such as `slint::VectorModel`
///
/// An implementation of the Model can provide data to slint by re-implementing the `row_count` and
/// `row_data` functions. It is the responsibility of the Model implementation to call the
/// `Model::notify_row_changed()`, `Model::notify_row_added()`, `Model::notify_row_removed()`, or
/// `Model::notify_reset()` functions when the underlying data changes.
///
/// Note that the Model is not thread-safe. All Model operations need to be done in the main thread.
/// If you need to update the model data from another thread, use the
/// `slint::invoke_from_event_loop()` function to send the data to the main thread and update the
/// model.
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
    ///
    /// Your model implementation should call this function after the data of a row changes.
    void notify_row_changed(size_t row)
    {
        private_api::assert_main_thread();
        if (std::binary_search(tracked_rows.begin(), tracked_rows.end(), row)) {
            model_row_data_dirty_property.mark_dirty();
        }
        for_each_peers([=](auto peer) { peer->row_changed(row); });
    }
    /// Notify the views that rows were added
    ///
    /// Your model implementation should call this function after the row were added.
    void notify_row_added(size_t index, size_t count)
    {
        private_api::assert_main_thread();
        model_row_count_dirty_property.mark_dirty();
        tracked_rows.clear();
        model_row_data_dirty_property.mark_dirty();
        for_each_peers([=](auto peer) { peer->row_added(index, count); });
    }
    /// Notify the views that rows were removed
    ///
    /// Your model implementation should call this function after the row were removed.
    void notify_row_removed(size_t index, size_t count)
    {
        private_api::assert_main_thread();
        model_row_count_dirty_property.mark_dirty();
        tracked_rows.clear();
        model_row_data_dirty_property.mark_dirty();
        for_each_peers([=](auto peer) { peer->row_removed(index, count); });
    }

    /// Notify the views that the model has been changed and that everything needs to be reloaded
    ///
    /// Your model implementation should call this function after the model has been changed.
    void notify_reset()
    {
        private_api::assert_main_thread();
        model_row_count_dirty_property.mark_dirty();
        tracked_rows.clear();
        model_row_data_dirty_property.mark_dirty();
        for_each_peers([=](auto peer) { peer->reset(); });
    }

    /// \deprecated
    [[deprecated("Renamed to notify_row_changed")]] void row_changed(size_t row)
    {
        notify_row_changed(row);
    }
    /// \deprecated
    [[deprecated("Renamed to notify_row_added")]] void row_added(size_t index, size_t count)
    {
        notify_row_added(index, count);
    }
    /// \deprecated
    [[deprecated("Renamed to notify_row_removed")]] void row_removed(size_t index, size_t count)
    {
        notify_row_removed(index, count);
    }
    /// \deprecated
    [[deprecated("Renamed to notify_reset")]] void reset() { notify_reset(); }

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
            this->notify_row_changed(i);
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
            this->notify_row_changed(i);
        }
    }

    /// Append a new row with the given value
    void push_back(const ModelData &value)
    {
        data.push_back(value);
        this->notify_row_added(data.size() - 1, 1);
    }

    /// Remove the row at the given index from the model
    void erase(size_t index)
    {
        data.erase(data.begin() + index);
        this->notify_row_removed(index, 1);
    }

    /// Inserts the given value as a new row at the specified index
    void insert(size_t index, const ModelData &value)
    {
        data.insert(data.begin() + index, value);
        this->notify_row_added(index, 1);
    }

    /// Erases all rows from the VectorModel.
    void clear()
    {
        if (!data.empty()) {
            data.clear();
            this->notify_reset();
        }
    }

    /// Replaces the underlying VectorModel's vector with \a array.
    void set_vector(std::vector<ModelData> array)
    {
        data = std::move(array);
        this->notify_reset();
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

        target_model.notify_row_added(insertion_point - accepted_rows.begin(),
                                      added_accepted_rows.size());
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
            target_model.notify_row_changed(existing_row_index);
        } else if (!is_contained && accepted_updated_row) {
            accepted_rows.insert(existing_row, index);
            target_model.notify_row_added(existing_row_index, 1);
        } else if (is_contained && !accepted_updated_row) {
            accepted_rows.erase(existing_row);
            target_model.notify_row_removed(existing_row_index, 1);
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
            target_model.notify_row_removed(*mapped_removed_index, mapped_removed_len);
        }
    }
    void reset() override
    {
        filtered_rows_dirty = true;
        update_mapping();
        target_model.notify_reset();
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

    void row_added(size_t index, size_t count) override
    {
        target_model.notify_row_added(index, count);
    }
    void row_changed(size_t index) override { target_model.notify_row_changed(index); }
    void row_removed(size_t index, size_t count) override
    {
        target_model.notify_row_removed(index, count);
    }
    void reset() override { target_model.notify_reset(); }

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
///
/// \code
/// auto source_model = std::make_shared<slint::VectorModel<Person>>(...);
/// auto mapped_model = std::make_shared<slint::MapModel<Person, SharedString>>(
///     source_model, [](const Person &person) {
//          return fmt::format("{} {}", person.first, person.last);
//      });
/// \endcode
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
            target_model.notify_row_added(std::distance(sorted_rows.begin(), insertion_point), 1);
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
            target_model.notify_row_changed(removed_row);
        } else {
            target_model.notify_row_removed(removed_row, 1);
            target_model.notify_row_added(inserted_row, 1);
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
            target_model.notify_row_removed(removed_row, 1);
        }
    }

    void reset() override
    {
        sorted_rows_dirty = true;
        target_model.notify_reset();
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
///
/// \code
/// auto source_model = std::make_shared<slint::VectorModel<SharedString>>(
//      std::vector<SharedString> { "lorem", "ipsum", "dolor" });
/// auto sorted_model = std::make_shared<slint::SortModel<SharedString>>(
///     source_model, [](auto lhs, auto rhs) { return lhs < rhs; }));
/// \endcode

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

        target_model.notify_row_added(row, count);
    }

    void row_changed(size_t changed_row) override
    {
        target_model.notify_row_changed(source_model->row_count() - 1 - changed_row);
    }

    void row_removed(size_t first_removed_row, size_t count) override
    {
        target_model.notify_row_removed(source_model->row_count() - first_removed_row, count);
    }

    void reset() override { target_model.notify_reset(); }

    std::shared_ptr<slint::Model<ModelData>> source_model;
    slint::ReverseModel<ModelData> &target_model;
};
}

/// The ReverseModel acts as an adapter model for a given source model by reserving all rows.
/// This means that the first row in the source model is the last row of this model, the second
/// row is the second last, and so on.
///
/// \code
/// auto source_model = std::make_shared<slint::VectorModel<int>>(
//      std::vector<int> { 1, 2, 3, 4, 5 });
/// auto reversed_model = std::make_shared<slint::ReverseModel<int>>(source_model);
/// \endcode
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
            if (index > data.size()) {
                // Can happen before ensure_updated was called
                return;
            }
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
            if (index >= data.size()) {
                return;
            }
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
            if (index + count > data.size()) {
                // Can happen before ensure_updated was called
                return;
            }
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

    private_api::Property<std::shared_ptr<Model<ModelData>>> model;
    mutable std::shared_ptr<RepeaterInner> inner;

    vtable::VRef<private_api::ItemTreeVTable> item_at(int i) const
    {
        const auto &x = inner->data.at(i);
        return { &C::static_vtable, const_cast<C *>(&(**x.ptr)) };
    }

public:
    template<typename F>
    void set_model_binding(F &&binding) const
    {
        model.set_binding(std::forward<F>(binding));
    }

    template<typename Parent>
    void ensure_updated(const Parent *parent) const
    {
        if (model.is_dirty()) {
            auto old_model = model.get_internal();
            auto m = model.get();
            if (!inner || old_model != m) {
                inner = std::make_shared<RepeaterInner>();
                if (m) {
                    inner->model = m;
                    m->attach_peer(inner);
                }
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
                                 const private_api::Property<float> *viewport_y,
                                 float listview_width, [[maybe_unused]] float listview_height) const
    {
        // TODO: the rust code in model.rs try to only allocate as many items as visible items
        ensure_updated(parent);

        float h = compute_layout_listview(viewport_width, listview_width, viewport_y->get());
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

    std::size_t len() const { return inner ? inner->data.size() : 0; }

    float compute_layout_listview(const private_api::Property<float> *viewport_width,
                                  float listview_width, float viewport_y) const
    {
        float offset = viewport_y;
        auto vp_width = listview_width;
        if (!inner)
            return offset;
        for (auto &x : inner->data) {
            vp_width = std::max(vp_width, (*x.ptr)->listview_layout(&offset));
        }
        viewport_width->set(vp_width);
        return offset - viewport_y;
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

    void for_each(auto &&f) const
    {
        if (inner) {
            for (auto &&x : inner->data) {
                f(*x.ptr);
            }
        }
    }
};

template<typename C>
class Conditional
{
    private_api::Property<bool> model;
    mutable std::optional<ComponentHandle<C>> instance;

public:
    template<typename F>
    void set_model_binding(F &&binding) const
    {
        model.set_binding(std::forward<F>(binding));
    }

    template<typename Parent>
    void ensure_updated(const Parent *parent) const
    {
        if (!model.get()) {
            instance = std::nullopt;
        } else if (!instance) {
            instance = C::create(parent);
            (*instance)->init();
        }
    }

    uint64_t visit(TraversalOrder order, private_api::ItemVisitorRefMut visitor) const
    {
        if (instance) {
            vtable::VRef<private_api::ItemTreeVTable> ref { &C::static_vtable,
                                                            const_cast<C *>(&(**instance)) };
            if (ref.vtable->visit_children_item(ref, -1, order, visitor)
                != std::numeric_limits<uint64_t>::max()) {
                return 0;
            }
        }
        return std::numeric_limits<uint64_t>::max();
    }

    vtable::VWeak<private_api::ItemTreeVTable> instance_at(std::size_t i) const
    {
        if (i != 0 || !instance) {
            return {};
        }
        return vtable::VWeak<private_api::ItemTreeVTable> { instance->into_dyn() };
    }

    private_api::IndexRange index_range() const { return private_api::IndexRange { 0, len() }; }

    std::size_t len() const { return instance ? 1 : 0; }

    void for_each(auto &&f) const
    {
        if (instance) {
            f(*instance);
        }
    }
};

} // namespace private_api

} // namespace slint
