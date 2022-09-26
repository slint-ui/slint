// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#include <chrono>
#define CATCH_CONFIG_MAIN
#include "catch2/catch.hpp"

#include <slint.h>

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

    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
    REQUIRE(even_rows->row_data(2) == 6);

    vec_model->insert(2, 10);
    REQUIRE(even_rows->row_count() == 4);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 10);
    REQUIRE(even_rows->row_data(2) == 4);
    REQUIRE(even_rows->row_data(3) == 6);
}

SCENARIO("Filtering Change")
{
    auto vec_model =
            std::make_shared<slint::VectorModel<int>>(std::vector<int> { 1, 2, 3, 4, 5, 6 });

    auto even_rows = std::make_shared<slint::FilterModel<int>>(
            vec_model, [](auto value) { return value % 2 == 0; });

    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
    REQUIRE(even_rows->row_data(2) == 6);

    // change leading odd 1 to odd 3 -> no change
    vec_model->set_row_data(0, 3);
    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
    REQUIRE(even_rows->row_data(2) == 6);

    // change trailing 6 to odd 1 -> one row less
    vec_model->set_row_data(5, 1);
    REQUIRE(even_rows->row_count() == 2);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);

    // change leading odd 3 to even 0 -> one new row
    vec_model->set_row_data(0, 0);
    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 0);
    REQUIRE(even_rows->row_data(1) == 2);
    REQUIRE(even_rows->row_data(2) == 4);
}

SCENARIO("Filtering Model Remove")
{
    auto vec_model =
            std::make_shared<slint::VectorModel<int>>(std::vector<int> { 1, 2, 3, 4, 5, 6 });

    auto even_rows = std::make_shared<slint::FilterModel<int>>(
            vec_model, [](auto value) { return value % 2 == 0; });

    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
    REQUIRE(even_rows->row_data(2) == 6);

    // Erase unrelated row
    vec_model->erase(0);
    REQUIRE(even_rows->row_count() == 3);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
    REQUIRE(even_rows->row_data(2) == 6);

    // Erase trailing even 6
    vec_model->erase(4);
    REQUIRE(even_rows->row_count() == 2);
    REQUIRE(even_rows->row_data(0) == 2);
    REQUIRE(even_rows->row_data(1) == 4);
}
