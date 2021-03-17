/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#pragma once

#include "sixtyfps.h"

#include "sixtyfps_interpreter_internal.h"

#include <optional>

namespace sixtyfps::interpreter {

class Value
{
public:
    Value() { cbindgen_private::sixtyfps_interpreter_value_new(&inner); }

    Value(const Value &other) { sixtyfps_interpreter_value_clone(&other.inner, &inner); }
    Value(Value &&other)
    {
        inner = other.inner;
        cbindgen_private::sixtyfps_interpreter_value_new(&other.inner);
    }
    Value &operator=(Value &&other)
    {
        if (this == &other)
            return *this;
        cbindgen_private::sixtyfps_interpreter_value_destructor(&inner);
        inner = other.inner;
        cbindgen_private::sixtyfps_interpreter_value_new(&other.inner);
        return *this;
    }
    Value &operator=(const Value &other)
    {
        if (this == &other)
            return *this;
        cbindgen_private::sixtyfps_interpreter_value_destructor(&inner);
        sixtyfps_interpreter_value_clone(&other.inner, &inner);
        return *this;
    }
    ~Value() { cbindgen_private::sixtyfps_interpreter_value_destructor(&inner); }

    using Type = cbindgen_private::ValueType;

    // only works on Type::Struct
    std::optional<Value> get_field(std::string_view) const;
    // only works on Type::Struct
    bool set_field(std::string_view, Value); // returns false if Value is not a Struct

    // optional<int> to_int() const;
    // optional<float> to_float() const;
    std::optional<double> to_number() const
    {
        if (auto *number = cbindgen_private::sixtyfps_interpreter_value_to_number(&inner)) {
            return *number;
        } else {
            return {};
        }
    }
    std::optional<sixtyfps::SharedString> to_string() const
    {
        if (auto *str = cbindgen_private::sixtyfps_interpreter_value_to_string(&inner)) {
            return *str;
        } else {
            return {};
        }
    }
    std::optional<bool> to_bool() const
    {
        if (auto *b = cbindgen_private::sixtyfps_interpreter_value_to_bool(&inner)) {
            return *b;
        } else {
            return {};
        }
    }
    inline std::optional<sixtyfps::SharedVector<Value>> to_array() const;
    std::optional<std::shared_ptr<sixtyfps::Model<Value>>> to_model() const;
    std::optional<sixtyfps::Brush> to_brush() const
    {
        if (auto *brush = cbindgen_private::sixtyfps_interpreter_value_to_brush(&inner)) {
            return *brush;
        } else {
            return {};
        }
    }
    // std::optional<Struct> to_struct() const;

    // template<typename T> std::optional<T> get() const;
    Value(double value) { cbindgen_private::sixtyfps_interpreter_value_new_double(value, &inner); }
    Value(const SharedString &str)
    {
        cbindgen_private::sixtyfps_interpreter_value_new_string(&str, &inner);
    }
    Value(bool b) { cbindgen_private::sixtyfps_interpreter_value_new_bool(b, &inner); }
    inline Value(const SharedVector<Value> &);
    Value(const std::shared_ptr<sixtyfps::Model<Value>> &);
    Value(const sixtyfps::Brush &brush)
    {
        cbindgen_private::sixtyfps_interpreter_value_new_brush(&brush, &inner);
    }
    // Value(const Struct &);
    explicit Value(Type);

    Type type() const { return cbindgen_private::sixtyfps_interpreter_value_type(&inner); }

private:
    using ValueOpaque = sixtyfps::cbindgen_private::ValueOpaque;
    ValueOpaque inner;
};

inline Value::Value(const sixtyfps::SharedVector<Value> &array)
{
    cbindgen_private::sixtyfps_interpreter_value_new_array(
            &reinterpret_cast<const sixtyfps::SharedVector<ValueOpaque> &>(array), &inner);
}

inline std::optional<sixtyfps::SharedVector<Value>> Value::to_array() const
{
    if (auto *array = cbindgen_private::sixtyfps_interpreter_value_to_array(&inner)) {
        return *reinterpret_cast<const sixtyfps::SharedVector<Value> *>(array);
    } else {
        return {};
    }
}

}