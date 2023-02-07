// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#include <chrono>
#define CATCH_CONFIG_MAIN
#include "catch2/catch.hpp"

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

SCENARIO("Mapped Model")
{
    auto vec_model = std::make_shared<slint::VectorModel<int>>(std::vector<int> { 1, 2, 3, 4 });

    auto plus_one_model = std::make_shared<slint::MapModel<int, int>>(
            vec_model, [](auto value) { return value + 1; });

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
