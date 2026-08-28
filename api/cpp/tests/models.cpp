// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#include <chrono>
#include <memory>
#define CATCH_CONFIG_MAIN
#include "catch2/catch_all.hpp"

#include <slint.h>

struct ModelObserver : public slint::private_api::ModelChangeListener
{
    void row_added(size_t index, size_t count) override
    {
        added_rows.push_back(Range { index, count });
    }
    void row_changed(size_t index) override { changed_rows.push_back(index); }
    void row_removed(size_t index, size_t count) override
    {
        removed_rows.push_back(Range { index, count });
    }
    void reset() override { model_reset = true; }

    void clear()
    {
        added_rows.clear();
        changed_rows.clear();
        removed_rows.clear();
        model_reset = false;
    }

    struct Range
    {
        size_t row_index;
        size_t count;

        bool operator==(const Range &) const = default;
    };
    std::vector<Range> added_rows;
    std::vector<size_t> changed_rows;
    std::vector<Range> removed_rows;
    bool model_reset = false;
};

std::ostream &operator<<(std::ostream &os, const ModelObserver::Range &value)
{
    os << "{ row_index: " << value.row_index << "; count: " << value.count << " }";
    return os;
}

SCENARIO("Filtering Model")
{
    auto vec_model =
            std::make_shared<slint::VectorModel<int>>(std::vector<int> { 1, 2, 3, 4, 5, 6 });

    auto even_rows = std::make_shared<slint::FilterModel<int>>(
            vec_model, [](auto value) { return value % 2 == 0; });

    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
    REQUIRE(even_rows->row_data(2) == 6);
}

SCENARIO("Filtering Insert")
{
    auto vec_model =
            std::make_shared<slint::VectorModel<int>>(std::vector<int> { 1, 2, 3, 4, 5, 6 });

    auto even_rows = std::make_shared<slint::FilterModel<int>>(
            vec_model, [](auto value) { return value % 2 == 0; });

    auto observer = std::make_shared<ModelObserver>();
    even_rows->attach_peer(observer);

    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
    REQUIRE(even_rows->row_data(2) == 6);

    vec_model->insert(2, 10);

    REQUIRE(observer->added_rows.size() == 1);
    REQUIRE(observer->added_rows[0] == ModelObserver::Range { 1, 1 });
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(even_rows->row_count() == 4);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 10);
    REQUIRE(even_rows->row_data(2) == 4);
    REQUIRE(even_rows->row_data(3) == 6);

    // insert odd number -> no change
    vec_model->insert(0, 1);

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(!observer->model_reset);
    observer->clear();
}

SCENARIO("Filtering Change")
{
    auto vec_model =
            std::make_shared<slint::VectorModel<int>>(std::vector<int> { 1, 2, 3, 4, 5, 6 });

    auto even_rows = std::make_shared<slint::FilterModel<int>>(
            vec_model, [](auto value) { return value % 2 == 0; });

    auto observer = std::make_shared<ModelObserver>();
    even_rows->attach_peer(observer);

    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
    REQUIRE(even_rows->row_data(2) == 6);

    // change leading odd 1 to odd 3 -> no change
    vec_model->set_row_data(0, 3);

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(!observer->model_reset);

    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
    REQUIRE(even_rows->row_data(2) == 6);

    // change trailing 6 to odd 1 -> one row less
    vec_model->set_row_data(5, 1);

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.size() == 1);
    REQUIRE(observer->removed_rows[0] == ModelObserver::Range { 2, 1 });
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(even_rows->row_count() == 2);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);

    // change leading odd 3 to even 0 -> one new row
    vec_model->set_row_data(0, 0);

    REQUIRE(observer->added_rows.size() == 1);
    REQUIRE(observer->added_rows[0] == ModelObserver::Range { 0, 1 });
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 0);
    REQUIRE(even_rows->row_data(1) == 2);
    REQUIRE(even_rows->row_data(2) == 4);

    // change trailing filtered 4 to even 0 -> one changed row
    vec_model->set_row_data(3, 0);

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.size() == 1);
    REQUIRE(observer->changed_rows[0] == 2);
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 0);
    REQUIRE(even_rows->row_data(1) == 2);
    REQUIRE(even_rows->row_data(2) == 0);
}

SCENARIO("Filtering Model Remove")
{
    auto vec_model =
            std::make_shared<slint::VectorModel<int>>(std::vector<int> { 1, 2, 3, 4, 5, 6 });

    auto even_rows = std::make_shared<slint::FilterModel<int>>(
            vec_model, [](auto value) { return value % 2 == 0; });

    auto observer = std::make_shared<ModelObserver>();
    even_rows->attach_peer(observer);

    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
    REQUIRE(even_rows->row_data(2) == 6);

    // Erase unrelated row
    vec_model->erase(0);

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
    REQUIRE(even_rows->row_data(2) == 6);

    // Erase trailing even 6
    vec_model->erase(4);

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.size() == 1);
    REQUIRE(observer->removed_rows[0] == ModelObserver::Range { 2, 1 });
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(even_rows->row_count() == 2);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
}

SCENARIO("Filtering Model Reset")
{
    auto vec_model =
            std::make_shared<slint::VectorModel<int>>(std::vector<int> { 1, 2, 3, 4, 5, 6 });

    bool even = true;

    auto even_rows = std::make_shared<slint::FilterModel<int>>(
            vec_model, [&even](auto value) { return value % 2 == !even; });

    auto observer = std::make_shared<ModelObserver>();
    even_rows->attach_peer(observer);

    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
    REQUIRE(even_rows->row_data(2) == 6);

    even = false;
    even_rows->reset();

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(observer->model_reset);
    observer->clear();

    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 1);
    REQUIRE(even_rows->row_data(1) == 3);
    REQUIRE(even_rows->row_data(2) == 5);
}

template<typename ModelData>
class TestDeferredFilterModel : public slint::FilterModel<ModelData>
{
public:
    TestDeferredFilterModel(bool &initialized, bool &filtered,
                            std::shared_ptr<slint::Model<ModelData>> source_model)
        : slint::FilterModel<ModelData> { std::move(source_model),
                                          [&filtered]([[maybe_unused]] const ModelData &) {
                                              if (!filtered) {
                                                  filtered = true;
                                              }
                                              return true;
                                          } }
    {
        initialized = true;
    }
};

SCENARIO("Filtering Model Ensure Deferred")
{
    auto source_model =
            std::make_shared<slint::VectorModel<int>>(std::vector<int> { 0, 1, 2, 3, 4 });

    bool initialized = false;
    bool filtered = false;

    auto filter_model =
            std::make_shared<TestDeferredFilterModel<int>>(initialized, filtered, source_model);
    REQUIRE(initialized);
    REQUIRE_FALSE(filtered);

    filter_model->row_data(0);
    REQUIRE(filtered);
}

SCENARIO("Mapped Model")
{
    auto vec_model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 1, 2, 3, 4 });

    int to_add = 1;

    auto plus_one_model = std::make_shared<slint::MapModel<int, int>>(
            vec_model, [&to_add](auto value) { return value + to_add; });

    auto observer = std::make_shared<ModelObserver>();
    plus_one_model->attach_peer(observer);

    REQUIRE(plus_one_model->row_count() == 4);
    REQUIRE(plus_one_model->row_data(0) == 2);
    REQUIRE(plus_one_model->row_data(1) == 3);
    REQUIRE(plus_one_model->row_data(2) == 4);
    REQUIRE(plus_one_model->row_data(3) == 5);

    vec_model->insert(0, 100);

    REQUIRE(observer->added_rows.size() == 1);
    REQUIRE(observer->added_rows[0] == ModelObserver::Range { 0, 1 });
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(plus_one_model->row_count() == 5);
    REQUIRE(plus_one_model->row_data(0) == 101);
    REQUIRE(plus_one_model->row_data(1) == 2);
    REQUIRE(plus_one_model->row_data(2) == 3);
    REQUIRE(plus_one_model->row_data(3) == 4);
    REQUIRE(plus_one_model->row_data(4) == 5);

    vec_model->set_row_data(1, 3);

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.size() == 1);
    REQUIRE(observer->changed_rows[0] == 1);
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(plus_one_model->row_count() == 5);
    REQUIRE(plus_one_model->row_data(0) == 101);
    REQUIRE(plus_one_model->row_data(1) == 4);
    REQUIRE(plus_one_model->row_data(2) == 3);
    REQUIRE(plus_one_model->row_data(3) == 4);
    REQUIRE(plus_one_model->row_data(4) == 5);

    vec_model->erase(3);

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.size() == 1);
    REQUIRE(observer->removed_rows[0] == ModelObserver::Range { 3, 1 });
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(plus_one_model->row_count() == 4);
    REQUIRE(plus_one_model->row_data(0) == 101);
    REQUIRE(plus_one_model->row_data(1) == 4);
    REQUIRE(plus_one_model->row_data(2) == 3);
    REQUIRE(plus_one_model->row_data(3) == 5);

    to_add = 51;
    plus_one_model->reset();

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(observer->model_reset);
    observer->clear();

    REQUIRE(plus_one_model->row_count() == 4);
    REQUIRE(plus_one_model->row_data(0) == 151);
    REQUIRE(plus_one_model->row_data(1) == 54);
    REQUIRE(plus_one_model->row_data(2) == 53);
    REQUIRE(plus_one_model->row_data(3) == 55);
}

SCENARIO("Sorted Model Insert")
{
    auto vec_model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 3, 4, 1, 2 });

    auto sorted_model = std::make_shared<slint::SortModel<int>>(
            vec_model, [](auto lhs, auto rhs) { return lhs < rhs; });

    auto observer = std::make_shared<ModelObserver>();
    sorted_model->attach_peer(observer);

    REQUIRE(sorted_model->row_count() == 4);
    REQUIRE(sorted_model->row_data(0) == 1);
    REQUIRE(sorted_model->row_data(1) == 2);
    REQUIRE(sorted_model->row_data(2) == 3);
    REQUIRE(sorted_model->row_data(3) == 4);

    vec_model->insert(0, 10);

    REQUIRE(observer->added_rows.size() == 1);
    REQUIRE(observer->added_rows[0] == ModelObserver::Range { 4, 1 });
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(sorted_model->row_count() == 5);
    REQUIRE(sorted_model->row_data(0) == 1);
    REQUIRE(sorted_model->row_data(1) == 2);
    REQUIRE(sorted_model->row_data(2) == 3);
    REQUIRE(sorted_model->row_data(3) == 4);
    REQUIRE(sorted_model->row_data(4) == 10);
}

SCENARIO("Sorted Model Remove")
{
    auto vec_model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 3, 4, 1, 2 });

    auto sorted_model = std::make_shared<slint::SortModel<int>>(
            vec_model, [](auto lhs, auto rhs) { return lhs < rhs; });

    auto observer = std::make_shared<ModelObserver>();
    sorted_model->attach_peer(observer);

    REQUIRE(sorted_model->row_count() == 4);
    REQUIRE(sorted_model->row_data(0) == 1);
    REQUIRE(sorted_model->row_data(1) == 2);
    REQUIRE(sorted_model->row_data(2) == 3);
    REQUIRE(sorted_model->row_data(3) == 4);

    /// Remove the entry with the value 4
    vec_model->erase(1);

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.size() == 1);
    REQUIRE(observer->removed_rows[0] == ModelObserver::Range { 3, 1 });
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(sorted_model->row_count() == 3);
    REQUIRE(sorted_model->row_data(0) == 1);
    REQUIRE(sorted_model->row_data(1) == 2);
    REQUIRE(sorted_model->row_data(2) == 3);
}

class BatchRemoveModel : public slint::Model<int>
{
public:
    BatchRemoveModel(std::vector<int> array) : data(std::move(array)) { }
    size_t row_count() const override { return data.size(); }
    std::optional<int> row_data(size_t i) const override
    {
        if (i >= data.size())
            return {};
        return data[i];
    }
    void erase(size_t index, size_t count)
    {
        data.erase(data.begin() + index, data.begin() + index + count);
        this->notify_row_removed(index, count);
    }

private:
    std::vector<int> data;
};

SCENARIO("Sorted Model Insert Adjustment")
{
    // Insertions before the end shift the mapping entries above them; appends skip that.
    for (size_t insert_at : { size_t(0), size_t(3), size_t(7), size_t(10) }) {
        auto origin = std::make_shared<slint::VectorModel<int>>(
                std::vector<int> { 50, 10, 40, 20, 30, 90, 60, 80, 70, 0 });
        auto sorted = std::make_shared<slint::SortModel<int>>(
                origin, [](auto lhs, auto rhs) { return lhs < rhs; });
        REQUIRE(sorted->row_count() == 10);
        origin->insert(insert_at, 35);
        origin->insert(insert_at, 45);
        std::vector<int> result;
        for (size_t i = 0; i < sorted->row_count(); ++i)
            result.push_back(*sorted->row_data(i));
        REQUIRE(result == std::vector<int> { 0, 10, 20, 30, 35, 40, 45, 50, 60, 70, 80, 90 });
    }

    auto origin =
            std::make_shared<slint::VectorModel<int>>(std::vector<int> { 50, 10, 40, 20, 30 });
    auto sorted = std::make_shared<slint::SortModel<int>>(
            origin, [](auto lhs, auto rhs) { return lhs < rhs; });
    REQUIRE(sorted->row_count() == 5);
    origin->push_back(35);
    origin->push_back(5);
    origin->push_back(100);
    std::vector<int> result;
    for (size_t i = 0; i < sorted->row_count(); ++i)
        result.push_back(*sorted->row_data(i));
    REQUIRE(result == std::vector<int> { 5, 10, 20, 30, 35, 40, 50, 100 });
}

SCENARIO("Sorted Model Batch Remove")
{
    auto source_model = std::make_shared<BatchRemoveModel>(std::vector<int> { 3, 4, 1, 2 });

    auto sorted_model = std::make_shared<slint::SortModel<int>>(
            source_model, [](auto lhs, auto rhs) { return lhs < rhs; });

    auto observer = std::make_shared<ModelObserver>();
    sorted_model->attach_peer(observer);

    REQUIRE(sorted_model->row_count() == 4);
    REQUIRE(sorted_model->row_data(0) == 1);
    REQUIRE(sorted_model->row_data(1) == 2);
    REQUIRE(sorted_model->row_data(2) == 3);
    REQUIRE(sorted_model->row_data(3) == 4);

    /// Remove the entries with the values 4 and 1 in one notification
    source_model->erase(1, 2);

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.size() == 2);
    REQUIRE(observer->removed_rows[0] == ModelObserver::Range { 0, 1 });
    REQUIRE(observer->removed_rows[1] == ModelObserver::Range { 2, 1 });
    REQUIRE(!observer->model_reset);

    REQUIRE(sorted_model->row_count() == 2);
    REQUIRE(sorted_model->row_data(0) == 2);
    REQUIRE(sorted_model->row_data(1) == 3);
}

SCENARIO("Sorted Model Change")
{
    auto vec_model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 3, 4, 1, 2 });

    auto sorted_model = std::make_shared<slint::SortModel<int>>(
            vec_model, [](auto lhs, auto rhs) { return lhs < rhs; });

    auto observer = std::make_shared<ModelObserver>();
    sorted_model->attach_peer(observer);

    REQUIRE(sorted_model->row_count() == 4);
    REQUIRE(sorted_model->row_data(0) == 1);
    REQUIRE(sorted_model->row_data(1) == 2);
    REQUIRE(sorted_model->row_data(2) == 3);
    REQUIRE(sorted_model->row_data(3) == 4);

    /// Change the entry with the value 4 to 10 -> maintain order
    vec_model->set_row_data(1, 10);

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.size() == 1);
    REQUIRE(observer->changed_rows[0] == 3);
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(sorted_model->row_count() == 4);
    REQUIRE(sorted_model->row_data(0) == 1);
    REQUIRE(sorted_model->row_data(1) == 2);
    REQUIRE(sorted_model->row_data(2) == 3);
    REQUIRE(sorted_model->row_data(3) == 10);

    /// Change the entry with the value 10 to 0 -> new order with remove and insert
    vec_model->set_row_data(1, 0);

    REQUIRE(observer->added_rows.size() == 1);
    REQUIRE(observer->added_rows[0] == ModelObserver::Range { 0, 1 });
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.size() == 1);
    REQUIRE(observer->removed_rows[0] == ModelObserver::Range { 3, 1 });
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(sorted_model->row_count() == 4);
    REQUIRE(sorted_model->row_data(0) == 0);
    REQUIRE(sorted_model->row_data(1) == 1);
    REQUIRE(sorted_model->row_data(2) == 2);
    REQUIRE(sorted_model->row_data(3) == 3);
}

SCENARIO("Sorted Model Reset")
{
    auto vec_model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 3, 4, 1, 2 });

    bool ascending = true;

    auto sorted_model =
            std::make_shared<slint::SortModel<int>>(vec_model, [&ascending](auto lhs, auto rhs) {
                return ascending ? lhs < rhs : rhs < lhs;
            });

    auto observer = std::make_shared<ModelObserver>();
    sorted_model->attach_peer(observer);

    REQUIRE(sorted_model->row_count() == 4);
    REQUIRE(sorted_model->row_data(0) == 1);
    REQUIRE(sorted_model->row_data(1) == 2);
    REQUIRE(sorted_model->row_data(2) == 3);
    REQUIRE(sorted_model->row_data(3) == 4);

    ascending = false;
    sorted_model->reset();

    REQUIRE(sorted_model->row_count() == 4);
    REQUIRE(sorted_model->row_data(0) == 4);
    REQUIRE(sorted_model->row_data(1) == 3);
    REQUIRE(sorted_model->row_data(2) == 2);
    REQUIRE(sorted_model->row_data(3) == 1);

    REQUIRE(observer->model_reset);
}

template<typename ModelData>
class TestDeferredSortModel : public slint::SortModel<ModelData>
{
public:
    TestDeferredSortModel(bool &initialized, bool &sorted,
                          std::shared_ptr<slint::Model<ModelData>> source_model)
        : slint::SortModel<ModelData> { std::move(source_model),
                                        [&sorted](const ModelData &first, const ModelData &second) {
                                            if (!sorted) {
                                                sorted = true;
                                            }
                                            return first > second;
                                        } }
    {
        initialized = true;
    }
};

SCENARIO("Sorted Model Ensure Deferred")
{
    auto source_model =
            std::make_shared<slint::VectorModel<int>>(std::vector<int> { 0, 1, 2, 3, 4 });

    bool initialized = false;
    bool sorted = false;

    auto sort_model =
            std::make_shared<TestDeferredSortModel<int>>(initialized, sorted, source_model);
    REQUIRE(initialized);
    REQUIRE_FALSE(sorted);

    sort_model->row_data(0);
    REQUIRE(sorted);
}

SCENARIO("Reverse Model Insert")
{
    auto vec_model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 3, 4, 1, 2 });

    auto reverse_model = std::make_shared<slint::ReverseModel<int>>(vec_model);

    auto observer = std::make_shared<ModelObserver>();
    reverse_model->attach_peer(observer);

    REQUIRE(reverse_model->row_count() == 4);
    REQUIRE(reverse_model->row_data(0) == 2);
    REQUIRE(reverse_model->row_data(1) == 1);
    REQUIRE(reverse_model->row_data(2) == 4);
    REQUIRE(reverse_model->row_data(3) == 3);

    vec_model->insert(0, 10);

    REQUIRE(observer->added_rows.size() == 1);
    REQUIRE(observer->added_rows[0] == ModelObserver::Range { 4, 1 });
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(reverse_model->row_count() == 5);
    REQUIRE(reverse_model->row_data(0) == 2);
    REQUIRE(reverse_model->row_data(1) == 1);
    REQUIRE(reverse_model->row_data(2) == 4);
    REQUIRE(reverse_model->row_data(3) == 3);
    REQUIRE(reverse_model->row_data(4) == 10);
}

SCENARIO("Reverse Model Remove")
{
    auto vec_model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 3, 4, 1, 2 });

    auto reverse_model = std::make_shared<slint::ReverseModel<int>>(vec_model);

    auto observer = std::make_shared<ModelObserver>();
    reverse_model->attach_peer(observer);

    REQUIRE(reverse_model->row_count() == 4);
    REQUIRE(reverse_model->row_data(0) == 2);
    REQUIRE(reverse_model->row_data(1) == 1);
    REQUIRE(reverse_model->row_data(2) == 4);
    REQUIRE(reverse_model->row_data(3) == 3);

    /// Remove the entry with the value 4
    vec_model->erase(1);

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.size() == 1);
    REQUIRE(observer->removed_rows[0] == ModelObserver::Range { 2, 1 });
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(reverse_model->row_count() == 3);
    REQUIRE(reverse_model->row_data(0) == 2);
    REQUIRE(reverse_model->row_data(1) == 1);
    REQUIRE(reverse_model->row_data(2) == 3);
}

SCENARIO("Reverse Model Change")
{
    auto vec_model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 3, 4, 1, 2 });

    auto reverse_model = std::make_shared<slint::ReverseModel<int>>(vec_model);

    auto observer = std::make_shared<ModelObserver>();
    reverse_model->attach_peer(observer);

    REQUIRE(reverse_model->row_count() == 4);
    REQUIRE(reverse_model->row_data(0) == 2);
    REQUIRE(reverse_model->row_data(1) == 1);
    REQUIRE(reverse_model->row_data(2) == 4);
    REQUIRE(reverse_model->row_data(3) == 3);

    /// Change the entry with the value 4 to 10 -> maintain order
    vec_model->set_row_data(1, 10);

    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.size() == 1);
    REQUIRE(observer->changed_rows[0] == 2);
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(!observer->model_reset);
    observer->clear();

    REQUIRE(reverse_model->row_count() == 4);
    REQUIRE(reverse_model->row_data(0) == 2);
    REQUIRE(reverse_model->row_data(1) == 1);
    REQUIRE(reverse_model->row_data(2) == 10);
    REQUIRE(reverse_model->row_data(3) == 3);

    vec_model->clear();
    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(observer->model_reset);
    observer->clear();

    REQUIRE(reverse_model->row_count() == 0);
}

TEST_CASE("VectorModel clear and replace")
{
    using namespace slint::private_api;

    auto model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 0, 1, 2, 3, 4 });

    auto observer = std::make_shared<ModelObserver>();
    model->attach_peer(observer);

    REQUIRE(model->row_count() == 5);
    model->clear();
    REQUIRE(model->row_count() == 0);
    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(observer->model_reset);
    observer->clear();

    model->clear();
    REQUIRE(!observer->model_reset);
    observer->clear();

    model->set_vector({ 2, 3, 4 });
    REQUIRE(model->row_count() == 3);
    REQUIRE(model->row_data(1) == 3);
    REQUIRE(observer->added_rows.empty());
    REQUIRE(observer->changed_rows.empty());
    REQUIRE(observer->removed_rows.empty());
    REQUIRE(observer->model_reset);

    // Test that taking a vector by value compiles
    std::vector<int> new_data { 5, 6, 7, 8 };
    model->set_vector(new_data);
}

TEST_CASE("Model any-change tracking")
{
    using namespace slint::private_api;

    auto model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 0, 1, 2, 3, 4 });

    PropertyTracker tracker;
    auto find_two = [&] {
        return tracker.evaluate(
                [&] { return model_find_index(model, [](int x) { return x == 2; }); });
    };

    REQUIRE(find_two() == 2);
    REQUIRE(!tracker.is_dirty());

    // Any row change dirties the binding, including rows past the match,
    // as track_any_change() tracks all rows regardless of short-circuiting.
    model->set_row_data(4, 42);
    REQUIRE(tracker.is_dirty());
    REQUIRE(find_two() == 2);
    REQUIRE(!tracker.is_dirty());

    model->set_row_data(2, 22);
    REQUIRE(tracker.is_dirty());
    REQUIRE(find_two() == -1);
    REQUIRE(!tracker.is_dirty());

    model->push_back(2);
    REQUIRE(tracker.is_dirty());
    REQUIRE(find_two() == 5);
    REQUIRE(!tracker.is_dirty());

    model->erase(0);
    REQUIRE(tracker.is_dirty());
    REQUIRE(find_two() == 4);
    REQUIRE(!tracker.is_dirty());

    // A row change right after add/remove cleared the tracking state still
    // dirties the binding, because the re-evaluation re-tracked the model.
    model->set_row_data(0, 7);
    REQUIRE(tracker.is_dirty());
    REQUIRE(find_two() == 4);
    REQUIRE(!tracker.is_dirty());

    model->set_vector({});
    REQUIRE(tracker.is_dirty());
    REQUIRE(find_two() == -1);
    REQUIRE(!tracker.is_dirty());
}

TEST_CASE("Model any-change tracking for model_all and model_any")
{
    using namespace slint::private_api;

    auto model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 0, 1, 2, 3, 4 });

    PropertyTracker any_tracker;
    auto has_two = [&] {
        return any_tracker.evaluate([&] { return model_any(model, [](int x) { return x == 2; }); });
    };
    PropertyTracker all_tracker;
    auto none_is_two = [&] {
        return all_tracker.evaluate([&] { return model_all(model, [](int x) { return x != 2; }); });
    };

    REQUIRE(has_two());
    REQUIRE(!any_tracker.is_dirty());
    REQUIRE(!none_is_two());
    REQUIRE(!all_tracker.is_dirty());

    // Any row change dirties both bindings, including rows past the match,
    // as track_any_change() tracks all rows regardless of short-circuiting.
    model->set_row_data(4, 42);
    REQUIRE(any_tracker.is_dirty());
    REQUIRE(all_tracker.is_dirty());
    REQUIRE(has_two());
    REQUIRE(!none_is_two());
    REQUIRE(!any_tracker.is_dirty());
    REQUIRE(!all_tracker.is_dirty());

    model->set_row_data(2, 22);
    REQUIRE(any_tracker.is_dirty());
    REQUIRE(all_tracker.is_dirty());
    REQUIRE(!has_two());
    REQUIRE(none_is_two());
    REQUIRE(!any_tracker.is_dirty());
    REQUIRE(!all_tracker.is_dirty());

    model->push_back(2);
    REQUIRE(any_tracker.is_dirty());
    REQUIRE(all_tracker.is_dirty());
    REQUIRE(has_two());
    REQUIRE(!none_is_two());
    REQUIRE(!any_tracker.is_dirty());
    REQUIRE(!all_tracker.is_dirty());

    model->erase(0);
    REQUIRE(any_tracker.is_dirty());
    REQUIRE(all_tracker.is_dirty());
    REQUIRE(has_two());
    REQUIRE(!none_is_two());
    REQUIRE(!any_tracker.is_dirty());
    REQUIRE(!all_tracker.is_dirty());

    // A row change right after add/remove cleared the tracking state still
    // dirties both bindings, because the re-evaluation re-tracked the model.
    model->set_row_data(0, 7);
    REQUIRE(any_tracker.is_dirty());
    REQUIRE(all_tracker.is_dirty());
    REQUIRE(has_two());
    REQUIRE(!none_is_two());
    REQUIRE(!any_tracker.is_dirty());
    REQUIRE(!all_tracker.is_dirty());

    // model_all() is vacuously true over an empty model, unlike model_any().
    model->set_vector({});
    REQUIRE(any_tracker.is_dirty());
    REQUIRE(all_tracker.is_dirty());
    REQUIRE(!has_two());
    REQUIRE(none_is_two());
    REQUIRE(!any_tracker.is_dirty());
    REQUIRE(!all_tracker.is_dirty());
}

// A model whose middle row has no data, to pin down how the array predicates treat a
// row that is in range but unreadable.
struct AbsentRowModel : slint::Model<int>
{
    size_t row_count() const override { return 3; }
    std::optional<int> row_data(size_t row) const override
    {
        switch (row) {
        case 0:
            return 1;
        case 1:
            return std::nullopt;
        default:
            return 3;
        }
    }
};

TEST_CASE("Array predicates skip absent rows")
{
    using namespace slint::private_api;

    auto model = std::make_shared<AbsentRowModel>();

    // All three predicates skip the absent row, rather than failing the whole model
    // or feeding the predicate a default value in its place.
    REQUIRE(model_all(model, [](int x) { return x > 0; }));
    REQUIRE(!model_any(model, [](int x) { return x == 0; }));
    REQUIRE(model_find_index(model, [](int x) { return x == 3; }) == 2);
}

TEST_CASE("Model any-change tracking outside a binding")
{
    using namespace slint::private_api;

    auto model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 0, 1, 2, 3, 4 });

    PropertyTracker tracker;
    REQUIRE(tracker.evaluate([&]() {
        model->track_row_data_changes(1);
        return model->row_data(1);
    }) == 1);
    REQUIRE(!tracker.is_dirty());

    // Evaluating a predicate while no binding is being evaluated — as generated code does
    // from a callback or function body — registers no dependency, so it must not latch
    // whole-model tracking and start dirtying bindings that only asked for one row.
    REQUIRE(model_find_index(model, [](int x) { return x == 2; }) == 2);
    model->set_row_data(0, 100);
    REQUIRE(!tracker.is_dirty());

    // The row the tracker did ask for still works.
    model->set_row_data(1, 100);
    REQUIRE(tracker.is_dirty());
}

TEST_CASE("Model any-change tracking subsumes row tracking")
{
    using namespace slint::private_api;

    auto model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 0, 1, 2, 3, 4 });

    // Track the whole model, so that every row is now implicitly tracked.
    PropertyTracker any_tracker;
    REQUIRE(any_tracker.evaluate([&] {
        return model_find_index(model, [](int x) { return x == 2; });
    }) == 2);
    REQUIRE(!any_tracker.is_dirty());

    // track_row_data_changes() no longer records the row individually while that
    // is the case, but a binding tracking a single row must still be notified.
    PropertyTracker row_tracker;
    REQUIRE(row_tracker.evaluate([&] {
        model->track_row_data_changes(0);
        return model->row_data(0);
    }) == 0);
    REQUIRE(!row_tracker.is_dirty());

    model->set_row_data(0, 9);
    REQUIRE(row_tracker.is_dirty());
    REQUIRE(any_tracker.is_dirty());
}
